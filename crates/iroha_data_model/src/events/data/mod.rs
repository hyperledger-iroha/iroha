//! Data events.

use std::{format, string::String, vec, vec::Vec};

pub use events::DataEvent;
pub use filters::{DataEventFilter, EscrowEventFilter};
use iroha_macro::FromVariant;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

#[cfg(not(feature = "json"))]
#[cfg(feature = "transparent_api")]
use super::EventFilter;
pub use crate::Registered;
use crate::prelude::*;
pub mod escrow;
mod events;
mod filters;
#[cfg(feature = "governance")]
pub mod governance;
pub mod musubi;
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
        escrow::prelude::*,
        events::prelude::*,
        filters::prelude::*,
        musubi::prelude::*,
        oracle::prelude::*,
        social::prelude::*,
        soradns::{SoradnsDirectoryEvent, SoradnsDirectoryEventSet},
        sorafs::prelude::*,
        space_directory::prelude::*,
    };
}
