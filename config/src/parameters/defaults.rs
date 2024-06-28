//! Parameters default values

// TODO: document if needed
#![allow(missing_docs)]

use std::{
    num::{NonZeroU32, NonZeroUsize},
    time::Duration,
};

use nonzero_ext::nonzero;

pub mod queue {
    use super::*;

    pub const CAPACITY: NonZeroUsize = nonzero!(2_usize.pow(16));
    pub const CAPACITY_PER_USER: NonZeroUsize = nonzero!(2_usize.pow(16));
    // 24 hours
    pub const TRANSACTION_TIME_TO_LIVE: Duration = Duration::from_secs(24 * 60 * 60);
    pub const FUTURE_THRESHOLD: Duration = Duration::from_secs(1);
}

pub mod kura {
    pub const STORE_DIR: &str = "./storage";
}

pub mod network {
    use super::*;

    pub const TRANSACTION_GOSSIP_PERIOD: Duration = Duration::from_secs(1);
    pub const TRANSACTION_GOSSIP_SIZE: NonZeroU32 = nonzero!(500u32);

    pub const BLOCK_GOSSIP_PERIOD: Duration = Duration::from_secs(10);
    pub const BLOCK_GOSSIP_SIZE: NonZeroU32 = nonzero!(4u32);

    pub const IDLE_TIMEOUT: Duration = Duration::from_secs(60);
}

pub mod snapshot {
    use super::*;

    pub const STORE_DIR: &str = "./storage/snapshot";
    // The default frequency of making snapshots is 1 minute, need to be adjusted for larger world state view size
    pub const CREATE_EVERY: Duration = Duration::from_secs(60);
}

pub mod torii {
    use std::time::Duration;

    use iroha_config_base::util::Bytes;

    pub const MAX_CONTENT_LEN: Bytes<u64> = Bytes(2_u64.pow(20) * 16);
    pub const QUERY_IDLE_TIME: Duration = Duration::from_secs(30);
}

pub mod telemetry {
    use std::time::Duration;

    /// Default minimal retry period
    pub const MIN_RETRY_PERIOD: Duration = Duration::from_secs(1);
    /// Default maximum exponent for the retry delay
    pub const MAX_RETRY_DELAY_EXPONENT: u8 = 4;
}
