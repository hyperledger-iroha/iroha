//! Encode a `SetAccountAliasBinding` instruction for `iroha tx stdin`.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use iroha_data_model::{
    account::{AccountId, address::ChainDiscriminantGuard, rekey::AccountAlias},
    isi::{
        InstructionBox, account_alias_lease::AcquireAccountAliasLease,
        domain_link::SetAccountAliasBinding,
    },
    nexus::DataSpaceId,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let account = args.next().ok_or("missing account id")?;
    let label = args.next().ok_or("missing alias label")?;
    let domain = args.next().ok_or("missing alias domain, use - for none")?;
    let dataspace = args.next().ok_or("missing dataspace id")?;
    let chain_discriminant = args
        .next()
        .map(|raw| raw.parse())
        .transpose()?
        .unwrap_or(369);
    let payer = args.next();
    let term_years = args.next().map(|raw| raw.parse()).transpose()?.unwrap_or(1);

    let _chain_guard = ChainDiscriminantGuard::enter(chain_discriminant);
    let account = AccountId::parse_encoded(&account)?.into_account_id();
    let payer = payer
        .as_deref()
        .map(AccountId::parse_encoded)
        .transpose()?
        .map(iroha_data_model::account::ParsedAccountId::into_account_id);
    let alias = AccountAlias::new(
        label.parse()?,
        if domain == "-" {
            None
        } else {
            Some(domain.parse()?)
        },
        DataSpaceId::new(dataspace.parse()?),
    );
    let mut instructions = Vec::new();
    if let Some(payer) = payer {
        instructions.push(InstructionBox::from(AcquireAccountAliasLease::new(
            alias.clone(),
            account.clone(),
            payer,
            term_years,
            None,
        )));
    }
    instructions.push(InstructionBox::from(SetAccountAliasBinding::bind(
        account, alias, None,
    )));

    print!("[");
    for (idx, instruction) in instructions.iter().enumerate() {
        if idx > 0 {
            print!(",");
        }
        let encoded = STANDARD.encode(norito::to_bytes::<InstructionBox>(instruction)?);
        print!(r#""{encoded}""#);
    }
    println!("]");
    Ok(())
}
