//! Regenerate Norito codec sample files.
//!
//! External dependencies:
//! - `clap` for CLI parsing
//! - `color-eyre` for error handling

use std::{fs, path::PathBuf, str::FromStr};

use clap::Parser;
use color_eyre::eyre::Result;
use iroha_data_model::{
    Registrable,
    account::{Account, NewAccount},
    domain::{Domain, DomainId},
    metadata::Metadata,
    name::Name,
    sorafs_uri::SorafsUri,
};
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
    let account = sample_account()?;
    let json = norito::json::to_json_pretty(&account)?;
    fs::write(out_dir.join("account.json"), format!("{json}\n"))?;
    let bin = account.encode();
    fs::write(out_dir.join("account.bin"), bin)?;
    Ok(())
}

fn sample_account() -> Result<NewAccount> {
    let mut metadata = Metadata::default();
    metadata.insert(
        Name::from_str("hat")?,
        Json::from(norito::json!({ "Name": "white" })),
    );

    Ok(Account::new((*ALICE_ID).clone()).with_metadata(metadata))
}

fn regenerate_domain(out_dir: &PathBuf) -> Result<()> {
    let domain = sample_domain()?;
    let json = norito::json::to_json_pretty(&domain)?;
    fs::write(out_dir.join("domain.json"), format!("{json}\n"))?;
    let bin = domain.encode();
    fs::write(out_dir.join("domain.bin"), bin)?;
    Ok(())
}

fn sample_domain() -> Result<Domain> {
    let mut metadata = Metadata::default();
    metadata.insert(Name::from_str("Is_Jabberwocky_alive")?, Json::from(true));

    let domain = Domain::new(DomainId::try_new("wonderland", "universal")?)
        .with_logo(SorafsUri::from_str(
            "sorafs://Qme7ss3ARVgxv6rXqVPiikMJ8u2NLgmgszg13pYrDKEoiu",
        )?)
        .with_metadata(metadata)
        .build(&ALICE_ID);

    Ok(domain)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::Identifiable;

    #[test]
    fn account_sample_uses_current_registration_shape() {
        let account = sample_account().expect("account sample should build");

        assert_eq!(account.id(), &*ALICE_ID);
        assert!(account.label().is_none());
        assert!(account.uaid().is_none());
        assert!(account.opaque_ids().is_empty());
    }

    #[test]
    fn domain_sample_is_registered_to_alice() {
        let domain = sample_domain().expect("domain sample should build");
        let expected_id =
            DomainId::try_new("wonderland", "universal").expect("sample domain id should parse");

        assert_eq!(domain.id(), &expected_id);
        assert_eq!(domain.owned_by(), &*ALICE_ID);
    }
}
