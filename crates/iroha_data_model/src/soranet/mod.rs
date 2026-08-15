//! `SoraNet`-specific data model extensions.
//!
//! This module hosts forward-looking types for privacy tickets, relay
//! incentives, and additional transport metadata surfaced by the `SoraNet`
//! anonymity layer. The initial implementation focused on zero-knowledge
//! privacy tickets so SoraFS streaming requests can remain anonymous while
//! remaining auditable by relays and gateways. The incentive scaffolding
//! introduced in SNNet-7 models relay bonding, bandwidth attestations, and
//! reward instructions so the treasury can remunerate SoraNet relays with
//! deterministic Norito payloads.
#![allow(clippy::module_name_repetitions)]
use iroha_crypto::{Algorithm, PublicKey, Signature};
/// Canonical 32-byte digest type used across `SoraNet` payloads.
pub type Digest32 = [u8; 32];
/// Relay identifier derived from the directory fingerprint.
pub type RelayId = Digest32;
pub(crate) fn signature_for_public_key_algorithm(
    public_key: &PublicKey,
    signature: &Signature,
) -> Result<Signature, iroha_crypto::Error> {
    let algorithm = public_key
        .try_algorithm()
        .map_err(|_| iroha_crypto::Error::BadSignature)?;
    match algorithm {
        Algorithm::Ed25519 => iroha_crypto::ed25519_parse_signature(signature.payload())
            .map_err(|_| iroha_crypto::Error::BadSignature),
        Algorithm::MlDsa => iroha_crypto::mldsa65_parse_signature(signature.payload())
            .map_err(|_| iroha_crypto::Error::BadSignature),
        _ => Signature::try_from_bytes(signature.payload())
            .map_err(|_| iroha_crypto::Error::BadSignature),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::KeyPair;
    #[test]
    fn signature_for_public_key_algorithm_rejects_malformed_mldsa_signature_lengths() {
        let key_pair = KeyPair::try_random_with_algorithm(Algorithm::MlDsa)
            .expect("generate checked SoraNet ML-DSA fixture keypair");
        let signature = Signature::try_new(key_pair.private_key(), b"soranet mldsa")
            .expect("sign checked SoraNet ML-DSA fixture");
        signature_for_public_key_algorithm(key_pair.public_key(), &signature)
            .expect("valid SoraNet ML-DSA signature parses");
        let valid_signature = signature.payload().to_vec();
        for (label, replacement_signature) in [
            (
                "short",
                valid_signature[..valid_signature.len() - 1].to_vec(),
            ),
            ("overlong", {
                let mut payload = valid_signature.clone();
                payload.push(0x5F);
                payload
            }),
        ] {
            let signature = Signature::from_bytes(&replacement_signature);
            assert_eq!(
                signature_for_public_key_algorithm(key_pair.public_key(), &signature),
                Err(iroha_crypto::Error::BadSignature),
                "{label} SoraNet ML-DSA signature length was not rejected"
            );
        }
    }
}
/// Incentive and payout scaffolding for SoraNet relays.
pub mod incentives;
/// Aggregated privacy-preserving telemetry buckets and summaries.
pub mod privacy_metrics;
/// Privacy ticket payloads and envelopes.
pub mod ticket;
/// VPN cell/control-plane payloads for the native SoraNet tunnel.
pub mod vpn;
/// Re-export commonly used `SoraNet` types.
pub mod prelude {
    pub use super::{
        Digest32, RelayId,
        incentives::{
            BandwidthConfidenceV1, RelayBandwidthProofPayloadV1, RelayBandwidthProofSignatureError,
            RelayBandwidthProofV1, RelayBondLedgerEntryV1, RelayBondPolicyV1,
            RelayComplianceStatusV1, RelayEpochMetricsV1, RelayRewardInstructionV1,
        },
        privacy_metrics::{
            SoranetGarAbuseCountV1, SoranetGarAbuseShareV1, SoranetLatencyPercentileV1,
            SoranetPrivacyBucketMetricsV1, SoranetPrivacyEventActiveSampleV1,
            SoranetPrivacyEventGarAbuseCategoryV1, SoranetPrivacyEventHandshakeFailureV1,
            SoranetPrivacyEventHandshakeSuccessV1, SoranetPrivacyEventKindV1,
            SoranetPrivacyEventThrottleV1, SoranetPrivacyEventV1,
            SoranetPrivacyEventVerifiedBytesV1, SoranetPrivacyHandshakeFailureV1,
            SoranetPrivacyModeV1, SoranetPrivacyPrioShareV1, SoranetPrivacyThrottleScopeV1,
        },
        ticket::{TicketBodyV1, TicketEnvelopeV1, TicketScopeV1},
        vpn::{
            VPN_CELL_LEN, VpnCellClassV1, VpnCellError, VpnCellFlagsV1, VpnCellHeaderV1, VpnCellV1,
            VpnControlPlaneV1, VpnCoverPlanEntryV1, VpnCoverScheduleV1, VpnExitClassV1,
            VpnFlowLabelV1, VpnLeaseRecordV1, VpnLeaseStatusV1, VpnPaddedCellV1, VpnQuotePolicyV1,
            VpnRouteV1, VpnSessionReceiptV1, VpnTariffV1, VpnUsageVoucherBodyV1, VpnUsageVoucherV1,
        },
    };
}
