//! Compiled source-chain profiles of the running release (`specs/sccp.md` §4.13.2, §4.13.3).
//!
//! A profile holds everything about a source chain that is fixed by the chain itself rather than
//! by Taira governance: genesis identity, slot timing, the hard-fork schedule with its fork
//! versions, and `supported_until`, the last epoch this release's verifier supports. None of it
//! is stored in light-client `params`: a source-chain hard fork needs a Taira release that extends
//! the profile, and the same light client then continues with no Parliament action.
//!
//! [`policy_hash_contribution`] commits to every compiled profile and to the verifier's fixed
//! bounds, so peers running releases with different profiles cannot silently diverge once it is
//! bound into the consensus policy hash.

use iroha_data_model::bridge::SccpNetworkV1;

use crate::{
    ethereum_native::{
        EthereumLightClientError, FINALITY_PARTICIPANT_THRESHOLD, ForkActivation, ForkSchedule,
        SLOTS_PER_EPOCH, SLOTS_PER_SYNC_COMMITTEE_PERIOD,
    },
    v1::hashes::keccak256,
};

/// Ethereum mainnet beacon genesis time (seconds).
pub const ETHEREUM_MAINNET_GENESIS_TIME_S: u64 = 1_606_824_023;
/// Ethereum slot duration (seconds).
pub const ETHEREUM_SECONDS_PER_SLOT: u64 = 12;
/// Ethereum mainnet genesis validators root.
pub const ETHEREUM_MAINNET_GENESIS_VALIDATORS_ROOT: [u8; 32] = [
    0x4b, 0x36, 0x3d, 0xb9, 0x4e, 0x28, 0x61, 0x20, 0xd7, 0x6e, 0xb9, 0x05, 0x34, 0x0f, 0xdd, 0x4e,
    0x54, 0xbf, 0xe9, 0xf0, 0x6b, 0xf3, 0x3f, 0xf6, 0xcf, 0x5a, 0xd2, 0x7f, 0x51, 0x1b, 0xfe, 0x95,
];
/// Ethereum mainnet Altair-through-Fulu activation epochs and fork versions.
pub const ETHEREUM_MAINNET_FORKS: [ForkActivation; 6] = [
    ForkActivation::new(74_240, [0x01, 0, 0, 0]),
    ForkActivation::new(144_896, [0x02, 0, 0, 0]),
    ForkActivation::new(194_048, [0x03, 0, 0, 0]),
    ForkActivation::new(269_568, [0x04, 0, 0, 0]),
    ForkActivation::new(364_032, [0x05, 0, 0, 0]),
    ForkActivation::new(411_392, [0x06, 0, 0, 0]),
];
/// Last Ethereum epoch this release supports (2027-06-30T00:00:00Z).
///
/// No fork after Fulu is compiled. Each release moves this bound to the epoch before the next
/// scheduled fork, or to its own support horizon while none is scheduled; wallets stop burning
/// seven days before it (§7.2).
pub const ETHEREUM_MAINNET_SUPPORTED_UNTIL_EPOCH: u64 = 540_337;
/// EIP-2935 history storage contract.
pub const ETHEREUM_HISTORY_STORAGE_ADDRESS: [u8; 20] = [
    0x00, 0x00, 0xf9, 0x08, 0x27, 0xf1, 0xc5, 0x3a, 0x10, 0xcb, 0x7a, 0x02, 0x33, 0x5b, 0x17, 0x53,
    0x20, 0x00, 0x29, 0x35,
];
/// Keccak-256 of the EIP-2935 history storage contract code on mainnet.
pub const ETHEREUM_HISTORY_STORAGE_CODE_HASH: [u8; 32] = [
    0x6e, 0x49, 0xe6, 0x67, 0x82, 0x03, 0x7c, 0x05, 0x55, 0x89, 0x78, 0x70, 0xe2, 0x9f, 0xa5, 0xe5,
    0x52, 0xda, 0xf4, 0x71, 0x95, 0x52, 0x13, 0x1a, 0x0a, 0xbc, 0xe7, 0x79, 0xda, 0xec, 0x0a, 0x5d,
];
/// EIP-2935 `HISTORY_SERVE_WINDOW`: `1 <= E - B <= 8191` for a `HistoryContract` proof.
pub const ETHEREUM_HISTORY_SERVE_WINDOW: u64 = 8_191;
/// Hard bound on `LightClientUpdate`s in one advance (§4.13.3).
pub const ETHEREUM_MAX_UPDATES_PER_ADVANCE: usize = 16;
/// Hard bound on parent-linked headers in one `HeaderChain` ancestry.
pub const ETHEREUM_MAX_ANCESTRY_HEADERS: usize = 256;
/// Hard bound on headers in one `Backfill` segment.
pub const ETHEREUM_MAX_BACKFILL_HEADERS: usize = 256;
/// Largest amount by which a source slot may lead the Taira block time.
pub const MAX_SOURCE_FUTURE_MS: u64 = 60_000;

/// Compiled Ethereum chain profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EthereumChainProfileV1 {
    /// Beacon genesis time (seconds).
    pub genesis_time_s: u64,
    /// Slot duration (seconds).
    pub seconds_per_slot: u64,
    /// Genesis validators root bound into every signing domain.
    pub genesis_validators_root: [u8; 32],
    /// Altair-through-Fulu activations.
    pub forks: [ForkActivation; 6],
    /// Last supported epoch; anything signed or finalized later fails closed.
    pub supported_until_epoch: u64,
    /// EIP-2935 history storage contract.
    pub history_storage_address: [u8; 20],
    /// Code hash of the history storage contract.
    pub history_storage_code_hash: [u8; 32],
    /// EIP-2935 ring size.
    pub history_serve_window: u64,
}

/// Ethereum mainnet profile compiled into this release.
pub const ETHEREUM_MAINNET: EthereumChainProfileV1 = EthereumChainProfileV1 {
    genesis_time_s: ETHEREUM_MAINNET_GENESIS_TIME_S,
    seconds_per_slot: ETHEREUM_SECONDS_PER_SLOT,
    genesis_validators_root: ETHEREUM_MAINNET_GENESIS_VALIDATORS_ROOT,
    forks: ETHEREUM_MAINNET_FORKS,
    supported_until_epoch: ETHEREUM_MAINNET_SUPPORTED_UNTIL_EPOCH,
    history_storage_address: ETHEREUM_HISTORY_STORAGE_ADDRESS,
    history_storage_code_hash: ETHEREUM_HISTORY_STORAGE_CODE_HASH,
    history_serve_window: ETHEREUM_HISTORY_SERVE_WINDOW,
};

impl EthereumChainProfileV1 {
    /// The validated fork schedule.
    ///
    /// # Errors
    ///
    /// Returns the schedule error of a malformed profile (never for [`ETHEREUM_MAINNET`]).
    pub fn schedule(&self) -> Result<ForkSchedule, EthereumLightClientError> {
        ForkSchedule::new(self.genesis_validators_root, self.forks)
    }

    /// The same profile with another `supported_until` epoch (a release extending the profile).
    #[must_use]
    pub const fn with_supported_until_epoch(mut self, epoch: u64) -> Self {
        self.supported_until_epoch = epoch;
        self
    }

    /// Unix time of the start of `slot` in milliseconds, or `None` on overflow.
    #[must_use]
    pub const fn slot_start_ms(&self, slot: u64) -> Option<u64> {
        let Some(offset) = slot.checked_mul(self.seconds_per_slot) else {
            return None;
        };
        let Some(seconds) = self.genesis_time_s.checked_add(offset) else {
            return None;
        };
        seconds.checked_mul(1_000)
    }

    /// The slot in progress at unix time `unix_ms` (slot 0 before genesis).
    #[must_use]
    pub const fn slot_at_ms(&self, unix_ms: u64) -> u64 {
        let seconds = unix_ms / 1_000;
        if seconds < self.genesis_time_s || self.seconds_per_slot == 0 {
            return 0;
        }
        (seconds - self.genesis_time_s) / self.seconds_per_slot
    }

    /// First slot of a sync-committee period, or `None` on overflow.
    #[must_use]
    pub const fn period_start_slot(period: u64) -> Option<u64> {
        period.checked_mul(SLOTS_PER_SYNC_COMMITTEE_PERIOD)
    }

    /// Unix time at which `period` starts, in milliseconds.
    #[must_use]
    pub const fn period_start_ms(&self, period: u64) -> Option<u64> {
        match Self::period_start_slot(period) {
            Some(slot) => self.slot_start_ms(slot),
            None => None,
        }
    }

    /// Unix time at which the committee of `period` is superseded (the next period starts).
    #[must_use]
    pub const fn period_end_ms(&self, period: u64) -> Option<u64> {
        match period.checked_add(1) {
            Some(next) => self.period_start_ms(next),
            None => None,
        }
    }

    /// Whether `slot` lies at or before the supported epoch bound.
    #[must_use]
    pub const fn supports_slot(&self, slot: u64) -> bool {
        slot / SLOTS_PER_EPOCH <= self.supported_until_epoch
    }

    /// Unix time (ms) of the first slot beyond `supported_until`: evidence signed from then on
    /// fails closed until a release extends the profile. Wallets stop burning seven days before
    /// it (§7.2).
    #[must_use]
    pub const fn supported_until_ms(&self) -> Option<u64> {
        match self.supported_until_epoch.checked_add(1) {
            Some(epoch) => match epoch.checked_mul(SLOTS_PER_EPOCH) {
                Some(slot) => self.slot_start_ms(slot),
                None => None,
            },
            None => None,
        }
    }

    /// Unix time at which the EIP-2935 history contract became active (the Electra/Prague
    /// activation), in milliseconds.
    #[must_use]
    pub const fn history_contract_active_from_ms(&self) -> Option<u64> {
        match self.forks[4].epoch().checked_mul(SLOTS_PER_EPOCH) {
            Some(slot) => self.slot_start_ms(slot),
            None => None,
        }
    }

    /// Canonical fixed-layout bytes committed by the policy hash.
    #[must_use]
    pub fn policy_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(256);
        out.extend_from_slice(b"SCCP/LC/PROFILE/ETHEREUM/V1");
        out.extend_from_slice(&self.genesis_time_s.to_be_bytes());
        out.extend_from_slice(&self.seconds_per_slot.to_be_bytes());
        out.extend_from_slice(&self.genesis_validators_root);
        for activation in self.forks {
            out.extend_from_slice(&activation.epoch().to_be_bytes());
            out.extend_from_slice(&activation.version());
        }
        out.extend_from_slice(&self.supported_until_epoch.to_be_bytes());
        out.extend_from_slice(&self.history_storage_address);
        out.extend_from_slice(&self.history_storage_code_hash);
        out.extend_from_slice(&self.history_serve_window.to_be_bytes());
        for bound in [
            FINALITY_PARTICIPANT_THRESHOLD,
            ETHEREUM_MAX_UPDATES_PER_ADVANCE,
            ETHEREUM_MAX_ANCESTRY_HEADERS,
            ETHEREUM_MAX_BACKFILL_HEADERS,
        ] {
            out.extend_from_slice(&u64::try_from(bound).unwrap_or(u64::MAX).to_be_bytes());
        }
        out.extend_from_slice(&MAX_SOURCE_FUTURE_MS.to_be_bytes());
        out
    }
}

/// Placeholder for a source chain whose light client lands in a later workstream.
///
/// Its policy bytes name the network and mark it pending, so filling the profile changes the
/// policy hash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PendingChainProfileV1 {
    /// Source chain.
    pub network: SccpNetworkV1,
}

impl PendingChainProfileV1 {
    /// Canonical bytes committed by the policy hash.
    #[must_use]
    pub fn policy_bytes(self) -> Vec<u8> {
        let mut out = b"SCCP/LC/PROFILE/PENDING/V1".to_vec();
        out.extend_from_slice(self.network.profile_key().as_bytes());
        out
    }
}

/// Every compiled source-chain profile of the running release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SccpChainProfilesV1 {
    /// Ethereum mainnet.
    pub ethereum: EthereumChainProfileV1,
    /// BSC mainnet. TODO(ws38): compile the BSC fork schedule with `supported_until`.
    pub bsc: PendingChainProfileV1,
    /// TRON mainnet. TODO(ws39): compile the TRON profile with `supported_until`.
    pub tron: PendingChainProfileV1,
    /// TON mainnet. TODO(ws3A): compile the TON profile with `supported_until`.
    pub ton: PendingChainProfileV1,
}

/// The profiles compiled into this release.
pub const COMPILED_PROFILES: SccpChainProfilesV1 = SccpChainProfilesV1 {
    ethereum: ETHEREUM_MAINNET,
    bsc: PendingChainProfileV1 {
        network: SccpNetworkV1::BscMainnet,
    },
    tron: PendingChainProfileV1 {
        network: SccpNetworkV1::TronMainnet,
    },
    ton: PendingChainProfileV1 {
        network: SccpNetworkV1::TonMainnet,
    },
};

impl SccpChainProfilesV1 {
    /// The profiles compiled into this release.
    #[must_use]
    pub const fn compiled() -> &'static Self {
        &COMPILED_PROFILES
    }

    /// The same profiles with another Ethereum profile.
    #[must_use]
    pub const fn with_ethereum(mut self, ethereum: EthereumChainProfileV1) -> Self {
        self.ethereum = ethereum;
        self
    }

    /// Keccak-256 commitment to every profile, in network order.
    #[must_use]
    pub fn policy_hash(&self) -> [u8; 32] {
        let profiles = [
            self.ethereum.policy_bytes(),
            self.bsc.policy_bytes(),
            self.tron.policy_bytes(),
            self.ton.policy_bytes(),
        ];
        let lengths: Vec<[u8; 4]> = profiles
            .iter()
            .map(|bytes| u32::try_from(bytes.len()).unwrap_or(u32::MAX).to_be_bytes())
            .collect();
        let mut parts: Vec<&[u8]> = vec![b"SCCP/LC/PROFILES/V1"];
        for (length, bytes) in lengths.iter().zip(&profiles) {
            parts.push(length);
            parts.push(bytes);
        }
        keccak256(&parts)
    }
}

/// Keccak-256 commitment to the compiled chain profiles and verifier bounds.
///
/// Core binds it into the consensus policy hash next to the `[zk.sccp]` limits.
#[must_use]
pub fn policy_hash_contribution() -> [u8; 32] {
    SccpChainProfilesV1::compiled().policy_hash()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ethereum_native::EthereumFork;

    #[test]
    fn mainnet_schedule_matches_the_consensus_config() {
        let schedule = ETHEREUM_MAINNET.schedule().expect("valid mainnet schedule");
        assert_eq!(
            schedule.genesis_validators_root(),
            ETHEREUM_MAINNET_GENESIS_VALIDATORS_ROOT
        );
        // Fulu activated at 2025-12-03T21:49:11Z.
        assert_eq!(
            ETHEREUM_MAINNET.slot_start_ms(411_392 * 32),
            Some(1_764_798_551_000)
        );
        assert_eq!(
            schedule.fork_at_slot(411_392 * 32).map(|(fork, _)| fork),
            Ok(EthereumFork::Fulu)
        );
        assert_eq!(
            schedule
                .fork_at_slot(411_392 * 32 - 1)
                .map(|(fork, _)| fork),
            Ok(EthereumFork::Electra)
        );
        assert_eq!(
            ETHEREUM_MAINNET.history_contract_active_from_ms(),
            Some(1_746_612_311_000)
        );
    }

    #[test]
    fn slot_and_period_timing_is_exact() {
        let profile = ETHEREUM_MAINNET;
        assert_eq!(profile.slot_start_ms(0), Some(1_606_824_023_000));
        assert_eq!(profile.slot_at_ms(1_606_824_023_000), 0);
        assert_eq!(profile.slot_at_ms(1_606_824_034_999), 0);
        assert_eq!(profile.slot_at_ms(1_606_824_035_000), 1);
        assert_eq!(profile.slot_at_ms(0), 0);
        assert_eq!(EthereumChainProfileV1::period_start_slot(2), Some(16_384));
        assert_eq!(
            profile.period_end_ms(0),
            profile.slot_start_ms(8_192),
            "period 0 ends when period 1 starts"
        );
        assert_eq!(profile.period_start_ms(1), profile.period_end_ms(0));
        assert_eq!(profile.slot_start_ms(u64::MAX), None);
        assert_eq!(profile.period_end_ms(u64::MAX), None);
    }

    #[test]
    fn supported_until_is_an_inclusive_epoch_bound() {
        let profile = ETHEREUM_MAINNET.with_supported_until_epoch(10);
        assert_eq!(profile.supported_until_ms(), profile.slot_start_ms(11 * 32));
        // 2027-06-30T00:00:00Z falls inside the last supported epoch.
        let until = ETHEREUM_MAINNET.supported_until_ms().expect("time");
        assert!(until > 1_814_313_600_000 && until <= 1_814_313_600_000 + 384_000);
        assert_eq!(
            ETHEREUM_MAINNET
                .with_supported_until_epoch(u64::MAX)
                .supported_until_ms(),
            None
        );
        assert!(profile.supports_slot(10 * 32 + 31));
        assert!(!profile.supports_slot(11 * 32));
        assert!(ETHEREUM_MAINNET.supports_slot(478_462 * 32));
        assert!(!ETHEREUM_MAINNET.supports_slot((ETHEREUM_MAINNET_SUPPORTED_UNTIL_EPOCH + 1) * 32));
    }

    #[test]
    fn policy_hash_binds_every_profile_field() {
        let compiled = *SccpChainProfilesV1::compiled();
        assert_eq!(policy_hash_contribution(), compiled.policy_hash());
        let extended = compiled.with_ethereum(ETHEREUM_MAINNET.with_supported_until_epoch(600_000));
        assert_ne!(extended.policy_hash(), compiled.policy_hash());
        let mut other_root = compiled;
        other_root.ethereum.genesis_validators_root[0] ^= 1;
        assert_ne!(other_root.policy_hash(), compiled.policy_hash());
        let mut other_pending = compiled;
        other_pending.ton.network = SccpNetworkV1::BscMainnet;
        assert_ne!(other_pending.policy_hash(), compiled.policy_hash());
        assert!(
            PendingChainProfileV1 {
                network: SccpNetworkV1::TronMainnet
            }
            .policy_bytes()
            .ends_with(b"tron-mainnet")
        );
    }
}
