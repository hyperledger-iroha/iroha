//! Regenerate Norito codec sample files.
//!
//! External dependencies:
//! - `clap` for CLI parsing
//! - `color-eyre` for error handling

use std::{fs, path::PathBuf, str::FromStr};

use clap::Parser;
use color_eyre::eyre::Result;
use iroha_data_model::{account::NewAccount, ipfs::IpfsPath, metadata::Metadata, name::Name};
use iroha_primitives::json::Json;
use iroha_test_samples::ALICE_ID;
use norito::codec::Encode;

/// Regenerate Norito codec sample files used by `kagami`.
#[derive(Parser)]
struct Args {
    /// Output directory for regenerated samples
    #[arg(long, default_value = "crates/iroha_kagami/samples/codec")]
    out: PathBuf,
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let args = Args::parse();
    fs::create_dir_all(&args.out)?;

    regenerate_account(&args.out)?;
    regenerate_domain(&args.out)?;
    Ok(())
}

fn regenerate_account(out_dir: &PathBuf) -> Result<()> {

    let mut metadata = Metadata::default();
    metadata.insert(
        Name::from_str("hat")?,
        Json::from(norito::json!({ "Name": "white" })),
    );

    let account = NewAccount {
        id: (*ALICE_ID).clone(),
        metadata,
        label: None,
    };

    let json = norito::json::to_json_pretty(&account)?;
    fs::write(out_dir.join("account.json"), format!("{json}\n"))?;
    let bin = account.encode();
    fs::write(out_dir.join("account.bin"), bin)?;
    Ok(())
}

fn regenerate_domain(out_dir: &PathBuf) -> Result<()> {
    let mut metadata = Metadata::default();
    metadata.insert(Name::from_str("Is_Jabberwocky_alive")?, Json::from(true));

    let domain = iroha_data_model::domain::Domain::new("wonderland".parse()?)
        .with_logo(IpfsPath::from_str(
            "/ipfs/Qme7ss3ARVgxv6rXqVPiikMJ8u2NLgmgszg13pYrDKEoiu",
        )?)
        .with_metadata(metadata);

    let json = norito::json::to_json_pretty(&domain)?;
    fs::write(out_dir.join("domain.json"), format!("{json}\n"))?;
    let bin = domain.encode();
    fs::write(out_dir.join("domain.bin"), bin)?;
    Ok(())
}
