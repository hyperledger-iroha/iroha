//! Data events.

use std::{format, string::String, vec, vec::Vec};

pub use events::{DataEvent, confidential};
pub use filters::{DataEventFilter, OfflineTransferEventFilter};
use iroha_macro::FromVariant;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

#[cfg(not(feature = "json"))]
#[cfg(feature = "transparent_api")]
use super::EventFilter;
pub use crate::Registered;
use crate::prelude::*;
mod events;
mod filters;
#[cfg(feature = "governance")]
pub mod governance;
pub mod offline;
pub mod oracle;
pub mod proof;
pub mod runtime_upgrade;
pub mod smart_contract;
pub mod social;
pub mod soradns;
pub mod sorafs;
pub mod space_directory;
pub mod verifying_keys;

/// Exports common structs and enums from this module.
pub mod prelude {
    pub use super::{
        confidential::prelude::*,
        events::prelude::*,
        filters::prelude::*,
        offline::prelude::*,
        oracle::prelude::*,
        social::prelude::*,
        soradns::{SoradnsDirectoryEvent, SoradnsDirectoryEventSet},
        sorafs::prelude::*,
        space_directory::prelude::*,
    };
}
