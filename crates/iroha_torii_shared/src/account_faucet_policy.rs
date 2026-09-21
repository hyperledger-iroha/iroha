//! Public faucet identity and issuance policy, discovered separately from prepared transactions.

use iroha_data_model::{NetworkId, account::AccountId, asset::AssetDefinitionId};
use iroha_primitives::numeric::Quantity;
use norito::derive::{JsonDeserialize, JsonSerialize};

/// Maximum canonical public faucet policy response body.
pub const ACCOUNT_FAUCET_POLICY_MAX_BYTES: usize = 4096;

/// Operator-configured faucet policy bound to one exact genesis and address profile.
///
/// Clients obtain this through their trusted network endpoint before preparing a claim. They
/// retain the policy independently and require the prepared envelope to match it exactly.
#[derive(Clone, Debug, PartialEq, Eq, JsonSerialize, JsonDeserialize)]
#[norito(deny_unknown_fields)]
pub struct AccountFaucetAdvertisement {
    /// First-release policy schema.
    pub schema_version: u16,
    /// Exact genesis-derived signing identity.
    pub network_id: NetworkId,
    /// Canonical account address discriminant.
    pub network_prefix: u16,
    /// Account whose funds and signature authorize each claim, including its transaction fees.
    pub authority: AccountId,
    /// Resolved canonical asset definition issued by this faucet.
    pub asset_definition_id: AssetDefinitionId,
    /// Exact positive quantity transferred by each claim.
    pub amount: Quantity,
}
