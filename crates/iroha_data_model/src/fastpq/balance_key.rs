//! Canonical identity of a FASTPQ asset balance leaf.

use crate::{account::AccountId, asset::id::AssetDefinitionId};

/// First-release transfer balance identity committed by the FASTPQ sparse Merkle tree.
///
/// The canonical Norito frame supplies the V1 schema identity and declared layout. The full
/// domainless account controller is included; display prefixes and aliases are never key material.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    norito::Encode,
    norito::Decode,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
)]
#[norito_schema(name = "iroha_data_model::fastpq::FastpqBalanceKeyV1")]
pub struct FastpqBalanceKeyV1 {
    /// Canonical asset definition identity.
    pub asset_definition: AssetDefinitionId,
    /// Complete canonical account controller that owns the balance.
    pub account: AccountId,
}

/// Encode the sole V1 FASTPQ transfer balance key.
///
/// The host, prover, and fixture builders use the same canonical frame independently of ambient
/// Norito layout guards or the chain discriminant used to display an account address.
///
/// # Errors
/// Returns a Norito error if the typed identity cannot be encoded.
pub fn transfer_balance_key(
    asset_definition: &AssetDefinitionId,
    account: &AccountId,
) -> Result<Vec<u8>, norito::Error> {
    norito::encode_canonical(&FastpqBalanceKeyV1 {
        asset_definition: asset_definition.clone(),
        account: account.clone(),
    })
}

#[cfg(test)]
mod tests;
