//! Native SoraNet VPN lease escrow instruction handlers.
use super::{Error, Execute, asset::isi::assert_numeric_spec_with};
use crate::{
    smartcontracts::isi::domain::isi::ensure_controller_capabilities,
    state::{StateReadOnly, StateTransaction, WorldReadOnly},
};
use eyre::Result;
use iroha_crypto::{Algorithm, derive_non_signing_ed25519_public_key};
#[cfg(test)]
use iroha_crypto::{Hash, HashOf, KeyPair};
use iroha_data_model::{
    IntoKeyValue,
    account::{Account, AccountId},
    asset::{AssetDefinitionId, AssetId},
    domain::DomainId,
    isi::vpn::{OpenVpnLeaseEscrow, RefundExpiredVpnLease, SettleVpnLease},
    name::Name,
    prelude::*,
    soranet::vpn::{
        VPN_DEFAULT_TUNNEL_MTU_BYTES, VpnLeaseRecordV1, VpnLeaseStatusV1, VpnQuoteBodyV1,
        VpnQuotePolicyV1, VpnSessionReceiptV1, VpnSignedQuoteV1, VpnUsageVoucherBodyV1,
        VpnUsageVoucherV1, derive_vpn_address_plan_v1, derive_vpn_lease_id_v1,
        derive_vpn_session_id_v1,
    },
    transaction::SignedTransaction,
};
use iroha_executor_data_model::permission::soranet::CanIssueSoranetVpnQuote;
use iroha_primitives::numeric::Quantity;
use mv::storage::StorageReadOnly;
use norito::codec::Encode;
use std::str::FromStr;
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
const VPN_MAX_SIGNED_QUOTE_BYTES_V1: usize = 32 * 1024;
const VPN_MAX_RELAY_ENDPOINT_BYTES_V1: usize = 1_024;
const VPN_MAX_TLS_SERVER_NAME_BYTES_V1: usize = 253;
const VPN_MAX_METER_FAMILY_BYTES_V1: usize = 128;
const VPN_MAX_ROUTE_ENTRIES_V1: usize = 64;
const VPN_MAX_ROUTE_BYTES_V1: usize = 128;
const VPN_MAX_DNS_ENTRIES_V1: usize = 8;
const VPN_MAX_DNS_BYTES_V1: usize = 64;
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
fn ensure_non_zero_policy_commitment(label: &str, value: &[u8; 32]) -> Result<(), Error> {
    if *value == [0_u8; 32] {
        return Err(validation_err(format!("{label} must not be zero")));
    }
    Ok(())
}
fn ensure_bounded_canonical_text(label: &str, value: &str, max_bytes: usize) -> Result<(), Error> {
    if value.is_empty()
        || value != value.trim()
        || value.len() > max_bytes
        || value.chars().any(char::is_control)
    {
        return Err(validation_err(format!(
            "{label} must be non-empty, unpadded, control-free UTF-8 of at most {max_bytes} bytes"
        )));
    }
    Ok(())
}
fn ensure_bounded_canonical_text_list(
    label: &str,
    values: &[String],
    minimum_items: usize,
    maximum_items: usize,
    maximum_item_bytes: usize,
) -> Result<(), Error> {
    if !(minimum_items..=maximum_items).contains(&values.len()) {
        return Err(validation_err(format!(
            "{label} must contain {minimum_items}..={maximum_items} entries"
        )));
    }
    for (index, value) in values.iter().enumerate() {
        ensure_bounded_canonical_text(label, value, maximum_item_bytes)?;
        if values[..index].iter().any(|previous| previous == value) {
            return Err(validation_err(format!(
                "{label} must not contain duplicate entries"
            )));
        }
    }
    Ok(())
}
fn ensure_canonical_quote_policy(
    body: &VpnQuoteBodyV1,
    custody_account_id: &AccountId,
) -> Result<(), Error> {
    let policy = &body.policy;
    ensure_non_zero_32("vpn relay id", &policy.relay_id)?;
    ensure_non_zero_policy_commitment(
        "vpn relay descriptor commitment",
        &policy.descriptor_commit,
    )?;
    ensure_non_zero_policy_commitment("vpn relay TLS SPKI pin", &policy.relay_tls_spki_sha256)?;
    ensure_non_zero_policy_commitment(
        "vpn relay certificate commitment",
        &policy.relay_certificate_sha256,
    )?;
    ensure_non_zero_policy_commitment(
        "vpn directory snapshot commitment",
        &policy.directory_snapshot_digest,
    )?;
    ensure_bounded_canonical_text(
        "vpn quote relay endpoint",
        &policy.relay_endpoint,
        VPN_MAX_RELAY_ENDPOINT_BYTES_V1,
    )?;
    ensure_bounded_canonical_text(
        "vpn quote TLS server name",
        &policy.tls_server_name,
        VPN_MAX_TLS_SERVER_NAME_BYTES_V1,
    )?;
    ensure_bounded_canonical_text(
        "vpn quote meter family",
        &policy.meter_family,
        VPN_MAX_METER_FAMILY_BYTES_V1,
    )?;
    ensure_bounded_canonical_text_list(
        "vpn quote routes",
        &policy.route_pushes,
        1,
        VPN_MAX_ROUTE_ENTRIES_V1,
        VPN_MAX_ROUTE_BYTES_V1,
    )?;
    ensure_bounded_canonical_text_list(
        "vpn quote excluded routes",
        &policy.excluded_routes,
        0,
        VPN_MAX_ROUTE_ENTRIES_V1,
        VPN_MAX_ROUTE_BYTES_V1,
    )?;
    ensure_bounded_canonical_text_list(
        "vpn quote DNS resolvers",
        &policy.dns_servers,
        1,
        VPN_MAX_DNS_ENTRIES_V1,
        VPN_MAX_DNS_BYTES_V1,
    )?;
    if policy.route_pushes.iter().any(|route| {
        policy
            .excluded_routes
            .iter()
            .any(|excluded| excluded == route)
    }) {
        return Err(validation_err(
            "vpn quote included and excluded route sets must be disjoint",
        ));
    }
    if policy.flow_label_bits != 24 {
        return Err(validation_err("vpn quote flow-label width must be 24 bits"));
    }
    if policy.mtu_bytes != u64::from(VPN_DEFAULT_TUNNEL_MTU_BYTES) {
        return Err(validation_err(format!(
            "vpn quote MTU must be {VPN_DEFAULT_TUNNEL_MTU_BYTES} bytes"
        )));
    }
    if policy.padding_budget_ms == 0 {
        return Err(validation_err(
            "vpn quote padding budget must be greater than zero",
        ));
    }
    if !body.address_slot.is_valid() {
        return Err(validation_err(
            "vpn quote address slot is outside the V1 pool",
        ));
    }
    let expected_addresses = derive_vpn_address_plan_v1(body.address_slot);
    if policy.tunnel_addresses != expected_addresses.client_tunnel_addresses {
        return Err(validation_err(
            "vpn quote tunnel addresses do not match its typed address slot",
        ));
    }
    if policy.fee_asset_id != body.asset_definition.to_string() {
        return Err(validation_err(
            "vpn quote fee asset label does not match its typed asset definition",
        ));
    }
    if &policy.escrow_account_id != custody_account_id {
        return Err(validation_err(
            "vpn quote escrow account does not match deterministic protocol custody",
        ));
    }
    let lease_duration_ms = body
        .expires_at_ms
        .checked_sub(body.valid_after_ms)
        .ok_or_else(|| validation_err("vpn quote expiry precedes its validity start"))?;
    let expected_duration_ms = policy
        .lease_secs
        .checked_mul(1_000)
        .ok_or_else(|| validation_err("vpn quote lease duration overflows milliseconds"))?;
    if lease_duration_ms != expected_duration_ms || policy.lease_secs == 0 {
        return Err(validation_err(
            "vpn quote policy duration does not match its signed validity interval",
        ));
    }
    ensure_relay_trust_covers_lease(&policy.relay_id, policy, body.expires_at_ms)
}
fn vpn_quote_operator_is_authorized<W: WorldReadOnly>(
    operator_account_id: &AccountId,
    world: &W,
) -> Result<bool, Error> {
    let required_permission: Permission = CanIssueSoranetVpnQuote.into();
    Ok(world
        .account_permissions_iter(operator_account_id)?
        .into_iter()
        .any(|permission| permission == &required_permission)
        || world
            .account_roles_iter(operator_account_id)
            .any(|role_id| {
                world.roles().get(role_id).is_some_and(|role| {
                    role.permissions()
                        .any(|permission| permission == &required_permission)
                })
            }))
}
fn verify_operator_quote(
    quote: &VpnSignedQuoteV1,
    authority: &AccountId,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<AccountId, Error> {
    quote.verify().map_err(|error| {
        validation_err(format!("vpn operator quote signature is invalid: {error}"))
    })?;
    let quote_size = quote.encode().len();
    if quote_size > VPN_MAX_SIGNED_QUOTE_BYTES_V1 {
        return Err(validation_err(format!(
            "vpn signed quote has {quote_size} bytes; maximum is {VPN_MAX_SIGNED_QUOTE_BYTES_V1}"
        )));
    }
    let body = &quote.body;
    if !vpn_quote_operator_is_authorized(&body.operator_account_id, &state_transaction.world)? {
        return Err(validation_err(
            "vpn quote signer does not hold CanIssueSoranetVpnQuote",
        ));
    }
    if &body.network_id != state_transaction.network_id() {
        return Err(validation_err(
            "vpn quote is bound to a different exact network",
        ));
    }
    if &body.client_account_id != authority {
        return Err(validation_err(
            "vpn quote client account does not match transaction authority",
        ));
    }
    ensure_non_zero_32("vpn quote id", &body.quote_id)?;
    ensure_non_zero_32("vpn lease id", &body.lease_id)?;
    ensure_non_zero_16("vpn session id", &body.session_id)?;
    let expected_lease_id =
        derive_vpn_lease_id_v1(&body.network_id, body.quote_id, &body.client_account_id);
    if body.lease_id != expected_lease_id {
        return Err(validation_err(
            "vpn lease id is not the canonical network/client/quote derivation",
        ));
    }
    let expected_session_id = derive_vpn_session_id_v1(
        &body.network_id,
        body.quote_id,
        &body.client_account_id,
        body.address_slot,
    );
    if body.session_id != expected_session_id {
        return Err(validation_err(
            "vpn session id is not the canonical network/client/quote/slot derivation",
        ));
    }
    let now_ms = state_transaction.block_unix_timestamp_ms();
    if now_ms < body.valid_after_ms || now_ms >= body.expires_at_ms {
        return Err(validation_err("vpn quote is not currently valid"));
    }
    if body.settlement_grace_ms == 0
        || body
            .expires_at_ms
            .checked_add(body.settlement_grace_ms)
            .is_none()
    {
        return Err(validation_err(
            "vpn quote settlement grace must be non-zero and timestamp-safe",
        ));
    }
    ensure_xor_asset(&body.asset_definition)?;
    ensure_positive(&body.tariff.lease_fee)?;
    if body.metering_public_key.try_algorithm() != Ok(Algorithm::Ed25519) {
        return Err(validation_err(
            "vpn metering public key must use Ed25519 for the V1 helper-ticket format",
        ));
    }
    let custody = vpn_lease_custody_account_id(
        state_transaction.network_id(),
        &body.lease_id,
        &body.asset_definition,
    )?;
    ensure_canonical_quote_policy(body, &custody)?;
    Ok(custody)
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
    AssetDefinitionId::derive_from_components(domain, name)
}
fn ensure_xor_asset(asset_definition: &AssetDefinitionId) -> Result<(), Error> {
    let expected = xor_asset_definition_id();
    if asset_definition != &expected {
        return Err(validation_err(format!(
            "vpn lease fee asset must be XOR ({expected})"
        )));
    }
    Ok(())
}
/// Derive the deterministic protocol custody account for a VPN lease.
pub fn vpn_lease_custody_account_id(
    network_id: &iroha_data_model::NetworkId,
    lease_id: &[u8; 32],
    asset_definition: &AssetDefinitionId,
) -> Result<AccountId, Error> {
    let asset_definition = asset_definition.to_string();
    let public_key = derive_non_signing_ed25519_public_key(
        VPN_LEASE_CUSTODY_ACCOUNT_DOMAIN.as_bytes(),
        &[network_id.as_bytes(), lease_id, asset_definition.as_bytes()],
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
/// Return whether an account is still bound by a funded active VPN lease.
///
/// Rekeying or unregistering such an account would invalidate the signed
/// client binding and make the eventual escrow refund undeliverable.
pub(crate) fn is_active_vpn_client<W: WorldReadOnly>(world: &W, account_id: &AccountId) -> bool {
    world
        .vpn_active_lease_by_account()
        .get(account_id)
        .is_some()
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
    if receipt.exit_class != record.quote_policy.exit_class {
        return Err(validation_err("vpn receipt exit class mismatch"));
    }
    if receipt.started_at_ms < record.opened_at_ms || receipt.ended_at_ms > record.expires_at_ms {
        return Err(validation_err(
            "vpn receipt service interval falls outside the signed lease",
        ));
    }
    if voucher.body.issued_at_ms < record.opened_at_ms
        || voucher.body.issued_at_ms > record.expires_at_ms
    {
        return Err(validation_err(
            "vpn voucher issuance timestamp falls outside the signed lease",
        ));
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
fn refund_active_vpn_lease(
    mut record: VpnLeaseRecordV1,
    authority: &AccountId,
    now_ms: u64,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    if record.status != VpnLeaseStatusV1::Active {
        return Err(validation_err("vpn lease is not active"));
    }
    if now_ms < record.refund_available_at_ms() {
        return Err(validation_err("vpn lease refund is not available yet"));
    }
    let escrow_asset = custody_asset(&record);
    let destination = client_asset(&record);
    transfer_numeric_asset_for_vpn(
        state_transaction,
        VerifiedVpnNumericBatch::new(
            VerifiedVpnNumericPurpose::Refund {
                lease_id: record.lease_id,
                authority: authority.clone(),
            },
            vec![(escrow_asset, destination, record.lease_fee.clone())],
        ),
    )?;
    record.status = VpnLeaseStatusV1::Refunded;
    record.refunded_at_ms = Some(now_ms);
    record.refunded_fee = record.lease_fee.clone();
    state_transaction
        .world
        .put_vpn_lease(record)
        .map_err(validation_err)?;
    Ok(())
}
fn release_expired_vpn_claims_for_quote(
    body: &VpnQuoteBodyV1,
    authority: &AccountId,
    now_ms: u64,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    if let Some(existing_lease_id) = state_transaction
        .world
        .vpn_active_lease_by_account
        .get(&body.client_account_id)
        .copied()
    {
        let record = state_transaction
            .world
            .vpn_leases
            .get(&existing_lease_id)
            .cloned()
            .ok_or_else(|| validation_err("vpn active-account index is stale"))?;
        refund_active_vpn_lease(record, authority, now_ms, state_transaction)?;
    }
    if let Some(existing_lease_id) = state_transaction
        .world
        .vpn_active_lease_by_address_slot
        .get(&body.address_slot)
        .copied()
    {
        let record = state_transaction
            .world
            .vpn_leases
            .get(&existing_lease_id)
            .cloned()
            .ok_or_else(|| validation_err("vpn active-address index is stale"))?;
        refund_active_vpn_lease(record, authority, now_ms, state_transaction)?;
    }
    Ok(())
}
impl Execute for OpenVpnLeaseEscrow {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let quote = self.quote;
        let body = &quote.body;
        let custody = verify_operator_quote(&quote, authority, state_transaction)?;
        if state_transaction
            .world
            .vpn_leases
            .get(&body.lease_id)
            .is_some()
        {
            return Err(validation_err("vpn lease already exists"));
        }
        let spec = state_transaction
            .numeric_spec_for(&body.asset_definition)
            .map_err(Error::from)?;
        assert_numeric_spec_with(body.tariff.lease_fee.as_numeric(), spec)?;
        state_transaction.world.account(authority)?;
        state_transaction.world.account(&body.operator_account_id)?;
        state_transaction
            .world
            .asset_definition(&body.asset_definition)?;
        let now_ms = state_transaction.block_unix_timestamp_ms();
        release_expired_vpn_claims_for_quote(body, authority, now_ms, state_transaction)?;
        if state_transaction
            .world
            .vpn_active_lease_by_account
            .get(&body.client_account_id)
            .is_some()
        {
            return Err(validation_err(
                "vpn client account already has an active lease",
            ));
        }
        if state_transaction
            .world
            .vpn_active_lease_by_address_slot
            .get(&body.address_slot)
            .is_some()
        {
            return Err(validation_err("vpn address slot is already claimed"));
        }
        let open_tx_hash = state_transaction
            .current_tx_hash
            .as_ref()
            .map(hash_to_bytes)
            .ok_or_else(|| validation_err("vpn lease opening requires a transaction hash"))?;
        let client_asset = AssetId::new(body.asset_definition.clone(), authority.clone());
        let custody_asset = AssetId::new(body.asset_definition.clone(), custody.clone());
        let custody_created = ensure_custody_account(&custody, state_transaction)?;
        let transfer_result = transfer_numeric_asset_for_vpn(
            state_transaction,
            VerifiedVpnNumericBatch::new(
                VerifiedVpnNumericPurpose::Funding {
                    lease_id: body.lease_id,
                    authority: authority.clone(),
                },
                vec![(client_asset, custody_asset, body.tariff.lease_fee.clone())],
            ),
        );
        if transfer_result.is_err() && custody_created {
            state_transaction.world.accounts.remove(custody.clone());
        }
        transfer_result?;
        let record = VpnLeaseRecordV1 {
            lease_id: body.lease_id,
            session_id: body.session_id,
            quote_id: body.quote_id,
            client_account_id: authority.clone(),
            operator_account_id: body.operator_account_id.clone(),
            metering_public_key: body.metering_public_key.clone(),
            asset_definition: body.asset_definition.clone(),
            lease_fee: body.tariff.lease_fee.clone(),
            custody_account_id: custody,
            relay_id: body.policy.relay_id,
            tariff: body.tariff.clone(),
            quote_policy: body.policy.clone(),
            address_slot: body.address_slot,
            signed_quote: quote.clone(),
            open_tx_hash,
            status: VpnLeaseStatusV1::Active,
            opened_at_ms: now_ms,
            expires_at_ms: body.expires_at_ms,
            settlement_grace_ms: body.settlement_grace_ms,
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
            .put_vpn_lease(record)
            .map_err(validation_err)?;
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
        if now_ms >= record.refund_available_at_ms() {
            return Err(validation_err("vpn lease settlement grace window expired"));
        }
        if self.relay_receipt.ended_at_ms > now_ms || self.client_voucher.body.issued_at_ms > now_ms
        {
            return Err(validation_err(
                "vpn settlement receipt and voucher must not be dated in the future",
            ));
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
            .put_vpn_lease(record)
            .map_err(validation_err)?;
        Ok(())
    }
}
impl Execute for RefundExpiredVpnLease {
    fn execute(
        self,
        authority: &AccountId,
        state_transaction: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        let Some(record) = state_transaction
            .world
            .vpn_leases
            .get(&self.lease_id)
            .cloned()
        else {
            return Err(validation_err("vpn lease not found"));
        };
        let now_ms = state_transaction.block_unix_timestamp_ms();
        refund_active_vpn_lease(record, authority, now_ms, state_transaction)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_config::parameters::actual::LaneConfig;
    use iroha_primitives::numeric::Numeric;
    fn nano_quantity(nanos: u64) -> Quantity {
        Quantity::from_canonical_numeric(Numeric::new(u128::from(nanos), 9))
            .expect("test nano-XOR value is a valid quantity")
    }
    fn checked_keypair() -> KeyPair {
        KeyPair::try_random().expect("VPN fixture key generation should succeed")
    }
    fn vpn_test_state() -> crate::state::State {
        crate::state::State::new_for_testing(
            crate::state::World::new(),
            crate::kura::Kura::blank_kura_for_testing(),
            crate::query::store::LiveQueryStore::start_test(),
        )
    }
    fn vpn_test_block_header() -> iroha_data_model::block::BlockHeader {
        iroha_data_model::block::BlockHeader::new(
            std::num::NonZeroU64::new(1).expect("VPN test block height is non-zero"),
            None,
            None,
            None,
            0,
            0,
        )
    }
    fn vpn_test_network_id(seed: u8) -> iroha_data_model::NetworkId {
        iroha_data_model::NetworkId::from_genesis_hash(
            HashOf::<iroha_data_model::block::BlockHeader>::from_untyped_unchecked(
                Hash::prehashed([seed; Hash::LENGTH]),
            ),
        )
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
        let network_id = vpn_test_network_id(0x21);
        let asset_definition = xor_asset_definition_id();
        let lease_id = [0x11; 32];
        let first = vpn_lease_custody_account_id(&network_id, &lease_id, &asset_definition)
            .expect("custody account derivation succeeds");
        let second = vpn_lease_custody_account_id(&network_id, &lease_id, &asset_definition)
            .expect("custody account derivation is repeatable");
        assert_eq!(first, second);
        let mut different_lease_id = lease_id;
        different_lease_id[0] ^= 0x01;
        let different =
            vpn_lease_custody_account_id(&network_id, &different_lease_id, &asset_definition)
                .expect("different custody account derivation succeeds");
        assert_ne!(first, different);
        let different_network =
            vpn_lease_custody_account_id(&vpn_test_network_id(0x22), &lease_id, &asset_definition)
                .expect("different exact-network custody derivation succeeds");
        assert_ne!(first, different_network);
        let mut public_seed_material = Vec::new();
        public_seed_material.extend_from_slice(VPN_LEASE_CUSTODY_ACCOUNT_DOMAIN.as_bytes());
        public_seed_material.extend_from_slice(network_id.as_bytes());
        public_seed_material.extend_from_slice(&lease_id);
        public_seed_material.extend_from_slice(asset_definition.to_string().as_bytes());
        let public_seed: [u8; Hash::LENGTH] = Hash::new(public_seed_material).into();
        let public_seed_keypair = KeyPair::try_from_seed(public_seed.to_vec(), Algorithm::Ed25519)
            .expect("public seed derives");
        assert_ne!(
            first,
            AccountId::new(public_seed_keypair.public_key().clone()),
            "VPN custody must not expose a signing key through public seed derivation"
        );
    }
    fn settlement_record_and_voucher(
        mut voucher_body: VpnUsageVoucherBodyV1,
    ) -> (VpnLeaseRecordV1, VpnSessionReceiptV1, VpnUsageVoucherV1) {
        let key_pair = checked_keypair();
        let client_account_id = AccountId::new(key_pair.public_key().clone());
        let operator_key = checked_keypair();
        let operator_account_id = AccountId::new(operator_key.public_key().clone());
        let network_id = vpn_test_network_id(0x31);
        let address_slot = iroha_data_model::soranet::vpn::VpnAddressSlotV1::new(23)
            .expect("fixture address slot");
        let lease_id = iroha_data_model::soranet::vpn::derive_vpn_lease_id_v1(
            &network_id,
            voucher_body.quote_id,
            &client_account_id,
        );
        voucher_body.session_id = iroha_data_model::soranet::vpn::derive_vpn_session_id_v1(
            &network_id,
            voucher_body.quote_id,
            &client_account_id,
            address_slot,
        );
        voucher_body.sequence = 9;
        let voucher = VpnUsageVoucherV1::try_sign(voucher_body, key_pair.private_key())
            .expect("vpn usage voucher fixture should sign");
        let tariff = iroha_data_model::soranet::vpn::VpnTariffV1 {
            lease_fee: nano_quantity(1_000),
            active_fee_per_minute: nano_quantity(60),
            ingress_fee_per_mib: nano_quantity(100),
            egress_fee_per_mib: nano_quantity(200),
        };
        let asset_definition = xor_asset_definition_id();
        let custody_account_id =
            vpn_lease_custody_account_id(&network_id, &lease_id, &asset_definition)
                .expect("derive fixture custody");
        let address_plan = iroha_data_model::soranet::vpn::derive_vpn_address_plan_v1(address_slot);
        let quote_policy = iroha_data_model::soranet::vpn::VpnQuotePolicyV1 {
            exit_class: iroha_data_model::soranet::vpn::VpnExitClassV1::Standard,
            relay_endpoint: "/dns/relay.example/udp/9443/quic".to_owned(),
            relay_id: voucher.body.relay_id,
            descriptor_commit: [0x22; 32],
            tls_server_name: "relay.example".to_owned(),
            relay_tls_spki_sha256: [0xAB; 32],
            relay_certificate_sha256: [0x33; 32],
            directory_snapshot_digest: [0x66; 32],
            relay_trust_valid_until_ms: 10_000,
            lease_secs: 9,
            meter_family: "soranet.vpn.standard".to_owned(),
            fee_asset_id: asset_definition.to_string(),
            escrow_account_id: custody_account_id.clone(),
            route_pushes: vec!["0.0.0.0/0".to_owned()],
            excluded_routes: Vec::new(),
            dns_servers: vec!["1.1.1.1".to_owned()],
            tunnel_addresses: address_plan.client_tunnel_addresses,
            mtu_bytes: u64::from(iroha_data_model::soranet::vpn::VPN_DEFAULT_TUNNEL_MTU_BYTES),
            flow_label_bits: 24,
            padding_budget_ms: 15,
        };
        let signed_quote = VpnSignedQuoteV1::try_sign(
            VpnQuoteBodyV1 {
                network_id,
                quote_id: voucher.body.quote_id,
                lease_id,
                session_id: voucher.body.session_id,
                address_slot,
                client_account_id: client_account_id.clone(),
                operator_account_id: operator_account_id.clone(),
                metering_public_key: key_pair.public_key().clone(),
                asset_definition: asset_definition.clone(),
                tariff: tariff.clone(),
                policy: quote_policy.clone(),
                valid_after_ms: 1_000,
                expires_at_ms: 10_000,
                settlement_grace_ms: 1_000,
            },
            operator_key.private_key(),
        )
        .expect("sign fixture VPN quote");
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
            lease_id,
            session_id: voucher.body.session_id,
            quote_id: voucher.body.quote_id,
            client_account_id,
            operator_account_id,
            metering_public_key: key_pair.public_key().clone(),
            asset_definition,
            lease_fee: tariff.lease_fee.clone(),
            custody_account_id,
            relay_id: voucher.body.relay_id,
            tariff,
            quote_policy,
            address_slot,
            signed_quote,
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
    #[test]
    fn canonical_quote_policy_rejects_unsigned_address_and_economic_projections() {
        let body = VpnUsageVoucherBodyV1 {
            session_id: [0_u8; 16],
            quote_id: [0x81; 32],
            relay_id: [0x33; 32],
            sequence: 0,
            ingress_bytes: 0,
            egress_bytes: 0,
            active_ms: 0,
            issued_at_ms: 2_000,
        };
        let (record, _, _) = settlement_record_and_voucher(body);
        ensure_canonical_quote_policy(&record.signed_quote.body, &record.custody_account_id)
            .expect("exact signed quote policy is canonical");
        let mut wrong_address = record.signed_quote.body.clone();
        wrong_address.policy.tunnel_addresses = vec!["10.0.0.2/30".to_owned()];
        assert!(ensure_canonical_quote_policy(&wrong_address, &record.custody_account_id).is_err());
        let mut wrong_fee_asset = record.signed_quote.body.clone();
        wrong_fee_asset.policy.fee_asset_id = "other#universal.universal".to_owned();
        assert!(
            ensure_canonical_quote_policy(&wrong_fee_asset, &record.custody_account_id).is_err()
        );
        let mut missing_trust = record.signed_quote.body.clone();
        missing_trust.policy.directory_snapshot_digest = [0_u8; 32];
        assert!(ensure_canonical_quote_policy(&missing_trust, &record.custody_account_id).is_err());
        let mut oversized_endpoint = record.signed_quote.body.clone();
        oversized_endpoint.policy.relay_endpoint = "x".repeat(VPN_MAX_RELAY_ENDPOINT_BYTES_V1 + 1);
        assert!(
            ensure_canonical_quote_policy(&oversized_endpoint, &record.custody_account_id).is_err()
        );
        let mut too_many_routes = record.signed_quote.body.clone();
        too_many_routes.policy.route_pushes = (0..=VPN_MAX_ROUTE_ENTRIES_V1)
            .map(|index| format!("10.0.{index}.0/24"))
            .collect();
        assert!(
            ensure_canonical_quote_policy(&too_many_routes, &record.custody_account_id).is_err()
        );
        let mut duplicate_route = record.signed_quote.body.clone();
        duplicate_route.policy.route_pushes = vec!["0.0.0.0/0".to_owned(); 2];
        assert!(
            ensure_canonical_quote_policy(&duplicate_route, &record.custody_account_id).is_err()
        );
        let mut zero_padding = record.signed_quote.body.clone();
        zero_padding.policy.padding_budget_ms = 0;
        assert!(ensure_canonical_quote_policy(&zero_padding, &record.custody_account_id).is_err());
    }
    #[test]
    fn vpn_quote_operator_authority_accepts_only_leaf_permission_directly_or_via_role() {
        let body = VpnUsageVoucherBodyV1 {
            session_id: [0_u8; 16],
            quote_id: [0x82; 32],
            relay_id: [0x33; 32],
            sequence: 0,
            ingress_bytes: 0,
            egress_bytes: 0,
            active_ms: 0,
            issued_at_ms: 2_000,
        };
        let (record, _, _) = settlement_record_and_voucher(body);
        let operator = record.operator_account_id.clone();
        let state = vpn_test_state();
        let mut block = state.block(vpn_test_block_header());
        let mut transaction = block.transaction();
        let account = Account::new(operator.clone()).build(&operator);
        let (id, value) = account.into_key_value();
        transaction.world.accounts.insert(id, value);
        assert!(
            !vpn_quote_operator_is_authorized(&operator, &transaction.world)
                .expect("registered operator lookup")
        );
        let required: Permission = CanIssueSoranetVpnQuote.into();
        transaction.world.account_permissions.insert(
            operator.clone(),
            std::collections::BTreeSet::from([required.clone()]),
        );
        assert!(
            vpn_quote_operator_is_authorized(&operator, &transaction.world)
                .expect("direct issuer lookup")
        );
        transaction
            .world
            .account_permissions
            .remove(operator.clone());
        let role_id: RoleId = "soranet_vpn_quote_issuer".parse().expect("role id");
        let role = Role::new(role_id.clone(), operator.clone())
            .add_permission(required)
            .build(&operator);
        transaction.world.roles.insert(role_id.clone(), role);
        transaction.world.account_roles.insert(
            crate::role::RoleIdWithOwner::new(operator.clone(), role_id),
            (),
        );
        assert!(
            vpn_quote_operator_is_authorized(&operator, &transaction.world)
                .expect("role issuer lookup")
        );
    }
    #[test]
    fn active_address_slot_index_rejects_exact_historical_quote_collision_pair() {
        let mut quote_0009 = [0_u8; 32];
        quote_0009[31] = 0x09;
        let mut quote_0198 = [0_u8; 32];
        quote_0198[30..].copy_from_slice(&[0x01, 0x98]);
        let voucher_body = |quote_id| VpnUsageVoucherBodyV1 {
            session_id: [0_u8; 16],
            quote_id,
            relay_id: [0x33; 32],
            sequence: 0,
            ingress_bytes: 0,
            egress_bytes: 0,
            active_ms: 0,
            issued_at_ms: 2_000,
        };
        let (first, _, _) = settlement_record_and_voucher(voucher_body(quote_0009));
        let (second, _, _) = settlement_record_and_voucher(voucher_body(quote_0198));
        assert_ne!(first.client_account_id, second.client_account_id);
        assert_eq!(first.address_slot, second.address_slot);
        let world = crate::state::World::new();
        let mut block = world.block();
        let mut transaction =
            block.transaction_without_telemetry(LaneConfig::default(), /* block height */ 1);
        transaction
            .put_vpn_lease(first)
            .expect("first typed address claim is available");
        let error = transaction
            .put_vpn_lease(second)
            .expect_err("second active lease must not alias the same address slot");
        assert!(error.contains("address slot"), "unexpected error: {error}");
    }
    #[test]
    fn active_account_index_releases_only_on_terminal_transition() {
        let body = VpnUsageVoucherBodyV1 {
            session_id: [0_u8; 16],
            quote_id: [0x91; 32],
            relay_id: [0x33; 32],
            sequence: 0,
            ingress_bytes: 0,
            egress_bytes: 0,
            active_ms: 0,
            issued_at_ms: 2_000,
        };
        let (active, _, _) = settlement_record_and_voucher(body);
        let mut terminal = active.clone();
        terminal.status = VpnLeaseStatusV1::Refunded;
        terminal.refunded_at_ms = Some(11_000);
        terminal.refunded_fee = terminal.lease_fee.clone();
        let state = vpn_test_state();
        let mut block = state.block(vpn_test_block_header());
        let mut transaction = block.transaction();
        transaction
            .world
            .put_vpn_lease(active.clone())
            .expect("active lease is indexed");
        assert!(is_active_vpn_client(
            &transaction.world,
            &active.client_account_id
        ));
        assert_eq!(
            transaction
                .world
                .vpn_active_lease_by_account
                .get(&active.client_account_id),
            Some(&active.lease_id)
        );
        transaction
            .world
            .put_vpn_lease(terminal)
            .expect("terminal transition releases active claims");
        assert!(
            transaction
                .world
                .vpn_active_lease_by_account
                .get(&active.client_account_id)
                .is_none()
        );
        assert!(
            transaction
                .world
                .vpn_active_lease_by_address_slot
                .get(&active.address_slot)
                .is_none()
        );
        assert!(!is_active_vpn_client(
            &transaction.world,
            &active.client_account_id
        ));
        let error = transaction
            .world
            .put_vpn_lease(active)
            .expect_err("terminal VPN lease must not be resurrected");
        assert!(error.contains("terminal status back to active"));
    }
    #[test]
    fn vpn_state_rejects_a_retained_quote_field_substitution() {
        let body = VpnUsageVoucherBodyV1 {
            session_id: [0_u8; 16],
            quote_id: [0xA1; 32],
            relay_id: [0x33; 32],
            sequence: 0,
            ingress_bytes: 0,
            egress_bytes: 0,
            active_ms: 0,
            issued_at_ms: 2_000,
        };
        let (mut forged, _, _) = settlement_record_and_voucher(body);
        let mut impossible_open = forged.clone();
        impossible_open.opened_at_ms = impossible_open.expires_at_ms;
        forged.signed_quote.body.policy.descriptor_commit[0] ^= 1;
        let world = crate::state::World::new();
        let mut block = world.block();
        let mut transaction =
            block.transaction_without_telemetry(LaneConfig::default(), /* block height */ 1);
        let error = transaction
            .put_vpn_lease(forged)
            .expect_err("retained quote substitution must fail before indexing");
        assert!(
            error.contains("invalid operator quote"),
            "unexpected error: {error}"
        );
        let error = transaction
            .put_vpn_lease(impossible_open)
            .expect_err("lease opening outside the signed interval must fail");
        assert!(
            error.contains("opened outside"),
            "unexpected error: {error}"
        );
    }
    #[test]
    fn vpn_state_revalidates_terminal_receipt_and_timeout_boundaries() {
        let body = VpnUsageVoucherBodyV1 {
            session_id: [0_u8; 16],
            quote_id: [0xA2; 32],
            relay_id: [0x33; 32],
            sequence: 0,
            ingress_bytes: 1_024,
            egress_bytes: 2_048,
            active_ms: 1_000,
            issued_at_ms: 2_000,
        };
        let (active, receipt, voucher) = settlement_record_and_voucher(body);
        let mut settled = active.clone();
        settled.status = VpnLeaseStatusV1::Settled;
        settled.settled_at_ms = Some(receipt.ended_at_ms);
        settled.highest_voucher_sequence = voucher.body.sequence;
        settled.client_voucher_hash = Some(voucher.hash());
        settled.relay_receipt_hash = Some(receipt.hash());
        settled.settled_relay_receipt = Some(receipt);
        settled.earned_fee = settled
            .settled_relay_receipt
            .as_ref()
            .expect("settled fixture receipt")
            .earned_fee
            .clone();
        settled.refunded_fee = settled
            .lease_fee
            .checked_sub(&settled.earned_fee)
            .expect("fixture fee conservation");
        let insert_fresh = |record| {
            let world = crate::state::World::new();
            let mut block = world.block();
            let mut transaction = block
                .transaction_without_telemetry(LaneConfig::default(), /* block height */ 1);
            transaction.put_vpn_lease(record).map(|_| ())
        };
        insert_fresh(settled.clone()).expect("canonical settled lease must rebuild");
        let mut wrong_account = settled.clone();
        let wrong_account_receipt_hash = {
            let wrong_account_receipt = wrong_account
                .settled_relay_receipt
                .as_mut()
                .expect("settled fixture receipt");
            wrong_account_receipt.account_hash[0] ^= 1;
            wrong_account_receipt.hash()
        };
        wrong_account.relay_receipt_hash = Some(wrong_account_receipt_hash);
        assert!(
            insert_fresh(wrong_account)
                .expect_err("receipt account substitution must fail")
                .contains("retained receipt")
        );
        let mut wrong_class = settled.clone();
        let wrong_class_receipt_hash = {
            let wrong_class_receipt = wrong_class
                .settled_relay_receipt
                .as_mut()
                .expect("settled fixture receipt");
            wrong_class_receipt.exit_class =
                iroha_data_model::soranet::vpn::VpnExitClassV1::HighSecurity;
            wrong_class_receipt.hash()
        };
        wrong_class.relay_receipt_hash = Some(wrong_class_receipt_hash);
        assert!(
            insert_fresh(wrong_class)
                .expect_err("receipt class substitution must fail")
                .contains("retained receipt")
        );
        let mut late_settlement = settled;
        late_settlement.settled_at_ms = Some(late_settlement.refund_available_at_ms());
        assert!(
            insert_fresh(late_settlement)
                .expect_err("settlement at the refund boundary must fail")
                .contains("retained receipt")
        );
        let mut early_refund = active;
        early_refund.status = VpnLeaseStatusV1::Refunded;
        early_refund.refunded_at_ms = Some(early_refund.refund_available_at_ms() - 1);
        early_refund.refunded_fee = early_refund.lease_fee.clone();
        assert!(
            insert_fresh(early_refund)
                .expect_err("refund before the settlement window closes must fail")
                .contains("terminal state")
        );
    }
}
