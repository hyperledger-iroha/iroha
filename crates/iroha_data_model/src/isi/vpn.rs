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
        /// Durable quote policy used to rebuild Torii VPN responses from WSV.
        pub quote_policy: crate::soranet::vpn::VpnQuotePolicyV1,
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
        quote_policy: crate::soranet::vpn::VpnQuotePolicyV1,
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
            quote_policy,
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

fn vpn_decode_flags() -> u8 {
    norito::core::effective_decode_flags().unwrap_or_else(norito::core::default_encode_flags)
}

impl<'a> norito::core::DecodeFromSlice<'a> for OpenVpnLeaseEscrow {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = vpn_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let lease_id = super::decode_aos_canonical_field::<[u8; 32]>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let session_id = super::decode_aos_canonical_field::<[u8; 16]>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let quote_id = super::decode_aos_canonical_field::<[u8; 32]>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let relay_id = super::decode_aos_canonical_field::<crate::soranet::RelayId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let operator_account_id = super::decode_aos_canonical_field::<crate::account::AccountId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let metering_public_key = super::decode_aos_canonical_field::<iroha_crypto::PublicKey>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let asset_definition = super::decode_aos_canonical_field::<crate::asset::AssetDefinitionId>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let lease_fee = super::decode_aos_canonical_field::<iroha_primitives::numeric::Numeric>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let tariff = super::decode_aos_canonical_field::<crate::soranet::vpn::VpnTariffV1>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let quote_policy = super::decode_aos_canonical_field::<
            crate::soranet::vpn::VpnQuotePolicyV1,
        >(super::read_aos_field(bytes, &mut offset, flags)?, flags)?;
        let expires_at_ms = super::decode_aos_canonical_field::<u64>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let settlement_grace_ms = super::decode_aos_canonical_field::<u64>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
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
                quote_policy,
                expires_at_ms,
                settlement_grace_ms,
            },
            offset,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for SettleVpnLease {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = vpn_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let lease_id = super::decode_aos_canonical_field::<[u8; 32]>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        let relay_receipt = super::decode_aos_canonical_field::<
            crate::soranet::vpn::VpnSessionReceiptV1,
        >(super::read_aos_field(bytes, &mut offset, flags)?, flags)?;
        let client_voucher = super::decode_aos_canonical_field::<
            crate::soranet::vpn::VpnUsageVoucherV1,
        >(super::read_aos_field(bytes, &mut offset, flags)?, flags)?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((
            Self {
                lease_id,
                relay_receipt,
                client_voucher,
            },
            offset,
        ))
    }
}

impl<'a> norito::core::DecodeFromSlice<'a> for RefundExpiredVpnLease {
    fn decode_from_slice(bytes: &'a [u8]) -> Result<(Self, usize), norito::core::Error> {
        let flags = vpn_decode_flags();
        if flags & norito::core::header_flags::PACKED_STRUCT != 0 {
            return super::decode_packed_instruction_payload::<Self>(bytes);
        }

        let mut offset = 0usize;
        let lease_id = super::decode_aos_canonical_field::<[u8; 32]>(
            super::read_aos_field(bytes, &mut offset, flags)?,
            flags,
        )?;
        if offset != bytes.len() {
            return Err(norito::core::Error::LengthMismatch);
        }
        norito::core::note_payload_access(bytes, offset);
        Ok((Self { lease_id }, offset))
    }
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use norito::{codec::Encode as _, core::DecodeFromSlice};

    use super::*;
    use crate::{
        account::AccountId,
        asset::AssetDefinitionId,
        domain::DomainId,
        soranet::vpn::{
            VpnExitClassV1, VpnQuotePolicyV1, VpnSessionReceiptV1, VpnTariffV1,
            VpnUsageVoucherBodyV1, VpnUsageVoucherV1,
        },
    };

    fn account(seed: u8) -> AccountId {
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        AccountId::new(key_pair.public_key().clone())
    }

    fn public_key(seed: u8) -> iroha_crypto::PublicKey {
        let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
        key_pair.public_key().clone()
    }

    fn asset_definition() -> AssetDefinitionId {
        AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("domain id"),
            "xor".parse().expect("asset name"),
        )
    }

    fn tariff() -> VpnTariffV1 {
        VpnTariffV1 {
            lease_fee_nanos: 10_000,
            active_fee_nanos_per_minute: 60,
            ingress_fee_nanos_per_mib: 100,
            egress_fee_nanos_per_mib: 200,
        }
    }

    fn quote_policy(account_id: &AccountId) -> VpnQuotePolicyV1 {
        VpnQuotePolicyV1 {
            exit_class: VpnExitClassV1::Standard,
            relay_endpoint: "/dns/relay.example/udp/9443/quic".to_owned(),
            lease_secs: 600,
            meter_family: "soranet.vpn.standard".to_owned(),
            fee_asset_id: "xor#universal.universal".to_owned(),
            escrow_account_id: account_id.clone(),
            route_pushes: vec!["0.0.0.0/0".to_owned()],
            excluded_routes: Vec::new(),
            dns_servers: vec!["1.1.1.1".to_owned()],
            tunnel_addresses: vec!["10.208.0.2/32".to_owned()],
            mtu_bytes: 1_280,
            flow_label_bits: 24,
            padding_budget_ms: 15,
            relay_tls_spki_sha256_hex: Some("ab".repeat(32)),
        }
    }

    fn usage_voucher() -> VpnUsageVoucherV1 {
        let key_pair = KeyPair::from_seed(vec![0x43; 32], Algorithm::Ed25519);
        let body = VpnUsageVoucherBodyV1 {
            session_id: [0x11; 16],
            quote_id: [0x22; 32],
            relay_id: [0x33; 32],
            sequence: 7,
            ingress_bytes: 128,
            egress_bytes: 256,
            active_ms: 1_500,
            issued_at_ms: 1_700_000_000_000,
        };
        let signature = Signature::new(key_pair.private_key(), &body.encode());
        VpnUsageVoucherV1 {
            body,
            client_public_key: key_pair.public_key().clone(),
            signature,
        }
    }

    fn session_receipt(voucher: &VpnUsageVoucherV1) -> VpnSessionReceiptV1 {
        VpnSessionReceiptV1 {
            session_id: [0x11; 16],
            quote_id: [0x22; 32],
            payment_tx_hash: [0x44; 32],
            account_hash: [0x55; 32],
            relay_id: [0x33; 32],
            ingress_bytes: 128,
            egress_bytes: 256,
            cover_bytes: 64,
            uptime_secs: 90,
            started_at_ms: 1_700_000_000_000,
            ended_at_ms: 1_700_000_090_000,
            exit_class: VpnExitClassV1::Standard,
            meter_hash: [0x66; 32],
            earned_fee_nanos: tariff().earned_fee_nanos(&voucher.body),
            highest_voucher_sequence: voucher.body.sequence,
            client_voucher_hash: voucher.hash(),
        }
    }

    fn assert_slice_roundtrip<T>(value: T)
    where
        T: Clone + PartialEq + core::fmt::Debug + norito::codec::Encode,
        for<'a> T: DecodeFromSlice<'a>,
    {
        let bytes = value.encode();
        let (decoded, used) = T::decode_from_slice(&bytes).expect("decode from slice");
        assert_eq!(used, bytes.len());
        assert_eq!(decoded, value);
    }

    fn assert_registry_decodes<T>(registry: &crate::isi::InstructionRegistry, value: T)
    where
        T: crate::isi::Instruction
            + norito::codec::Encode
            + 'static
            + norito::core::NoritoSerialize,
        for<'de> T: norito::core::NoritoDeserialize<'de>,
    {
        let wire_id = std::any::type_name::<T>();
        let (payload, flags) = norito::codec::encode_with_header_flags(&value);
        let framed =
            norito::core::frame_bare_with_header_flags::<T>(&payload, flags).expect("frame");
        let decoded = crate::isi::InstructionRegistry::decode(registry, wire_id, &framed)
            .expect("registered")
            .expect("decode");
        assert_eq!(crate::isi::Instruction::dyn_encode(&*decoded), payload);
    }

    #[test]
    fn vpn_decode_from_slice_roundtrips() {
        let voucher = usage_voucher();
        let escrow_account = account(0x42);
        assert_slice_roundtrip(OpenVpnLeaseEscrow::new(
            [0xAA; 32],
            [0x11; 16],
            [0x22; 32],
            [0x33; 32],
            escrow_account.clone(),
            public_key(0x43),
            asset_definition(),
            tariff().lease_fee_numeric(),
            tariff(),
            quote_policy(&escrow_account),
            1_700_000_600_000,
            60_000,
        ));
        assert_slice_roundtrip(SettleVpnLease::new(
            [0xAA; 32],
            session_receipt(&voucher),
            voucher,
        ));
        assert_slice_roundtrip(RefundExpiredVpnLease::new([0xAA; 32]));
    }

    #[test]
    fn vpn_registry_decodes_type_names() {
        let voucher = usage_voucher();
        let escrow_account = account(0x42);
        let registry = crate::isi::InstructionRegistry::new()
            .register_slice::<OpenVpnLeaseEscrow>()
            .register_slice::<SettleVpnLease>()
            .register_slice::<RefundExpiredVpnLease>();

        assert_registry_decodes(
            &registry,
            OpenVpnLeaseEscrow::new(
                [0xAA; 32],
                [0x11; 16],
                [0x22; 32],
                [0x33; 32],
                escrow_account.clone(),
                public_key(0x43),
                asset_definition(),
                tariff().lease_fee_numeric(),
                tariff(),
                quote_policy(&escrow_account),
                1_700_000_600_000,
                60_000,
            ),
        );
        assert_registry_decodes(
            &registry,
            SettleVpnLease::new([0xAA; 32], session_receipt(&voucher), voucher),
        );
        assert_registry_decodes(&registry, RefundExpiredVpnLease::new([0xAA; 32]));
    }
}
