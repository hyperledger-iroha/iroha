//! Retired node-configured terms of a signed genesis that predates genesis-authored policy.
//!
//! Before genesis authored its ordinary crypto admission and staking monetary policy, nodes
//! read them from `[crypto]` and `[nexus.staking]`, and the genesis execution-policy and
//! Nexus/AMX commitments bound those configured values. A network started that way keeps them
//! in its node configuration. Setting `crypto.allowed_signing` selects that legacy genesis
//! profile; a genesis that signs its own policies refuses it.
use std::{borrow::Cow, num::NonZeroU32, str::FromStr as _, time::Duration};

use error_stack::Report;
use iroha_config_base::{
    ReadConfig,
    env::FromEnvStr,
    util::{DurationMs, Emitter},
};
use iroha_crypto::Algorithm;
use iroha_data_model::{
    account::{AccountId, curve::CurveId},
    asset::AssetDefinitionId,
    block::consensus::PublicLaneStakingPolicy,
    crypto_admission_policy::CryptoAdmissionPolicyV1,
};
use iroha_primitives::numeric::Quantity;
use nonzero_ext::nonzero;
use norito::json;

use super::{AlgorithmListConfig, Crypto, NexusStaking, ParseError};
use crate::parameters::actual;

// The retired V1 `[nexus.staking]` defaults that a legacy profile may omit.
const MIN_VALIDATOR_STAKE: u64 = 1;
const MAX_STAKE_SHARES_PER_VALIDATOR: NonZeroU32 = nonzero!(256_u32);
const MAX_PENDING_UNBONDS_PER_SHARE: NonZeroU32 = nonzero!(8_u32);
const MAX_SLASH_BPS: u16 = 10_000;

/// Wrapper around a list of curve identifiers allowing env parsing.
#[derive(Debug, Clone, Default)]
pub struct CurveIdListConfig(pub Vec<u8>);
impl json::JsonDeserialize for CurveIdListConfig {
    fn json_deserialize(
        parser: &mut json::Parser<'_>,
    ) -> ::core::result::Result<Self, json::Error> {
        Vec::<u8>::json_deserialize(parser).map(Self)
    }
}
impl FromEnvStr for CurveIdListConfig {
    type Error = core::num::ParseIntError;
    fn from_env_str(value: Cow<'_, str>) -> std::result::Result<Self, Self::Error> {
        value
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(str::parse)
            .collect::<std::result::Result<_, _>>()
            .map(Self)
    }
}

/// Retired `[crypto.curves]` table of the legacy genesis profile (user view).
#[derive(Debug, Clone, Default, ReadConfig)]
pub struct CryptoCurves {
    /// Curve identifiers configured with `crypto.allowed_signing`; they must equal its curves.
    pub allowed_curve_ids: Option<CurveIdListConfig>,
}

/// Legacy `[crypto]` terms, taken before the ordinary crypto section is parsed.
pub(super) struct CryptoInput {
    allowed_signing: Option<AlgorithmListConfig>,
    allowed_curve_ids: Option<CurveIdListConfig>,
}

/// Legacy `[nexus.staking]` monetary terms, taken before Nexus is parsed.
pub(super) struct StakingInput {
    min_validator_stake: Option<Quantity>,
    max_stake_shares_per_validator: Option<NonZeroU32>,
    max_pending_unbonds_per_share: Option<NonZeroU32>,
    unbonding_delay_ms: Option<DurationMs>,
    max_slash_bps: Option<u16>,
    stake_asset_id: Option<String>,
    stake_escrow_account_id: Option<String>,
    slash_sink_account_id: Option<String>,
}

impl StakingInput {
    fn is_empty(&self) -> bool {
        self.min_validator_stake.is_none()
            && self.max_stake_shares_per_validator.is_none()
            && self.max_pending_unbonds_per_share.is_none()
            && self.unbonding_delay_ms.is_none()
            && self.max_slash_bps.is_none()
            && self.stake_asset_id.is_none()
            && self.stake_escrow_account_id.is_none()
            && self.slash_sink_account_id.is_none()
    }
}

impl Crypto {
    pub(super) fn take_legacy_genesis(&mut self) -> CryptoInput {
        CryptoInput {
            allowed_signing: self.allowed_signing.take(),
            allowed_curve_ids: self.curves.allowed_curve_ids.take(),
        }
    }
}

impl NexusStaking {
    pub(super) fn take_legacy_genesis(&mut self) -> StakingInput {
        StakingInput {
            min_validator_stake: self.min_validator_stake.take(),
            max_stake_shares_per_validator: self.max_stake_shares_per_validator.take(),
            max_pending_unbonds_per_share: self.max_pending_unbonds_per_share.take(),
            unbonding_delay_ms: self.unbonding_delay_ms.take(),
            max_slash_bps: self.max_slash_bps.take(),
            stake_asset_id: self.stake_asset_id.take(),
            stake_escrow_account_id: self.stake_escrow_account_id.take(),
            slash_sink_account_id: self.slash_sink_account_id.take(),
        }
    }
}

/// Validate the legacy profile; `None` when `crypto.allowed_signing` does not select it.
pub(super) fn parse(
    crypto: CryptoInput,
    staking: StakingInput,
    parsed: &actual::Crypto,
    emitter: &mut Emitter<ParseError>,
) -> Option<actual::LegacyGenesisV1> {
    let Some(allowed_signing) = crypto.allowed_signing else {
        if crypto.allowed_curve_ids.is_some() || !staking.is_empty() {
            emitter.emit(Report::new(ParseError::InvalidCryptoConfig).attach(
                "crypto.curves and the nexus.staking monetary keys belong to the legacy genesis \
                 profile, which crypto.allowed_signing selects",
            ));
        }
        return None;
    };
    let crypto = parse_crypto(&allowed_signing, crypto.allowed_curve_ids, parsed, emitter);
    let staking = parse_staking(staking, emitter);
    Some(actual::LegacyGenesisV1 {
        crypto: crypto?,
        staking: staking?,
    })
}

fn parse_crypto(
    allowed_signing: &AlgorithmListConfig,
    configured_curves: Option<CurveIdListConfig>,
    parsed: &actual::Crypto,
    emitter: &mut Emitter<ParseError>,
) -> Option<actual::LegacyGenesisCryptoV1> {
    let mut invalid = |message: String| {
        emitter.emit(Report::new(ParseError::InvalidCryptoConfig).attach(message));
    };
    let mut algorithms = Vec::new();
    for (index, value) in allowed_signing.0.iter().enumerate() {
        let Ok(algorithm) = Algorithm::from_str(value.trim()) else {
            invalid(format!(
                "crypto.allowed_signing[{index}] is not a supported algorithm: {value}"
            ));
            return None;
        };
        algorithms.push(algorithm);
    }
    // The retired parser kept one sorted set, so a repeated entry is not a second term.
    algorithms.sort_unstable();
    algorithms.dedup();
    if !algorithms.contains(&Algorithm::Ed25519) {
        invalid("crypto.allowed_signing must include ed25519".to_owned());
        return None;
    }
    if algorithms.contains(&Algorithm::Sm2) != parsed.default_hash.eq_ignore_ascii_case("sm3-256") {
        invalid(
            "crypto.allowed_signing includes sm2 exactly when crypto.default_hash is sm3-256"
                .to_owned(),
        );
        return None;
    }
    let policy = match CryptoAdmissionPolicyV1::new(algorithms.clone()) {
        Ok(policy) => policy,
        Err(error) => {
            invalid(format!(
                "crypto.allowed_signing is not a crypto policy: {error}"
            ));
            return None;
        }
    };
    let mut derived = policy
        .allowed_curves()
        .map(CurveId::as_u8)
        .collect::<Vec<_>>();
    derived.sort_unstable();
    derived.dedup();
    let mut allowed_curve_ids =
        configured_curves.map_or_else(|| derived.clone(), |curves| curves.0);
    allowed_curve_ids.sort_unstable();
    allowed_curve_ids.dedup();
    // Controller admission follows the algorithm list alone, so a narrower curve list
    // would be a restriction this node cannot enforce.
    if allowed_curve_ids != derived {
        invalid(format!(
            "crypto.curves.allowed_curve_ids must equal the curves of crypto.allowed_signing \
             ({derived:?})"
        ));
        return None;
    }
    Some(actual::LegacyGenesisCryptoV1 {
        default_hash: parsed.default_hash.clone(),
        allowed_signing: algorithms,
        sm2_distid_default: parsed.sm2_distid_default.clone(),
        allowed_curve_ids,
        policy,
    })
}

fn parse_staking(
    input: StakingInput,
    emitter: &mut Emitter<ParseError>,
) -> Option<actual::LegacyGenesisStakingV1> {
    let mut invalid = |message: String| {
        emitter.emit(Report::new(ParseError::InvalidNexusConfig).attach(message));
    };
    let (Some(stake_asset_id), Some(stake_escrow_account_id), Some(slash_sink_account_id)) = (
        input.stake_asset_id,
        input.stake_escrow_account_id,
        input.slash_sink_account_id,
    ) else {
        invalid(
            "the legacy genesis profile configures nexus.staking.stake_asset_id, \
             stake_escrow_account_id and slash_sink_account_id"
                .to_owned(),
        );
        return None;
    };
    // The retired configuration kept the trimmed asset literal and the account literals as given.
    let stake_asset_id = stake_asset_id.trim().to_owned();
    let asset = match AssetDefinitionId::parse_address_literal(&stake_asset_id) {
        Ok(asset) => asset,
        Err(error) => {
            invalid(format!(
                "invalid nexus.staking.stake_asset_id `{stake_asset_id}`: {error}"
            ));
            return None;
        }
    };
    let mut account = |key: &str, literal: &str| match AccountId::parse_encoded(literal) {
        Ok(account) => Some(account),
        Err(error) => {
            invalid(format!("invalid nexus.staking.{key} `{literal}`: {error}"));
            None
        }
    };
    let escrow = account("stake_escrow_account_id", &stake_escrow_account_id);
    let sink = account("slash_sink_account_id", &slash_sink_account_id);
    let (escrow, sink) = (escrow?, sink?);
    let min_validator_stake = input
        .min_validator_stake
        .unwrap_or_else(|| Quantity::from(MIN_VALIDATOR_STAKE));
    let max_stake_shares_per_validator = input
        .max_stake_shares_per_validator
        .unwrap_or(MAX_STAKE_SHARES_PER_VALIDATOR);
    let max_pending_unbonds_per_share = input
        .max_pending_unbonds_per_share
        .unwrap_or(MAX_PENDING_UNBONDS_PER_SHARE);
    let unbonding_delay = input
        .unbonding_delay_ms
        .map_or(Duration::ZERO, DurationMs::get);
    let max_slash_bps = input.max_slash_bps.unwrap_or(MAX_SLASH_BPS);
    let Ok(unbonding_delay_ms) = u64::try_from(unbonding_delay.as_millis()) else {
        invalid("nexus.staking.unbonding_delay_ms does not fit in u64".to_owned());
        return None;
    };
    let policy = PublicLaneStakingPolicy {
        min_validator_stake: min_validator_stake.clone(),
        max_stake_shares_per_validator,
        max_pending_unbonds_per_share,
        unbonding_delay_ms,
        max_slash_bps,
        stake_asset_id: asset,
        stake_escrow_account_id: escrow,
        slash_sink_account_id: sink,
    };
    if let Err(error) = policy.validate() {
        invalid(format!(
            "the legacy nexus.staking terms are not a staking policy: {error}"
        ));
        return None;
    }
    Some(actual::LegacyGenesisStakingV1 {
        min_validator_stake,
        max_stake_shares_per_validator,
        max_pending_unbonds_per_share,
        unbonding_delay,
        max_slash_bps,
        stake_asset_id,
        stake_escrow_account_id,
        slash_sink_account_id,
        policy,
    })
}

#[cfg(test)]
mod tests {
    use iroha_data_model::account::address::ChainDiscriminantGuard;

    use super::*;

    // The live Taira validators' retired `[crypto]` and `[nexus.staking]` terms.
    const TAIRA_CHAIN_DISCRIMINANT: u16 = 369;
    const TAIRA_STAKE_ASSET: &str = "6TEAJqbb8oEPmLncoNiMRbLEK6tw";
    const TAIRA_CUSTODY: &str = "testuﾛ1NﾆﾏﾃoヱaPｾﾀﾑQｱYｽｼHo9ﾑmｸa4Rﾖ3cnﾚｦﾋDﾈﾋWoUwFNPF2D";

    fn crypto(signing: &[&str], curves: Option<&[u8]>) -> CryptoInput {
        CryptoInput {
            allowed_signing: Some(AlgorithmListConfig(
                signing.iter().map(|name| (*name).to_owned()).collect(),
            )),
            allowed_curve_ids: curves.map(|curves| CurveIdListConfig(curves.to_vec())),
        }
    }

    fn taira_staking() -> StakingInput {
        StakingInput {
            min_validator_stake: None,
            max_stake_shares_per_validator: None,
            max_pending_unbonds_per_share: None,
            unbonding_delay_ms: Some(DurationMs(Duration::from_millis(1_814_400_000))),
            max_slash_bps: Some(10_000),
            stake_asset_id: Some(TAIRA_STAKE_ASSET.to_owned()),
            stake_escrow_account_id: Some(TAIRA_CUSTODY.to_owned()),
            slash_sink_account_id: Some(TAIRA_CUSTODY.to_owned()),
        }
    }

    fn parse_with(
        crypto: CryptoInput,
        staking: StakingInput,
    ) -> (Option<actual::LegacyGenesisV1>, bool) {
        let _scope = ChainDiscriminantGuard::enter(TAIRA_CHAIN_DISCRIMINANT);
        let mut emitter = Emitter::new();
        let parsed = parse(crypto, staking, &actual::Crypto::default(), &mut emitter);
        (parsed, emitter.into_result().is_ok())
    }

    #[test]
    fn taira_profile_selects_its_configured_crypto_and_staking_policy() {
        let (parsed, clean) = parse_with(
            crypto(&["ed25519", "secp256k1", "bls_normal"], Some(&[1, 3, 4])),
            taira_staking(),
        );
        assert!(clean);
        let legacy = parsed.expect("Taira legacy genesis profile");
        assert_eq!(
            legacy.crypto.policy.allowed_algorithms(),
            [
                Algorithm::Ed25519,
                Algorithm::Secp256k1,
                Algorithm::BlsNormal
            ]
        );
        assert_eq!(legacy.crypto.allowed_curve_ids, [1, 3, 4]);
        let staking = &legacy.staking;
        assert_eq!(staking.policy.min_validator_stake, Quantity::from(1_u64));
        assert_eq!(staking.policy.max_stake_shares_per_validator.get(), 256);
        assert_eq!(staking.policy.max_pending_unbonds_per_share.get(), 8);
        assert_eq!(staking.policy.unbonding_delay_ms, 1_814_400_000);
        assert_eq!(staking.policy.max_slash_bps, 10_000);
        assert_eq!(staking.stake_escrow_account_id, TAIRA_CUSTODY);
        assert_eq!(
            staking.policy.stake_escrow_account_id,
            staking.policy.slash_sink_account_id
        );
    }

    #[test]
    fn retired_keys_need_the_legacy_profile() {
        let (parsed, clean) = parse_with(
            CryptoInput {
                allowed_signing: None,
                allowed_curve_ids: None,
            },
            taira_staking(),
        );
        assert!(parsed.is_none());
        assert!(!clean);
        let (parsed, clean) = parse_with(
            CryptoInput {
                allowed_signing: None,
                allowed_curve_ids: None,
            },
            StakingInput {
                min_validator_stake: None,
                max_stake_shares_per_validator: None,
                max_pending_unbonds_per_share: None,
                unbonding_delay_ms: None,
                max_slash_bps: None,
                stake_asset_id: None,
                stake_escrow_account_id: None,
                slash_sink_account_id: None,
            },
        );
        assert!(parsed.is_none());
        assert!(clean, "an ordinary configuration selects no profile");
    }

    #[test]
    fn profile_refuses_terms_this_node_cannot_enforce() {
        for (crypto, staking) in [
            // A curve list narrower than the algorithms would restrict controllers.
            (
                crypto(&["ed25519", "secp256k1", "bls_normal"], Some(&[1, 4])),
                taira_staking(),
            ),
            (crypto(&["secp256k1"], None), taira_staking()),
            (
                crypto(&["ed25519"], None),
                StakingInput {
                    stake_asset_id: Some("xor#universal".to_owned()),
                    ..taira_staking()
                },
            ),
        ] {
            let (parsed, clean) = parse_with(crypto, staking);
            assert!(parsed.is_none());
            assert!(!clean);
        }
    }
}
