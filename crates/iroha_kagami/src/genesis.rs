use std::io::{BufWriter, Write};

use clap::Subcommand;

use crate::{Outcome, RunArgs};

mod embed_pop;
mod generate;
mod normalize;
mod npos;
mod pop;
pub mod profile;
mod sign;
mod validate;

pub(crate) use generate::{
    build_line_from_env, generate_default, validate_consensus_mode_for_line,
};
pub use npos::ensure_npos_parameters;
pub use profile::{
    GenesisProfile, ProfileDefaults, parse_vrf_seed_hex, profile_defaults, resolve_vrf_seed,
};

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
