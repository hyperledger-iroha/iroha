//! Genesis helpers (bootstrap, request/response plumbing).

pub mod bootstrap;

pub use bootstrap::{
    GenesisMessage, GenesisRequest, GenesisRequestKind, GenesisResponse, GenesisResponseError,
};
