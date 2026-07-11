use std::io::{BufWriter, Write};

use clap::Subcommand;
use color_eyre::eyre::eyre;
use iroha_genesis::RawGenesisTransaction;

use crate::{Outcome, RunArgs};

mod embed_pop;
mod generate;
mod normalize;
mod npos;
mod pop;
pub mod profile;
mod sign;
mod validate;

pub use generate::{
    ConsensusPolicy, build_line_from_env, generate_default, validate_consensus_mode_for_line,
};
pub use npos::ensure_npos_parameters;
pub use profile::{
    GenesisProfile, PUBLIC_XOR_ALIAS, ProfileDefaults, TAIRA_XOR_ASSET_DEFINITION_ID,
    parse_vrf_seed_hex, profile_defaults, profile_requires_npos, profile_uses_public_xor,
    public_xor_profile_for_chain_id, resolve_vrf_seed,
};

fn require_v2_wire_protocol_only(manifest: &RawGenesisTransaction) -> color_eyre::Result<()> {
    let expected = [u32::from(
        iroha_data_model::block::consensus_v2::PROTOCOL_VERSION,
    )];
    if manifest.wire_proto_versions() != expected {
        return Err(eyre!(
            "fresh genesis must advertise exactly wire_proto_versions = [2]; legacy, mixed, and downgrade protocol lists are prohibited"
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Subcommand)]
pub enum Args {
    Sign(sign::Args),
    Generate(generate::Args),
    /// Validate a genesis JSON file and report invalid identifiers
    Validate(validate::Args),
    /// Produce a BLS PoP (Proof-of-Possession) for a consensus key (BLS-normal)
    Pop(pop::Args),
    /// Embed one or more PoPs into a genesis JSON manifest (inline `topology` entries carrying `pop_hex`)
    EmbedPop(embed_pop::Args),
    /// Expand a genesis manifest and show the final ordered transactions
    Normalize(normalize::Args),
}

impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        match self {
            Args::Sign(args) => args.run(writer),
            Args::Generate(args) => args.run(writer),
            Args::Validate(args) => args.run(writer),
            Args::Pop(args) => args.run(writer),
            Args::EmbedPop(args) => args.run(writer),
            Args::Normalize(args) => args.run(writer),
        }
    }
}
