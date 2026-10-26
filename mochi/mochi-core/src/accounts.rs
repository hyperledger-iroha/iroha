//! In-memory test accounts, token drafts and canonical asset literals for desktop quick
//! actions.
//!
//! Generated keys never leave process memory: the desktop shows them only after an explicit
//! reveal, and every ledger change still goes through the reviewed transaction preview.

use crate::compose::{ComposeError, InstructionDraft, parse_account_id, parse_asset_definition_id};
use iroha_crypto::{ExposedPrivateKey, KeyPair};
use iroha_data_model::{
    account::{AccountId, address::ChainDiscriminantGuard},
    asset::{
        definition::Mintable,
        id::{AssetDefinitionId, AssetId},
    },
};
use iroha_model_base::{domain::DomainId, error::ParseError, name::Name};
use std::fmt;

/// A freshly generated single-signature account whose private key exists only in memory.
///
/// `Debug` is redacted so that embedding this value in diagnostics cannot disclose the key.
#[derive(Clone)]
pub struct GeneratedAccount {
    key_pair: KeyPair,
    account: AccountId,
}

impl GeneratedAccount {
    /// Generate a key pair with the default algorithm and derive its canonical account.
    #[must_use]
    pub fn generate() -> Self {
        let key_pair = KeyPair::random();
        let account = AccountId::new(key_pair.public_key().clone());
        Self { key_pair, account }
    }

    /// Canonical account identifier controlled by the generated key.
    #[must_use]
    pub fn account_id(&self) -> &AccountId {
        &self.account
    }

    /// Account literal formatted with the network's address discriminant.
    #[must_use]
    pub fn account_literal(&self, discriminant: u16) -> String {
        let _guard = ChainDiscriminantGuard::enter(discriminant);
        self.account.to_string()
    }

    /// Public key multihash for SDK and CLI configuration.
    #[must_use]
    pub fn public_key(&self) -> String {
        self.key_pair.public_key().to_string()
    }

    /// Private key multihash for local test tooling. Never log or persist this value.
    #[must_use]
    pub fn exposed_private_key(&self) -> String {
        ExposedPrivateKey(self.key_pair.private_key().clone()).to_string()
    }

    /// Draft registering this account; submission still requires a reviewed preview.
    #[must_use]
    pub fn register_draft(&self) -> InstructionDraft {
        InstructionDraft::RegisterAccount {
            account: self.account.clone(),
        }
    }
}

impl fmt::Debug for GeneratedAccount {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GeneratedAccount")
            .field("account", &self.account)
            .field("private_key", &"<redacted>")
            .finish()
    }
}

/// Longest domain label the name service prices.
const MAX_DOMAIN_LABEL_CHARS: usize = 63;

/// Domain label for a token name: lowercase ASCII letters and digits, with every other run of
/// characters folded into one dash, e.g. `Coffee Beans!` becomes `coffee-beans`.
///
/// Empty when the name has no ASCII letter or digit.
#[must_use]
pub fn token_domain_label(name: &str) -> String {
    let mut label = String::new();
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            label.push(character.to_ascii_lowercase());
        } else if !label.is_empty() && !label.ends_with('-') {
            label.push('-');
        }
    }
    let label = label
        .chars()
        .take(MAX_DOMAIN_LABEL_CHARS)
        .collect::<String>();
    label.trim_end_matches('-').to_owned()
}

/// A token its creator registers and first mints in one transaction, validated from input.
#[derive(Clone, Debug)]
pub struct TokenDraft {
    /// `<label>.universal`, which the creator must hold when the drafts apply.
    pub domain: DomainId,
    /// Definition derived from the token's label and domain: the same pair always names it.
    pub definition: AssetDefinitionId,
    /// The definition's registration in `domain`, then the first mint to the creator.
    pub drafts: Vec<InstructionDraft>,
}

impl TokenDraft {
    /// Validate a token named `name` in `<label>.universal` whose first `supply` goes to
    /// `creator`.
    ///
    /// # Errors
    /// A name without letters or digits or with reserved characters, an invalid domain label,
    /// or a non-canonical supply.
    pub fn new(
        name: &str,
        label: &str,
        supply: &str,
        creator: &AccountId,
    ) -> Result<Self, ComposeError> {
        let name = name.trim();
        let seed: Name = token_domain_label(name)
            .parse()
            .map_err(
                |error: ParseError| ComposeError::InvalidAssetDefinitionName {
                    name: name.to_owned(),
                    reason: format!("a token name needs at least one letter or digit ({error})"),
                },
            )?;
        let label = label.trim();
        let domain = DomainId::try_new(label, "universal").map_err(|error| {
            ComposeError::InvalidDomainId {
                domain: label.to_owned(),
                reason: format!("use lowercase letters, digits and dashes ({error})"),
            }
        })?;
        let definition = AssetDefinitionId::derive_from_components(domain.clone(), seed);
        let register = InstructionDraft::register_asset_definition_from_input(
            &definition.to_string(),
            name,
            Mintable::Infinitely,
        )?
        .in_domain(domain.clone());
        let mint = InstructionDraft::mint_from_input(
            &AssetId::new(definition.clone(), creator.clone()).to_string(),
            supply.trim(),
        )?;
        Ok(Self {
            domain,
            definition,
            drafts: vec![register, mint],
        })
    }
}

/// Canonical global-scope literal of the `definition` asset held by `account`.
///
/// Dataspace-scoped holdings keep their exact literal from state queries instead.
///
/// # Errors
/// Either identifier is not a canonical literal for the network's address discriminant.
pub fn asset_literal(
    definition: &str,
    account: &str,
    discriminant: u16,
) -> Result<String, ComposeError> {
    let _guard = ChainDiscriminantGuard::enter(discriminant);
    let definition = parse_asset_definition_id(definition.trim())?;
    let account = parse_account_id(account.trim())?;
    Ok(AssetId::new(definition, account).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const DISCRIMINANT: u16 = 0x02F1;
    /// Canonical Base58 asset definition used by the shared composer fixtures.
    const DEFINITION: &str = "6pEP9RjNoZ7beWkT3pLfKoM1dyfi";

    #[test]
    fn generated_accounts_are_unique_registrable_and_redacted() {
        let first = GeneratedAccount::generate();
        let second = GeneratedAccount::generate();
        assert_ne!(first.account_id(), second.account_id());
        let literal = first.account_literal(DISCRIMINANT);
        let _guard = ChainDiscriminantGuard::enter(DISCRIMINANT);
        assert_eq!(
            AccountId::parse_encoded(&literal).unwrap(),
            *first.account_id()
        );
        assert!(matches!(
            first.register_draft(),
            InstructionDraft::RegisterAccount { account } if account == *first.account_id()
        ));
        let private = first.exposed_private_key();
        assert!(!private.is_empty());
        assert_ne!(private, second.exposed_private_key());
        assert!(!first.public_key().is_empty());
        let debug = format!("{first:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains(&private));
    }

    #[test]
    fn asset_literal_round_trips_through_the_composer() {
        let holder = GeneratedAccount::generate();
        let account = holder.account_literal(DISCRIMINANT);
        let definition = DEFINITION;
        let literal = asset_literal(&format!(" {definition} "), &account, DISCRIMINANT).unwrap();
        assert_eq!(literal, format!("{definition}#{account}"));
        let _guard = ChainDiscriminantGuard::enter(DISCRIMINANT);
        let draft = InstructionDraft::mint_from_input(&literal, "5").unwrap();
        assert!(matches!(
            draft,
            InstructionDraft::MintAsset { asset, .. }
                if asset.account() == holder.account_id() && asset.to_string() == literal
        ));
        assert!(asset_literal("not a definition", &account, DISCRIMINANT).is_err());
        assert!(asset_literal(definition, "not an account", DISCRIMINANT).is_err());
    }

    #[test]
    fn token_names_fold_into_domain_labels() {
        assert_eq!(token_domain_label("Coffee Beans!"), "coffee-beans");
        assert_eq!(token_domain_label("  ☕ Mochi  Points "), "mochi-points");
        assert_eq!(token_domain_label("ラーメン"), "");
        let long = token_domain_label(&format!("{}!b", "a".repeat(62)));
        assert_eq!(
            long,
            "a".repeat(62),
            "63 characters, without a trailing dash"
        );
    }

    #[test]
    fn new_tokens_register_in_their_own_domain_then_mint_to_the_creator() {
        let creator = GeneratedAccount::generate();
        let token = TokenDraft::new(" Coffee ", "coffee", "1000", creator.account_id()).unwrap();
        assert_eq!(token.domain.to_string(), "coffee.universal");
        assert!(matches!(
            &token.drafts[..],
            [
                InstructionDraft::RegisterAssetDefinition { definition, name, mintable: Mintable::Infinitely, domain: Some(domain) },
                InstructionDraft::MintAsset { asset, quantity },
            ] if *definition == token.definition
                && name == "Coffee"
                && *domain == token.domain
                && asset.definition() == &token.definition
                && asset.account() == creator.account_id()
                && quantity.to_string() == "1000"
        ));
        let again = TokenDraft::new("Coffee", "coffee", "5", creator.account_id()).unwrap();
        assert_eq!(
            again.definition, token.definition,
            "the same pair names the same token"
        );
        let elsewhere = TokenDraft::new("Coffee", "cafe", "5", creator.account_id()).unwrap();
        assert_ne!(elsewhere.definition, token.definition);
        // Registration drafts keep their domain through the composer's JSON form.
        let json = token.drafts[0].to_json_value();
        assert_eq!(
            json.get("domain").and_then(norito::json::Value::as_str),
            Some("coffee.universal")
        );
        assert!(matches!(
            InstructionDraft::from_json_value(&json).unwrap(),
            InstructionDraft::RegisterAssetDefinition { domain: Some(domain), .. } if domain == token.domain
        ));
        assert!(token.drafts[0].summary().contains("in coffee.universal"));

        for (name, label, supply) in [
            ("☕", "coffee", "1"),
            ("Coffee", "Coffee Shop", "1"),
            ("Coffee", "", "1"),
            ("Coffee", "coffee", "lots"),
        ] {
            assert!(
                TokenDraft::new(name, label, supply, creator.account_id()).is_err(),
                "{name:?} {label:?} {supply:?}"
            );
        }
        assert!(matches!(
            TokenDraft::new("Coffee", "Coffee Shop", "1", creator.account_id()),
            Err(ComposeError::InvalidDomainId { .. })
        ));
    }
}
