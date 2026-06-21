//! Native SoraNet VPN lease escrow instruction handlers.

use std::str::FromStr;

use eyre::Result;
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    IntoKeyValue,
    account::{Account, AccountId},
    asset::{AssetDefinitionId, AssetId},
    domain::DomainId,
    events::data::prelude::{AssetChanged, AssetEvent},
    fastpq::TransferDeltaTranscript,
    isi::vpn::{OpenVpnLeaseEscrow, RefundExpiredVpnLease, SettleVpnLease},
    name::Name,
    prelude::*,
    soranet::vpn::{
        VpnLeaseRecordV1, VpnLeaseStatusV1, VpnSessionReceiptV1, VpnUsageVoucherBodyV1,
        VpnUsageVoucherV1,
    },
    transaction::SignedTransaction,
};
use iroha_primitives::numeric::Numeric;
use mv::storage::StorageReadOnly;

use super::{
    Error, Execute,
    asset::isi::{
        NumericAssetTransferSourcePolicy, apply_numeric_asset_transfer_delta,
        assert_numeric_spec_with, prepare_outbound_asset_transfer_control_update,
        update_control_record,
    },
};
use crate::{
    smartcontracts::isi::domain::isi::ensure_controller_capabilities,
    state::{StateReadOnly, StateTransaction, WorldReadOnly},
};

const VPN_LEASE_CUSTODY_SEED_LABEL: &str = "iroha-soranet-vpn-lease-v1";

fn validation_err(message: impl Into<String>) -> Error {
    iroha_data_model::isi::error::InstructionExecutionError::InvariantViolation(
        message.into().into(),
    )
}

fn ensure_non_zero_32(label: &str, value: &[u8; 32]) -> Result<(), Error> {
    if *value == [0u8; 32] {
        return Err(validation_err(format!("{label} must not be zero")));
    }
    Ok(())
}

fn ensure_non_zero_16(label: &str, value: &[u8; 16]) -> Result<(), Error> {
    if *value == [0u8; 16] {
        return Err(validation_err(format!("{label} must not be zero")));
    }
    Ok(())
}

fn ensure_positive(value: &Numeric) -> Result<(), Error> {
    if value.mantissa().is_negative() || value.is_zero() {
        return Err(validation_err("vpn lease fee must be positive"));
    }
    Ok(())
}

fn numeric_from_nanos(nanos: u64) -> Numeric {
    Numeric::new(u128::from(nanos), 9)
}

fn hash_to_bytes(hash: &iroha_crypto::HashOf<SignedTransaction>) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(hash.as_ref());
    bytes
}

fn account_hash(account_id: &AccountId) -> [u8; 32] {
    *blake3::hash(account_id.to_string().as_bytes()).as_bytes()
}

fn xor_asset_definition_id() -> AssetDefinitionId {
    let domain =
        DomainId::parse_fully_qualified("universal.universal").expect("static XOR domain id");
    let name = Name::from_str("xor").expect("static XOR asset name");
    AssetDefinitionId::new(domain, name)
}

fn ensure_xor_asset(asset_definition: &AssetDefinitionId) -> Result<(), Error> {
    let configured = iroha_config::parameters::defaults::soranet::vpn::fee_asset_id();
    if asset_definition != &xor_asset_definition_id() {
        return Err(validation_err(format!(
            "vpn lease fee asset must be XOR ({configured})"
        )));
    }
    Ok(())
}

/// Derive the deterministic protocol custody account for a VPN lease.
pub fn vpn_lease_custody_account_id(
    chain_id: &iroha_data_model::ChainId,
    lease_id: &[u8; 32],
    asset_definition: &AssetDefinitionId,
) -> Result<AccountId, Error> {
    let seed_material = format!(
        "{VPN_LEASE_CUSTODY_SEED_LABEL}|{}|{}|{asset_definition}",
        chain_id.as_str(),
        hex::encode(lease_id),
    );
    let seed: [u8; Hash::LENGTH] = Hash::new(seed_material).into();
    let keypair = KeyPair::try_from_seed(seed.to_vec(), Algorithm::Ed25519).map_err(|err| {
        validation_err(format!(
            "vpn lease custody account seed was rejected: {err}"
        ))
    })?;
    Ok(AccountId::new(keypair.public_key().clone()))
}

fn ensure_custody_account(
    custody: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<bool, Error> {
    ensure_controller_capabilities(
        custody.controller(),
        &state_transaction.crypto.allowed_signing,
        &state_transaction.crypto.allowed_curve_ids,
    )?;
    if state_transaction.world.account(custody).is_ok() {
        return Ok(false);
    }
    let account = Account {
        id: custody.clone(),
        metadata: Metadata::default(),
        label: None,
        uaid: None,
        opaque_ids: Vec::new(),
    };
    let (id, value) = account.into_key_value();
    state_transaction.world.accounts.insert(id, value);
    Ok(true)
}

fn transfer_numeric_asset_for_vpn(
    state_transaction: &mut StateTransaction<'_, '_>,
    source_id: &AssetId,
    destination_id: &AssetId,
    amount: &Numeric,
    source_policy: NumericAssetTransferSourcePolicy,
) -> Result<TransferDeltaTranscript, Error> {
    let control_update =
        prepare_outbound_asset_transfer_control_update(state_transaction, source_id, amount)?;
    let (source_id, destination_id, delta) = apply_numeric_asset_transfer_delta(
        state_transaction,
        source_id,
        destination_id,
        amount,
        source_policy,
    )?;
    if let Some(record) = control_update {
        update_control_record(state_transaction, source_id.account(), record)?;
    }

    #[allow(clippy::float_arithmetic)]
    #[cfg(feature = "telemetry")]
    state_transaction
        .telemetry
        .observe_tx_amount(amount.clone().to_f64());

    state_transaction.world.emit_events([
        AssetEvent::Removed(AssetChanged {
            asset: source_id,
            amount: amount.clone(),
        }),
        AssetEvent::Added(AssetChanged {
            asset: destination_id,
            amount: amount.clone(),
        }),
    ]);

    Ok(delta)
}

fn custody_asset(record: &VpnLeaseRecordV1) -> AssetId {
    AssetId::new(
        record.asset_definition.clone(),
        record.custody_account_id.clone(),
    )
}

fn client_asset(record: &VpnLeaseRecordV1) -> AssetId {
    AssetId::new(
        record.asset_definition.clone(),
        record.client_account_id.clone(),
    )
}

fn operator_asset(record: &VpnLeaseRecordV1) -> AssetId {
    AssetId::new(
        record.asset_definition.clone(),
        record.operator_account_id.clone(),
    )
}

fn verify_receipt_ids(
    record: &VpnLeaseRecordV1,
    receipt: &VpnSessionReceiptV1,
    voucher_body: &VpnUsageVoucherBodyV1,
) -> Result<(), Error> {
    if receipt.session_id != record.session_id || voucher_body.session_id != record.session_id {
        return Err(validation_err("vpn settlement session id mismatch"));
    }
    if receipt.quote_id != record.quote_id || voucher_body.quote_id != record.quote_id {
        return Err(validation_err("vpn settlement quote id mismatch"));
    }
    if receipt.relay_id != record.relay_id || voucher_body.relay_id != record.relay_id {
        return Err(validation_err("vpn settlement relay id mismatch"));
    }
    if receipt.payment_tx_hash != record.open_tx_hash {
        return Err(validation_err(
            "vpn settlement payment transaction mismatch",
        ));
    }
    if receipt.account_hash != account_hash(&record.client_account_id) {
        return Err(validation_err("vpn settlement client account mismatch"));
    }
    Ok(())
}

fn verify_vpn_settlement(
    record: &VpnLeaseRecordV1,
    receipt: &VpnSessionReceiptV1,
    voucher: &VpnUsageVoucherV1,
) -> Result<u64, Error> {
    verify_receipt_ids(record, receipt, &voucher.body)?;
    if voucher.client_public_key != record.metering_public_key {
        return Err(validation_err("vpn voucher public key mismatch"));
    }
    voucher.verify().map_err(|err| {
        validation_err(format!("vpn voucher signature verification failed: {err}"))
    })?;
    if receipt.client_voucher_hash != voucher.hash() {
        return Err(validation_err("vpn receipt voucher hash mismatch"));
    }
    if receipt.highest_voucher_sequence != voucher.body.sequence {
        return Err(validation_err("vpn receipt voucher sequence mismatch"));
    }
    if receipt.ingress_bytes != voucher.body.ingress_bytes
        || receipt.egress_bytes != voucher.body.egress_bytes
    {
        return Err(validation_err("vpn receipt byte counters mismatch"));
    }
    if u64::from(receipt.uptime_secs).saturating_mul(1_000) < voucher.body.active_ms {
        return Err(validation_err(
            "vpn receipt uptime is below voucher active time",
        ));
    }
    if receipt.ended_at_ms < receipt.started_at_ms {
        return Err(validation_err("vpn receipt end timestamp precedes start"));
    }

    let earned_fee_nanos = record.tariff.earned_fee_nanos(&voucher.body);
    if receipt.earned_fee_nanos != earned_fee_nanos {
        return Err(validation_err(
            "vpn receipt earned fee does not match tariff",
        ));
    }
    Ok(earned_fee_nanos)
}

impl Execute for OpenVpnLeaseEscrow {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        if state_transaction
            .world
            .vpn_leases
            .get(&self.lease_id)
            .is_some()
        {
            return Err(validation_err("vpn lease already exists"));
        }
        ensure_non_zero_32("vpn lease id", &self.lease_id)?;
        ensure_non_zero_16("vpn session id", &self.session_id)?;
        ensure_non_zero_32("vpn quote id", &self.quote_id)?;
        ensure_non_zero_32("vpn relay id", &self.relay_id)?;
        if &self.operator_account_id == authority {
            return Err(validation_err("vpn operator cannot fund own client lease"));
        }
        ensure_xor_asset(&self.asset_definition)?;
        ensure_positive(&self.lease_fee)?;
        if self.tariff.lease_fee_nanos == 0 {
            return Err(validation_err("vpn tariff lease fee must be positive"));
        }
        if self.lease_fee != self.tariff.lease_fee_numeric() {
            return Err(validation_err(
                "vpn lease fee must equal tariff lease_fee_nanos at nano-XOR scale",
            ));
        }
        let spec = state_transaction
            .numeric_spec_for(&self.asset_definition)
            .map_err(Error::from)?;
        assert_numeric_spec_with(&self.lease_fee, spec)?;
        state_transaction.world.account(authority)?;
        state_transaction.world.account(&self.operator_account_id)?;
        state_transaction
            .world
            .asset_definition(&self.asset_definition)?;
        let now_ms = state_transaction.block_unix_timestamp_ms();
        if self.expires_at_ms <= now_ms {
            return Err(validation_err("vpn lease expiry must be in the future"));
        }
        if self.settlement_grace_ms == 0 {
            return Err(validation_err(
                "vpn lease settlement grace window must be non-zero",
            ));
        }

        let open_tx_hash = state_transaction
            .current_tx_hash
            .as_ref()
            .map(hash_to_bytes)
            .ok_or_else(|| validation_err("vpn lease opening requires a transaction hash"))?;
        let custody = vpn_lease_custody_account_id(
            state_transaction.chain_id(),
            &self.lease_id,
            &self.asset_definition,
        )?;
        let client_asset = AssetId::new(self.asset_definition.clone(), authority.clone());
        let custody_asset = AssetId::new(self.asset_definition.clone(), custody.clone());
        let custody_created = ensure_custody_account(&custody, state_transaction)?;
        let transfer_result = transfer_numeric_asset_for_vpn(
            state_transaction,
            &client_asset,
            &custody_asset,
            &self.lease_fee,
            NumericAssetTransferSourcePolicy::User,
        );
        if transfer_result.is_err() && custody_created {
            state_transaction.world.accounts.remove(custody.clone());
        }
        let delta = transfer_result?;
        state_transaction.record_transfer_transcript(authority, delta)?;

        let record = VpnLeaseRecordV1 {
            lease_id: self.lease_id,
            session_id: self.session_id,
            quote_id: self.quote_id,
            client_account_id: authority.clone(),
            operator_account_id: self.operator_account_id,
            metering_public_key: self.metering_public_key,
            asset_definition: self.asset_definition,
            lease_fee: self.lease_fee,
            lease_fee_nanos: self.tariff.lease_fee_nanos,
            custody_account_id: custody,
            relay_id: self.relay_id,
            tariff: self.tariff,
            quote_policy: self.quote_policy,
            open_tx_hash,
            status: VpnLeaseStatusV1::Active,
            opened_at_ms: now_ms,
            expires_at_ms: self.expires_at_ms,
            settlement_grace_ms: self.settlement_grace_ms,
            settled_at_ms: None,
            refunded_at_ms: None,
            highest_voucher_sequence: 0,
            client_voucher_hash: None,
            relay_receipt_hash: None,
            settled_relay_receipt: None,
            earned_fee_nanos: 0,
            refunded_fee_nanos: 0,
        };
        state_transaction
            .world
            .vpn_leases
            .insert(record.lease_id, record);
        Ok(())
    }
}

impl Execute for SettleVpnLease {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let Some(mut record) = state_transaction
            .world
            .vpn_leases
            .get(&self.lease_id)
            .cloned()
        else {
            return Err(validation_err("vpn lease not found"));
        };
        if record.status != VpnLeaseStatusV1::Active {
            return Err(validation_err("vpn lease is not active"));
        }
        if &record.operator_account_id != authority {
            return Err(validation_err("only vpn operator may settle lease"));
        }
        let now_ms = state_transaction.block_unix_timestamp_ms();
        if now_ms > record.refund_available_at_ms() {
            return Err(validation_err("vpn lease settlement grace window expired"));
        }

        let earned_fee_nanos =
            verify_vpn_settlement(&record, &self.relay_receipt, &self.client_voucher)?;
        let refund_fee_nanos = record.lease_fee_nanos.saturating_sub(earned_fee_nanos);
        let escrow_asset = custody_asset(&record);
        let mut deltas = Vec::new();
        if earned_fee_nanos != 0 {
            let operator_asset = operator_asset(&record);
            let earned = numeric_from_nanos(earned_fee_nanos);
            let delta = transfer_numeric_asset_for_vpn(
                state_transaction,
                &escrow_asset,
                &operator_asset,
                &earned,
                NumericAssetTransferSourcePolicy::NativeEscrowCustody,
            )?;
            deltas.push(delta);
        }
        if refund_fee_nanos != 0 {
            let client_asset = client_asset(&record);
            let refund = numeric_from_nanos(refund_fee_nanos);
            let delta = transfer_numeric_asset_for_vpn(
                state_transaction,
                &escrow_asset,
                &client_asset,
                &refund,
                NumericAssetTransferSourcePolicy::NativeEscrowCustody,
            )?;
            deltas.push(delta);
        }
        state_transaction.record_transfer_transcripts(authority, deltas)?;

        record.status = VpnLeaseStatusV1::Settled;
        record.settled_at_ms = Some(now_ms);
        record.highest_voucher_sequence = self.client_voucher.body.sequence;
        record.client_voucher_hash = Some(self.client_voucher.hash());
        record.relay_receipt_hash = Some(self.relay_receipt.hash());
        record.settled_relay_receipt = Some(self.relay_receipt);
        record.earned_fee_nanos = earned_fee_nanos;
        record.refunded_fee_nanos = refund_fee_nanos;
        state_transaction
            .world
            .vpn_leases
            .insert(record.lease_id, record);
        Ok(())
    }
}

impl Execute for RefundExpiredVpnLease {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let Some(mut record) = state_transaction
            .world
            .vpn_leases
            .get(&self.lease_id)
            .cloned()
        else {
            return Err(validation_err("vpn lease not found"));
        };
        if record.status != VpnLeaseStatusV1::Active {
            return Err(validation_err("vpn lease is not active"));
        }
        let now_ms = state_transaction.block_unix_timestamp_ms();
        if now_ms < record.refund_available_at_ms() {
            return Err(validation_err("vpn lease refund is not available yet"));
        }
        let escrow_asset = custody_asset(&record);
        let client_asset = client_asset(&record);
        let delta = transfer_numeric_asset_for_vpn(
            state_transaction,
            &escrow_asset,
            &client_asset,
            &record.lease_fee,
            NumericAssetTransferSourcePolicy::NativeEscrowCustody,
        )?;
        state_transaction.record_transfer_transcript(authority, delta)?;

        record.status = VpnLeaseStatusV1::Refunded;
        record.refunded_at_ms = Some(now_ms);
        record.refunded_fee_nanos = record.lease_fee_nanos;
        state_transaction
            .world
            .vpn_leases
            .insert(record.lease_id, record);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checked_keypair() -> KeyPair {
        KeyPair::try_random().expect("VPN fixture key generation should succeed")
    }

    fn checked_account_id() -> AccountId {
        AccountId::new(checked_keypair().public_key().clone())
    }

    #[test]
    fn checked_keypair_preserves_default_algorithm() {
        assert_eq!(checked_keypair().algorithm(), Algorithm::default());
    }

    #[test]
    fn xor_asset_check_accepts_canonical_xor_id() {
        ensure_xor_asset(&xor_asset_definition_id()).expect("canonical XOR asset id");
    }

    #[test]
    fn vpn_lease_custody_account_id_uses_checked_deterministic_seed() {
        let chain_id = iroha_data_model::ChainId::from("vpn-custody-chain");
        let asset_definition = xor_asset_definition_id();
        let lease_id = [0x11; 32];

        let first = vpn_lease_custody_account_id(&chain_id, &lease_id, &asset_definition)
            .expect("custody account derivation succeeds");
        let second = vpn_lease_custody_account_id(&chain_id, &lease_id, &asset_definition)
            .expect("custody account derivation is repeatable");
        assert_eq!(first, second);

        let mut different_lease_id = lease_id;
        different_lease_id[0] ^= 0x01;
        let different =
            vpn_lease_custody_account_id(&chain_id, &different_lease_id, &asset_definition)
                .expect("different custody account derivation succeeds");
        assert_ne!(first, different);
    }

    fn settlement_record_and_voucher(
        mut voucher_body: VpnUsageVoucherBodyV1,
    ) -> (VpnLeaseRecordV1, VpnSessionReceiptV1, VpnUsageVoucherV1) {
        let key_pair = checked_keypair();
        voucher_body.sequence = 9;
        let voucher = VpnUsageVoucherV1::try_sign(voucher_body, key_pair.private_key())
            .expect("vpn usage voucher fixture should sign");
        let client_account_id = AccountId::new(key_pair.public_key().clone());
        let operator_key = checked_keypair();
        let tariff = iroha_data_model::soranet::vpn::VpnTariffV1 {
            lease_fee_nanos: 1_000,
            active_fee_nanos_per_minute: 60,
            ingress_fee_nanos_per_mib: 100,
            egress_fee_nanos_per_mib: 200,
        };
        let receipt = VpnSessionReceiptV1 {
            session_id: voucher.body.session_id,
            quote_id: voucher.body.quote_id,
            payment_tx_hash: [0x44; 32],
            account_hash: account_hash(&client_account_id),
            relay_id: voucher.body.relay_id,
            ingress_bytes: voucher.body.ingress_bytes,
            egress_bytes: voucher.body.egress_bytes,
            cover_bytes: 0,
            uptime_secs: 2,
            started_at_ms: 1_000,
            ended_at_ms: 3_000,
            exit_class: iroha_data_model::soranet::vpn::VpnExitClassV1::Standard,
            meter_hash: [0x55; 32],
            earned_fee_nanos: tariff.earned_fee_nanos(&voucher.body),
            highest_voucher_sequence: voucher.body.sequence,
            client_voucher_hash: voucher.hash(),
        };
        let record = VpnLeaseRecordV1 {
            lease_id: [0xAA; 32],
            session_id: voucher.body.session_id,
            quote_id: voucher.body.quote_id,
            client_account_id,
            operator_account_id: AccountId::new(operator_key.public_key().clone()),
            metering_public_key: key_pair.public_key().clone(),
            asset_definition: xor_asset_definition_id(),
            lease_fee: tariff.lease_fee_numeric(),
            lease_fee_nanos: tariff.lease_fee_nanos,
            custody_account_id: checked_account_id(),
            relay_id: voucher.body.relay_id,
            tariff,
            quote_policy: iroha_data_model::soranet::vpn::VpnQuotePolicyV1 {
                exit_class: iroha_data_model::soranet::vpn::VpnExitClassV1::Standard,
                relay_endpoint: "/dns/relay.example/udp/9443/quic".to_owned(),
                lease_secs: 600,
                meter_family: "soranet.vpn.standard".to_owned(),
                fee_asset_id: "xor#universal.universal".to_owned(),
                escrow_account_id: checked_account_id(),
                route_pushes: vec!["0.0.0.0/0".to_owned()],
                excluded_routes: Vec::new(),
                dns_servers: vec!["1.1.1.1".to_owned()],
                tunnel_addresses: vec!["10.208.0.2/32".to_owned()],
                mtu_bytes: u64::from(iroha_data_model::soranet::vpn::VPN_DEFAULT_TUNNEL_MTU_BYTES),
                flow_label_bits: 24,
                padding_budget_ms: 15,
                relay_tls_spki_sha256_hex: Some("ab".repeat(32)),
            },
            open_tx_hash: [0x44; 32],
            status: VpnLeaseStatusV1::Active,
            opened_at_ms: 1_000,
            expires_at_ms: 10_000,
            settlement_grace_ms: 1_000,
            settled_at_ms: None,
            refunded_at_ms: None,
            highest_voucher_sequence: 0,
            client_voucher_hash: None,
            relay_receipt_hash: None,
            settled_relay_receipt: None,
            earned_fee_nanos: 0,
            refunded_fee_nanos: 0,
        };
        (record, receipt, voucher)
    }

    #[test]
    fn settlement_verifies_voucher_and_recomputes_tariff() {
        let body = VpnUsageVoucherBodyV1 {
            session_id: [0x11; 16],
            quote_id: [0x22; 32],
            relay_id: [0x33; 32],
            sequence: 0,
            ingress_bytes: 1_048_576,
            egress_bytes: 1,
            active_ms: 1_500,
            issued_at_ms: 2_000,
        };
        let (record, receipt, voucher) = settlement_record_and_voucher(body);

        assert_eq!(
            verify_vpn_settlement(&record, &receipt, &voucher).expect("settlement valid"),
            receipt.earned_fee_nanos
        );
    }

    #[test]
    fn settlement_rejects_relay_overclaim() {
        let body = VpnUsageVoucherBodyV1 {
            session_id: [0x11; 16],
            quote_id: [0x22; 32],
            relay_id: [0x33; 32],
            sequence: 0,
            ingress_bytes: 1_048_576,
            egress_bytes: 1,
            active_ms: 1_500,
            issued_at_ms: 2_000,
        };
        let (record, mut receipt, voucher) = settlement_record_and_voucher(body);
        receipt.earned_fee_nanos = receipt.earned_fee_nanos.saturating_add(1);

        assert!(verify_vpn_settlement(&record, &receipt, &voucher).is_err());
    }
}
