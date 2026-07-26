#![no_main]

use arbitrary::Arbitrary;
use iroha_core::da::{
    LaneEpoch, ReplayCache, ReplayCacheConfig, ReplayFingerprint, ReplayKey,
};
use iroha_data_model::nexus::LaneId;
use libfuzzer_sys::fuzz_target;
use std::time::{Duration, Instant};

#[derive(Debug, Arbitrary)]
enum Action {
    Insert {
        sequence: u64,
        fingerprint: [u8; blake3::OUT_LEN],
        advance_micros: u32,
    },
    Clear {
        advance_micros: u32,
    },
}

#[derive(Debug, Arbitrary)]
struct Step {
    lane: u32,
    epoch: u64,
    action: Action,
}

fuzz_target!(|steps: Vec<Step>| {
    let mut now = Instant::now();
    let cache = ReplayCache::new(ReplayCacheConfig::new());

    for step in steps {
        let lane_epoch = LaneEpoch::new(LaneId::new(step.lane), step.epoch);
        match step.action {
            Action::Insert {
                sequence,
                fingerprint,
                advance_micros,
            } => {
                if let Some(next) =
                    now.checked_add(Duration::from_micros(u64::from(advance_micros) + 1))
                {
                    now = next;
                }
                let key = ReplayKey::new(
                    lane_epoch,
                    sequence,
                    ReplayFingerprint::from(fingerprint),
                );
                let _ = cache.insert(key, now);
            }
            Action::Clear { advance_micros } => {
                if let Some(next) =
                    now.checked_add(Duration::from_micros(u64::from(advance_micros) + 1))
                {
                    now = next;
                }
                cache.clear_lane_epoch(lane_epoch);
            }
        }
    }
});
