//! Localnet generation regression tests.
use super::*;
use iroha_config::{
    base::toml::TomlSource, kura::FsyncMode, logger::Directives, parameters::actual,
};
use iroha_data_model::{
    block::decode_framed_signed_block,
    isi::{GrantBox, MintBox, SetParameter, TransferBox},
    parameter::{
        Parameter,
        system::{Parameters, SumeragiConsensusMode, SumeragiNposParameters, consensus_metadata},
    },
    sumeragi::PROTOCOL_VERSION,
    transaction::Executable,
};
use iroha_executor_data_model::permission::account::CanDelegateAccountAliasResolution;
use norito::{json, literal};

include!("tests/part_01.rs");
include!("tests/part_02.rs");
