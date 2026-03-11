//! Generate sample Norito archives for account, domain, and trigger structures.

use std::{
    error::Error,
    path::{Path, PathBuf},
};

use iroha_data_model::{Registrable, events::pipeline::BlockEventFilter, prelude::*};
use norito::{core::NoritoSerialize, json::JsonSerialize};

fn main() -> Result<(), Box<dyn Error>> {
    iroha_genesis::init_instruction_registry();
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("samples/codec");

    // Account sample
    let domain: DomainId = "wonderland".parse()?;
    let public_key =
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03".parse()?;
    let account_id = AccountId::new(public_key);
    let mut account = Account::new(account_id.to_account_id(domain.clone())).build(&account_id);
    account
        .metadata
        .insert("hat".parse()?, norito::json!({ "Name": "white" }));
    write_sample(&out_dir, "account", &account)?;

    // Domain sample
    let mut domain_metadata = Metadata::default();
    domain_metadata.insert("Is_Jabberwocky_alive".parse()?, norito::json!(true));
    let domain = Domain::new("wonderland".parse()?)
        .with_logo(
            "/ipfs/Qme7ss3ARVgxv6rXqVPiikMJ8u2NLgmgszg13pYrDKEoiu"
                .parse()
                .expect("valid IPFS path"),
        )
        .with_metadata(domain_metadata)
        .build(&account_id);
    write_sample(&out_dir, "domain", &domain)?;

    // Trigger sample
    let instruction = Log::new(Level::INFO, "Hello from trigger".to_string());
    let action = Action::new(
        vec![instruction],
        Repeats::Exactly(1),
        account_id,
        BlockEventFilter::default(),
    );
    let trigger = Trigger::new("log_trigger".parse()?, action);
    write_sample(&out_dir, "trigger", &trigger)?;

    Ok(())
}

fn write_sample<T>(dir: &Path, name: &str, value: &T) -> Result<(), Box<dyn Error>>
where
    T: JsonSerialize + NoritoSerialize + norito::json::JsonDeserializeOwned,
{
    let json = norito::json::to_json_pretty(value)?;
    std::fs::write(dir.join(format!("{name}.json")), json.as_bytes())?;

    let parsed_value: norito::json::Value = norito::json::from_str(&json)?;
    let parsed: T = norito::json::value::from_value(parsed_value)?;
    let bytes = norito::to_bytes(&parsed)?;
    assert!(bytes.starts_with(&norito::core::MAGIC));
    std::fs::write(dir.join(format!("{name}.bin")), &bytes)?;
    Ok(())
}
