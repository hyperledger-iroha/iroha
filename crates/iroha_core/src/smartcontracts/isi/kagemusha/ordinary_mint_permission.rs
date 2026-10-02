//! Exact World permission for the separate ordinary pre-debit Mint issuer purpose.
//!
//! A full public FI policy is accepted only while the current World contains its exact purpose
//! token for the policy's Ed issuer account. Ordinary Mint proof admission, account consent,
//! global predecessor reservation and actual reserve debit are separate mandatory gates.

use crate::state::{StateReadOnly as _, StateTransaction, WorldReadOnly};
use iroha_data_model::{
    account::AccountId, kagemusha::KagemushaRetailEnrollmentIssuerPolicyV1, permission::Permission,
};
use iroha_executor_data_model::permission::kagemusha::CanAuthorizeKagemushaOrdinaryMint;
use mv::storage::StorageReadOnly as _;

/// Non-serializable evidence of an exact ordinary Mint issuer purpose in the actual current World.
/// Its private constructor supplies no proof, account debit, finalized credit or State effect.
#[derive(Debug)]
pub struct KagemushaWorldOrdinaryMintIssuerPurposeV1 {
    token: CanAuthorizeKagemushaOrdinaryMint,
}
impl KagemushaWorldOrdinaryMintIssuerPurposeV1 {
    /// Borrow the complete current World-authorized issuer policy.
    #[must_use]
    pub fn issuer_policy(&self) -> &KagemushaRetailEnrollmentIssuerPolicyV1 {
        &self.token.issuer_policy
    }
    /// Exact World-authorized ordinary proof release.
    #[must_use]
    pub fn release_id(&self) -> [u8; 32] {
        self.token.release_id
    }
    /// Borrow the exact full governance roots granted independently in the current World.
    #[must_use]
    pub fn app_identity_authority(
        &self,
    ) -> &iroha_data_model::kagemusha::KagemushaOrdinaryAppIdentityAuthorityPolicyV1 {
        &self.token.app_identity_authority
    }
    /// Exact complete clock-selection transport identity admitted by the same asset owner.
    #[must_use]
    pub fn clock_selection_original_sha256(&self) -> [u8; 32] {
        self.token.clock_selection_original_sha256
    }
    /// Borrow the same complete independently World-selected DATA authority for this pool.
    #[must_use]
    pub fn lineage_data_authority(
        &self,
    ) -> &iroha_data_model::kagemusha::KagemushaOrdinaryLineageDataAuthorityV1 {
        &self.token.lineage_data_authority
    }
    /// Recheck the same scope at actual deterministic transaction execution time.
    /// # Errors
    /// Refuses revocation, issuer expiry, another network or asset incarnation.
    pub fn recheck(&self, transaction: &StateTransaction<'_, '_>) -> Result<(), String> {
        require_current_scope(transaction, &self.token)
    }
}

/// Require a genuinely current, exactly World-granted ordinary Mint issuer purpose.
///
/// The decoded policy is data. Independent authority is the exact current direct/role permission,
/// including the full policy and release. Generic reserve or enrollment signing does not suffice.
/// # Errors
/// Refuses malformed policy, absent grant, wrong network/incarnation or original expiry.
pub fn admit_ordinary_mint_issuer_purpose_v1(
    transaction: &StateTransaction<'_, '_>,
    token: &CanAuthorizeKagemushaOrdinaryMint,
) -> Result<KagemushaWorldOrdinaryMintIssuerPurposeV1, String> {
    require_current_scope(transaction, token)?;
    Ok(KagemushaWorldOrdinaryMintIssuerPurposeV1 {
        token: token.clone(),
    })
}
fn require_current_scope(
    transaction: &StateTransaction<'_, '_>,
    token: &CanAuthorizeKagemushaOrdinaryMint,
) -> Result<(), String> {
    token.validate_scope()?;
    let policy = &token.issuer_policy;
    let runtime = &policy.runtime;
    let now = transaction.block_unix_timestamp_ms();
    if runtime.network_id != *transaction.network_id()
        || transaction
            .world
            .axt_asset_incarnations
            .get(&runtime.asset)
            .copied()
            != Some(runtime.asset_incarnation)
        || now < policy.valid_from_ms
        || now >= policy.expires_at_ms
    {
        return Err("ordinary Mint issuer World scope or original window differs".into());
    }
    let definition = transaction
        .world
        .asset_definition(&runtime.asset)
        .map_err(|error| error.to_string())?;
    token
        .lineage_data_authority
        .require_asset_definition_metadata(&definition)?;
    if !world_has_exact_ordinary_mint_issuer_permission_v1(&transaction.world, token)? {
        return Err("ordinary Mint issuer lacks the exact current World purpose".into());
    }
    Ok(())
}

/// Check the exact independently committed direct or role permission.
/// # Errors
/// Refuses malformed scope or failure reading the actual issuer's account permissions.
pub fn world_has_exact_ordinary_mint_issuer_permission_v1(
    world: &impl WorldReadOnly,
    token: &CanAuthorizeKagemushaOrdinaryMint,
) -> Result<bool, String> {
    token.validate_scope()?;
    let issuer = AccountId::new(token.issuer_policy.issuer_public_key.clone());
    let required: Permission = token.clone().into();
    let permissions = world
        .account_permissions_iter(&issuer)
        .map_err(|error| error.to_string())?;
    if permissions
        .into_iter()
        .any(|permission| permission == &required)
    {
        return Ok(true);
    }
    Ok(world.account_roles_iter(&issuer).any(|role_id| {
        world
            .roles()
            .get(role_id)
            .is_some_and(|role| role.permissions.contains(&required))
    }))
}
