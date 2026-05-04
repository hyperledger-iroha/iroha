use super::*;

isi! {
    /// Open a ledger-managed `SoraNet` VPN lease escrow funded in XOR.
    pub struct OpenVpnLeaseEscrow {
        /// Caller-selected lease identifier.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub lease_id: [u8; 32],
        /// Session identifier bound to the tunnel runtime.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub session_id: [u8; 16],
        /// Quote identifier that fixed pricing and relay policy.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub quote_id: [u8; 32],
        /// Relay fingerprint authorized by the quote.
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub relay_id: crate::soranet::RelayId,
        /// Operator account allowed to settle this lease.
        pub operator_account_id: crate::account::AccountId,
        /// Client public key that must sign cumulative usage vouchers.
        pub metering_public_key: iroha_crypto::PublicKey,
        /// Asset definition to lock. Native VPN leases require XOR.
        pub asset_definition: crate::asset::AssetDefinitionId,
        /// Amount to lock in protocol custody.
        pub lease_fee: iroha_primitives::numeric::Numeric,
        /// Deterministic tariff used to recompute earned fees.
        pub tariff: crate::soranet::vpn::VpnTariffV1,
        /// Absolute service expiry timestamp in milliseconds since the Unix epoch.
        pub expires_at_ms: u64,
        /// Additional settlement grace window after expiry, in milliseconds.
        pub settlement_grace_ms: u64,
    }
}

impl OpenVpnLeaseEscrow {
    /// Construct a VPN lease escrow opening instruction.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        lease_id: [u8; 32],
        session_id: [u8; 16],
        quote_id: [u8; 32],
        relay_id: crate::soranet::RelayId,
        operator_account_id: crate::account::AccountId,
        metering_public_key: iroha_crypto::PublicKey,
        asset_definition: crate::asset::AssetDefinitionId,
        lease_fee: iroha_primitives::numeric::Numeric,
        tariff: crate::soranet::vpn::VpnTariffV1,
        expires_at_ms: u64,
        settlement_grace_ms: u64,
    ) -> Self {
        Self {
            lease_id,
            session_id,
            quote_id,
            relay_id,
            operator_account_id,
            metering_public_key,
            asset_definition,
            lease_fee,
            tariff,
            expires_at_ms,
            settlement_grace_ms,
        }
    }
}

isi! {
    /// Settle a `SoraNet` VPN lease with a relay receipt and client voucher.
    pub struct SettleVpnLease {
        /// Lease identifier opened by [`OpenVpnLeaseEscrow`].
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub lease_id: [u8; 32],
        /// Relay receipt describing final session counters.
        pub relay_receipt: crate::soranet::vpn::VpnSessionReceiptV1,
        /// Highest cumulative usage voucher signed by the client.
        pub client_voucher: crate::soranet::vpn::VpnUsageVoucherV1,
    }
}

impl SettleVpnLease {
    /// Construct a VPN lease settlement instruction.
    #[must_use]
    pub fn new(
        lease_id: [u8; 32],
        relay_receipt: crate::soranet::vpn::VpnSessionReceiptV1,
        client_voucher: crate::soranet::vpn::VpnUsageVoucherV1,
    ) -> Self {
        Self {
            lease_id,
            relay_receipt,
            client_voucher,
        }
    }
}

isi! {
    /// Refund an expired `SoraNet` VPN lease after the relay settlement grace window.
    pub struct RefundExpiredVpnLease {
        /// Lease identifier opened by [`OpenVpnLeaseEscrow`].
        #[cfg_attr(feature = "json", norito(with = "crate::json_helpers::fixed_bytes"))]
        pub lease_id: [u8; 32],
    }
}

impl RefundExpiredVpnLease {
    /// Construct an expired VPN lease refund instruction.
    #[must_use]
    pub const fn new(lease_id: [u8; 32]) -> Self {
        Self { lease_id }
    }
}

impl crate::seal::Instruction for OpenVpnLeaseEscrow {}
impl crate::seal::Instruction for SettleVpnLease {}
impl crate::seal::Instruction for RefundExpiredVpnLease {}
