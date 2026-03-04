//! Example program that serializes sample Iroha data structures with Norito.
use std::{io::Result, path::Path, str::FromStr};

use iroha_data_model::Registrable;
use iroha_data_model::{
    account::{AccountId, NewAccount},
    domain::{Domain, DomainId},
    events::{EventFilterBox, data::DataEventFilter},
    metadata::Metadata,
    transaction::Executable,
    trigger::{
        Trigger, TriggerId,
        action::{Action, Repeats},
    },
};
use iroha_primitives::const_vec::ConstVec;
use norito::{core::NoritoSerialize, json::JsonSerialize};

fn write_sample<T: NoritoSerialize + JsonSerialize>(
    dir: &Path,
    name: &str,
    value: &T,
) -> Result<()> {
    let bytes = norito::to_bytes(value)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err.to_string()))?;
    assert_eq!(&bytes[..4], &norito::core::MAGIC);
    std::fs::write(dir.join(format!("{name}.bin")), &bytes)?;
    let json = norito::json::to_json_pretty(value)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, err.to_string()))?;
    std::fs::write(dir.join(format!("{name}.json")), json)?;
    Ok(())
}

fn main() -> Result<()> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"));

    // Account
    let account_id = AccountId::from_str(
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland",
    )
    .unwrap();
    let mut account_metadata = Metadata::default();
    account_metadata.insert("hat".parse().unwrap(), "white");
    let account = NewAccount::new(account_id.clone()).with_metadata(account_metadata);

    // Domain
    let domain_id = DomainId::from_str("wonderland").unwrap();
    let mut domain_metadata = Metadata::default();
    domain_metadata.insert("Is_Jabberwocky_alive".parse().unwrap(), true);
    let domain = Domain::new(domain_id)
        .with_logo(
            "/ipfs/Qme7ss3ARVgxv6rXqVPiikMJ8u2NLgmgszg13pYrDKEoiu"
                .parse()
                .unwrap(),
        )
        .with_metadata(domain_metadata)
        .build(&account_id);

    // Trigger
    let mut trigger_metadata = Metadata::default();
    trigger_metadata.insert("tea_time".parse().unwrap(), 5_u32);
    let action = Action::new(
        Executable::Instructions(ConstVec::new_empty()),
        Repeats::Exactly(1),
        account_id,
        EventFilterBox::Data(DataEventFilter::Any),
    )
    .with_metadata(trigger_metadata);
    let trigger = Trigger::new(TriggerId::from_str("tea_party").unwrap(), action);

    write_sample(dir, "account", &account)?;
    write_sample(dir, "domain", &domain)?;
    write_sample(dir, "trigger", &trigger)?;

    Ok(())
}
