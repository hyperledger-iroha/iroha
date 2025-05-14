//! Functionality related to working with the configuration through client API.
//!
//! Intended usage:
//!
//! - Create [`ConfigGetDTO`] from [`base::Root`] and serialize it for the client
//! - Deserialize [`ConfigUpdateDTO`] from the client and apply the changes

use std::num::NonZero;

use iroha_crypto::PublicKey;
use iroha_data_model::Level;
use serde::{Deserialize, Serialize};

use crate::{logger::Directives, parameters::actual as base};

/// Subset of Iroha configuration to return to the clients.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(missing_docs)]
pub struct ConfigGetDTO {
    pub public_key: PublicKey,
    pub logger: Logger,
    pub network: Network,
    pub queue: Queue,
}

impl From<&'_ base::Root> for ConfigGetDTO {
    fn from(value: &'_ base::Root) -> Self {
        Self {
            public_key: value.common.key_pair.public_key().clone(),
            logger: (&value.logger).into(),
            network: value.into(),
            queue: (&value.queue).into(),
        }
    }
}

/// Subset of Iroha configuration that clients could update.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(missing_docs)]
pub struct ConfigUpdateDTO {
    pub logger: Logger,
}

/// Subset of [`super::logger`] configuration.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[allow(missing_docs)]
pub struct Logger {
    pub level: Level,
    pub filter: Option<Directives>,
}

impl From<&'_ base::Logger> for Logger {
    fn from(value: &'_ base::Logger) -> Self {
        Self {
            level: value.level,
            filter: value.filter.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[allow(missing_docs)]
pub struct Queue {
    pub capacity: NonZero<usize>,
}

impl From<&'_ base::Queue> for Queue {
    fn from(value: &'_ base::Queue) -> Self {
        Self {
            capacity: value.capacity,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[allow(missing_docs)]
pub struct Network {
    pub block_gossip_size: NonZero<u32>,
    pub block_gossip_period_ms: u32,
    pub transaction_gossip_size: NonZero<u32>,
    pub transaction_gossip_period_ms: u32,
}

impl From<&'_ base::Root> for Network {
    fn from(value: &'_ base::Root) -> Self {
        Self {
            block_gossip_size: value.block_sync.gossip_size,
            block_gossip_period_ms: u32::try_from(value.block_sync.gossip_period.as_millis())
                .expect("block gossip period should fit into a u32"),
            transaction_gossip_size: value.transaction_gossiper.gossip_size,
            transaction_gossip_period_ms: u32::try_from(
                value.transaction_gossiper.gossip_period.as_millis(),
            )
            .expect("transaction gossip period should fit into a u32"),
        }
    }
}

#[cfg(test)]
mod test {
    use iroha_crypto::KeyPair;
    use iroha_data_model::Level;
    use nonzero_ext::nonzero;

    use super::*;

    #[test]
    fn snapshot_serialized_form() {
        let value = ConfigGetDTO {
            public_key: KeyPair::from_seed(vec![1, 2, 3], <_>::default())
                .public_key()
                .clone(),
            logger: Logger {
                level: Level::TRACE,
                filter: None,
            },
            network: Network {
                block_gossip_size: nonzero!(10u32),
                block_gossip_period_ms: 5000,
                transaction_gossip_size: nonzero!(512u32),
                transaction_gossip_period_ms: 1000,
            },
            queue: Queue {
                capacity: nonzero!(656_565_usize),
            },
        };

        let actual = serde_json::to_string_pretty(&value).expect("The value is a valid JSON");

        // NOTE: whenever this is updated, make sure to update the documentation accordingly:
        //       https://docs.iroha.tech/reference/torii-endpoints.html
        //       -> Configuration endpoints
        let expected = expect_test::expect![[r#"
            {
              "public_key": "ed0120E159467BC273205BB250D41686FCA6CB85F163D5354B06CB3BA8FD42290EDF2A",
              "logger": {
                "level": "TRACE",
                "filter": null
              },
              "network": {
                "block_gossip_size": 10,
                "block_gossip_period_ms": 5000,
                "transaction_gossip_size": 512,
                "transaction_gossip_period_ms": 1000
              },
              "queue": {
                "capacity": 656565
              }
            }"#]];
        expected.assert_eq(&actual);
    }
}
