//! Deterministic settlement router and Kagemusha reserve membership authority.
//!
//! The optional proof-release file paths are daemon bootstrap inputs, not
//! protocol values. The loaded verifier is classified separately by State and
//! must be authenticated before complete-root publication. Reserve membership
//! is retained here because account unregistration and reserve mutation read it.

use std::collections::BTreeMap;

use iroha_config::parameters::actual::Settlement;
use iroha_data_model::{account::AccountId, asset::AssetDefinitionId};
use norito::{Decode, Encode, NoritoSchema};

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, NoritoSchema)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha:state:settlement-router:v1")]
struct SettlementRouterPolicyV1 {
    twap_window_secs: u64,
    twap_window_nanos: u32,
    epsilon_bps: u16,
    buffer_alert_pct: u8,
    buffer_throttle_pct: u8,
    buffer_xor_only_pct: u8,
    buffer_halt_pct: u8,
    buffer_horizon_hours: u16,
}

/// Exact first-release State-owned settlement policy projection.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, NoritoSchema)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha:state:settlement:v1")]
pub(super) struct SettlementPolicyV1 {
    router: SettlementRouterPolicyV1,
    reserve_accounts: BTreeMap<AssetDefinitionId, AccountId>,
}

impl SettlementPolicyV1 {
    /// Capture the router inputs and current reserve membership from State.
    pub(super) fn from_actual(config: &Settlement) -> Self {
        let router = &config.router;
        Self {
            router: SettlementRouterPolicyV1 {
                twap_window_secs: router.twap_window.as_secs(),
                twap_window_nanos: router.twap_window.subsec_nanos(),
                epsilon_bps: router.epsilon_bps,
                buffer_alert_pct: router.buffer_alert_pct,
                buffer_throttle_pct: router.buffer_throttle_pct,
                buffer_xor_only_pct: router.buffer_xor_only_pct,
                buffer_halt_pct: router.buffer_halt_pct,
                buffer_horizon_hours: router.buffer_horizon_hours,
            },
            reserve_accounts: config.kagemusha.reserve_accounts.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_config::parameters::actual::KagemushaV1ProofReleaseFiles;
    use iroha_crypto::{Algorithm, KeyPair};
    use std::{path::PathBuf, time::Duration};

    fn frame(config: &Settlement) -> Vec<u8> {
        norito::encode_canonical(&SettlementPolicyV1::from_actual(config)).unwrap()
    }

    fn asset(first: u8) -> AssetDefinitionId {
        let mut bytes = [0_u8; 16];
        bytes[0] = first;
        bytes[6] = 0x40;
        bytes[8] = 0x80;
        AssetDefinitionId::from_uuid_bytes(bytes).unwrap()
    }

    fn account(seed: &[u8]) -> AccountId {
        AccountId::new(
            KeyPair::from_seed(seed.to_vec(), Algorithm::Ed25519)
                .public_key()
                .clone(),
        )
    }

    #[test]
    fn settlement_policy_has_canonical_v1_roundtrip() {
        let mut config = Settlement::default();
        config
            .kagemusha
            .reserve_accounts
            .insert(asset(1), account(b"settlement-reserve-a"));
        let projected = SettlementPolicyV1::from_actual(&config);
        assert_eq!(
            SettlementPolicyV1::nominal_name(),
            "iroha:state:settlement:v1"
        );
        let bytes = norito::encode_canonical(&projected).unwrap();
        assert_eq!(
            norito::decode_canonical::<SettlementPolicyV1>(&bytes).unwrap(),
            projected
        );
        let _ambient = norito::core::DecodeFlagsGuard::enter(0);
        assert_eq!(norito::encode_canonical(&projected).unwrap(), bytes);
    }

    #[test]
    fn every_router_input_and_reserve_membership_changes_policy() {
        let mut original = Settlement::default();
        original
            .kagemusha
            .reserve_accounts
            .insert(asset(1), account(b"settlement-reserve-a"));
        let expected = frame(&original);
        let mut changed = original.clone();
        changed.router.twap_window += Duration::from_secs(1);
        assert_ne!(frame(&changed), expected, "TWAP seconds");
        let mut changed = original.clone();
        changed.router.twap_window += Duration::from_nanos(1);
        assert_ne!(frame(&changed), expected, "TWAP nanoseconds");
        let changes: [(&str, fn(&mut iroha_config::parameters::actual::Router)); 6] = [
            ("epsilon", |router| router.epsilon_bps += 1),
            ("alert", |router| router.buffer_alert_pct += 1),
            ("throttle", |router| router.buffer_throttle_pct += 1),
            ("XOR only", |router| router.buffer_xor_only_pct += 1),
            ("halt", |router| router.buffer_halt_pct += 1),
            ("horizon", |router| router.buffer_horizon_hours += 1),
        ];
        for (name, change) in changes {
            let mut changed = original.clone();
            change(&mut changed.router);
            assert_ne!(frame(&changed), expected, "{name}");
        }
        let mut added = original.clone();
        added
            .kagemusha
            .reserve_accounts
            .insert(asset(2), account(b"settlement-reserve-b"));
        assert_ne!(frame(&added), expected, "reserve membership");
        let mut changed = original;
        changed
            .kagemusha
            .reserve_accounts
            .insert(asset(1), account(b"settlement-reserve-c"));
        assert_ne!(frame(&changed), expected, "reserve account identity");
    }

    #[test]
    fn daemon_artifact_paths_do_not_change_state_policy() {
        let baseline = Settlement::default();
        let expected = frame(&baseline);
        let mut configured = baseline;
        let file = |name| PathBuf::from(format!("/runtime/kagemusha/{name}"));
        configured.kagemusha.proof_release = Some(KagemushaV1ProofReleaseFiles {
            manifest: file("manifest"),
            validation_receipt: file("validation"),
            authority_policy: file("authority"),
            attestation: file("attestation"),
            recursive_profile: file("profile"),
            artifact_directory: file("artifacts"),
        });
        assert_eq!(frame(&configured), expected);
    }
}
