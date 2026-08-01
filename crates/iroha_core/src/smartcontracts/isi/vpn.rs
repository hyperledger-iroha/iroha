//! Native SoraNet VPN lease escrow instruction handlers.

use std::str::FromStr;

use eyre::Result;
use iroha_crypto::derive_non_signing_ed25519_public_key;
#[cfg(test)]
use iroha_crypto::{Algorithm, Hash, KeyPair};
use iroha_data_model::{
    IntoKeyValue,
    account::{Account, AccountId},
    asset::{AssetDefinitionId, AssetId},
    domain::DomainId,
    isi::vpn::{OpenVpnLeaseEscrow, RefundExpiredVpnLease, SettleVpnLease},
    name::Name,
    prelude::*,
    soranet::vpn::{
        VpnLeaseRecordV1, VpnLeaseStatusV1, VpnQuotePolicyV1, VpnSessionReceiptV1,
        VpnUsageVoucherBodyV1, VpnUsageVoucherV1,
    },
    transaction::SignedTransaction,
};
use iroha_primitives::numeric::Quantity;
use mv::storage::StorageReadOnly;

use super::{Error, Execute, asset::isi::assert_numeric_spec_with};
use crate::{
    smartcontracts::isi::domain::isi::ensure_controller_capabilities,
    state::{StateReadOnly, StateTransaction, WorldReadOnly},
};

/// Exact VPN purpose carried by a one-shot numeric movement capability.
pub(in crate::smartcontracts::isi) enum VerifiedVpnNumericPurpose {
    Funding {
        lease_id: [u8; 32],
        authority: AccountId,
    },
    Settlement {
        lease_id: [u8; 32],
        authority: AccountId,
    },
    Refund {
        lease_id: [u8; 32],
        authority: AccountId,
    },
}

/// Non-reusable proof that VPN admission selected an exact atomic movement batch.
pub(in crate::smartcontracts::isi) struct VerifiedVpnNumericBatch {
    purpose: VerifiedVpnNumericPurpose,
    legs: Vec<(AssetId, AssetId, Quantity)>,
}

impl VerifiedVpnNumericBatch {
    fn new(purpose: VerifiedVpnNumericPurpose, legs: Vec<(AssetId, AssetId, Quantity)>) -> Self {
        Self { purpose, legs }
    }

    pub(in crate::smartcontracts::isi) fn into_parts(
        self,
    ) -> (VerifiedVpnNumericPurpose, Vec<(AssetId, AssetId, Quantity)>) {
        (self.purpose, self.legs)
    }
}

const VPN_LEASE_CUSTODY_ACCOUNT_DOMAIN: &str = "iroha-soranet-vpn-lease-v1";

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

fn ensure_positive(value: &Quantity) -> Result<(), Error> {
    if value.is_zero() {
        return Err(validation_err("vpn lease fee must be positive"));
    }
    Ok(())
}

fn ensure_relay_trust_covers_lease(
    relay_id: &[u8; 32],
    quote_policy: &VpnQuotePolicyV1,
    expires_at_ms: u64,
) -> Result<(), Error> {
    if &quote_policy.relay_id != relay_id {
        return Err(validation_err(
            "vpn quote policy relay id must match the lease relay id",
        ));
    }
    if expires_at_ms > quote_policy.relay_trust_valid_until_ms {
        return Err(validation_err(
            "vpn relay trust must remain valid for the complete lease",
        ));
    }
    Ok(())
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
    let asset_definition = asset_definition.to_string();
    let public_key = derive_non_signing_ed25519_public_key(
        VPN_LEASE_CUSTODY_ACCOUNT_DOMAIN.as_bytes(),
        &[
            chain_id.as_str().as_bytes(),
            lease_id,
            asset_definition.as_bytes(),
        ],
    );
    Ok(AccountId::new(public_key))
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
    authorization: VerifiedVpnNumericBatch,
) -> Result<(), Error> {
    crate::smartcontracts::isi::asset::isi::execute_verified_vpn_numeric_batch(
        state_transaction,
        authorization,
    )
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
) -> Result<Quantity, Error> {
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

    let earned_fee = record
        .tariff
        .earned_fee(&voucher.body)
        .map_err(|err| validation_err(format!("vpn tariff arithmetic failed: {err}")))?;
    if receipt.earned_fee != earned_fee {
        return Err(validation_err(
            "vpn receipt earned fee does not match tariff",
        ));
    }
    Ok(earned_fee)
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
        if self.tariff.lease_fee.is_zero() {
            return Err(validation_err("vpn tariff lease fee must be positive"));
        }
        if self.lease_fee != self.tariff.lease_fee {
            return Err(validation_err(
                "vpn lease fee must equal the tariff lease fee",
            ));
        }
        let spec = state_transaction
            .numeric_spec_for(&self.asset_definition)
            .map_err(Error::from)?;
        assert_numeric_spec_with(self.lease_fee.as_numeric(), spec)?;
        state_transaction.world.account(authority)?;
        state_transaction.world.account(&self.operator_account_id)?;
        state_transaction
            .world
            .asset_definition(&self.asset_definition)?;
        let now_ms = state_transaction.block_unix_timestamp_ms();
        if self.expires_at_ms <= now_ms {
            return Err(validation_err("vpn lease expiry must be in the future"));
        }
        ensure_relay_trust_covers_lease(&self.relay_id, &self.quote_policy, self.expires_at_ms)?;
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
            VerifiedVpnNumericBatch::new(
                VerifiedVpnNumericPurpose::Funding {
                    lease_id: self.lease_id,
                    authority: authority.clone(),
                },
                vec![(client_asset, custody_asset, self.lease_fee.clone())],
            ),
        );
        if transfer_result.is_err() && custody_created {
            state_transaction.world.accounts.remove(custody.clone());
        }
        transfer_result?;

        let record = VpnLeaseRecordV1 {
            lease_id: self.lease_id,
            session_id: self.session_id,
            quote_id: self.quote_id,
            client_account_id: authority.clone(),
            operator_account_id: self.operator_account_id,
            metering_public_key: self.metering_public_key,
            asset_definition: self.asset_definition,
            lease_fee: self.lease_fee,
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
            earned_fee: Quantity::zero(),
            refunded_fee: Quantity::zero(),
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

        let earned_fee = verify_vpn_settlement(&record, &self.relay_receipt, &self.client_voucher)?;
        let refund_fee = record
            .lease_fee
            .checked_sub(&earned_fee)
            .map_err(|err| validation_err(format!("vpn refund arithmetic failed: {err}")))?;
        let escrow_asset = custody_asset(&record);
        let mut legs = Vec::new();
        if !earned_fee.is_zero() {
            let operator_asset = operator_asset(&record);
            legs.push((escrow_asset.clone(), operator_asset, earned_fee.clone()));
        }
        if !refund_fee.is_zero() {
            let client_asset = client_asset(&record);
            legs.push((escrow_asset, client_asset, refund_fee.clone()));
        }
        transfer_numeric_asset_for_vpn(
            state_transaction,
            VerifiedVpnNumericBatch::new(
                VerifiedVpnNumericPurpose::Settlement {
                    lease_id: record.lease_id,
                    authority: authority.clone(),
                },
                legs,
            ),
        )?;

        record.status = VpnLeaseStatusV1::Settled;
        record.settled_at_ms = Some(now_ms);
        record.highest_voucher_sequence = self.client_voucher.body.sequence;
        record.client_voucher_hash = Some(self.client_voucher.hash());
        record.relay_receipt_hash = Some(self.relay_receipt.hash());
        record.settled_relay_receipt = Some(self.relay_receipt);
        record.earned_fee = earned_fee;
        record.refunded_fee = refund_fee;
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
        transfer_numeric_asset_for_vpn(
            state_transaction,
            VerifiedVpnNumericBatch::new(
                VerifiedVpnNumericPurpose::Refund {
                    lease_id: record.lease_id,
                    authority: authority.clone(),
                },
                vec![(escrow_asset, client_asset, record.lease_fee.clone())],
            ),
        )?;

        record.status = VpnLeaseStatusV1::Refunded;
        record.refunded_at_ms = Some(now_ms);
        record.refunded_fee = record.lease_fee.clone();
        state_transaction
            .world
            .vpn_leases
            .insert(record.lease_id, record);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use iroha_primitives::numeric::Numeric;

    use super::*;

    fn nano_quantity(nanos: u64) -> Quantity {
        Quantity::from_canonical_numeric(Numeric::new(u128::from(nanos), 9))
            .expect("test nano-XOR value is a valid quantity")
    }

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
    fn vpn_lease_custody_account_id_is_stable_without_a_public_signing_seed() {
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

        let legacy_seed_material = format!(
            "{VPN_LEASE_CUSTODY_ACCOUNT_DOMAIN}|{}|{}|{asset_definition}",
            chain_id.as_str(),
            hex::encode(lease_id),
        );
        let legacy_seed: [u8; Hash::LENGTH] = Hash::new(legacy_seed_material).into();
        let legacy_keypair = KeyPair::try_from_seed(legacy_seed.to_vec(), Algorithm::Ed25519)
            .expect("legacy public seed derives");
        assert_ne!(
            first,
            AccountId::new(legacy_keypair.public_key().clone()),
            "VPN custody must not expose a signing key through public seed derivation"
        );
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
            lease_fee: nano_quantity(1_000),
            active_fee_per_minute: nano_quantity(60),
            ingress_fee_per_mib: nano_quantity(100),
            egress_fee_per_mib: nano_quantity(200),
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
            earned_fee: tariff
                .earned_fee(&voucher.body)
                .expect("test tariff arithmetic succeeds"),
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
            lease_fee: tariff.lease_fee.clone(),
            custody_account_id: checked_account_id(),
            relay_id: voucher.body.relay_id,
            tariff,
            quote_policy: iroha_data_model::soranet::vpn::VpnQuotePolicyV1 {
                exit_class: iroha_data_model::soranet::vpn::VpnExitClassV1::Standard,
                relay_endpoint: "/dns/relay.example/udp/9443/quic".to_owned(),
                relay_id: voucher.body.relay_id,
                descriptor_commit: [0x22; 32],
                tls_server_name: "relay.example".to_owned(),
                relay_tls_spki_sha256: [0xAB; 32],
                relay_certificate_sha256: [0x33; 32],
                directory_snapshot_digest: [0x66; 32],
                relay_trust_valid_until_ms: 10_000,
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
            earned_fee: Quantity::zero(),
            refunded_fee: Quantity::zero(),
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
            receipt.earned_fee
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
        receipt.earned_fee = receipt
            .earned_fee
            .checked_add(&nano_quantity(1))
            .expect("test overclaim remains representable");

        assert!(verify_vpn_settlement(&record, &receipt, &voucher).is_err());
    }

    #[test]
    fn relay_trust_must_cover_complete_lease() {
        let body = VpnUsageVoucherBodyV1 {
            session_id: [0x11; 16],
            quote_id: [0x22; 32],
            relay_id: [0x33; 32],
            sequence: 0,
            ingress_bytes: 0,
            egress_bytes: 0,
            active_ms: 0,
            issued_at_ms: 2_000,
        };
        let (record, _, _) = settlement_record_and_voucher(body);

        ensure_relay_trust_covers_lease(
            &record.relay_id,
            &record.quote_policy,
            record.expires_at_ms,
        )
        .expect("trust valid through the exclusive lease end is accepted");
        let error = ensure_relay_trust_covers_lease(
            &record.relay_id,
            &record.quote_policy,
            record.expires_at_ms + 1,
        )
        .expect_err("trust expiring before the lease must be rejected");
        assert!(error.to_string().contains("complete lease"));
    }
}
