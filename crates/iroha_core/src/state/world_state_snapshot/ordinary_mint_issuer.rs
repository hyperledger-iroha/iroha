//! Exact positive ordinary Mint permission grant at one qualified native pre-tail cut.
use super::*;
use iroha_crypto::{Algorithm, PublicKey};
use iroha_data_model::{
    account::AccountValue,
    permission::{Permission, Permissions},
    role::{Role, RoleIdWithOwner},
};
use iroha_executor_data_model::permission::kagemusha::CanAuthorizeKagemushaOrdinaryMint;
impl State {
    /// Publish only an actually granted full direct row or one actually assigned full role.
    /// The signed HTTP corridor supplies the authenticated exact Ed issuer. A decoded token
    /// cannot create this actual World grant and no broad ledger-read permission is required.
    /// # Errors
    /// Refuses another signer, absent/currently revoked scope, changed cut or finite bounds.
    #[allow(clippy::too_many_arguments)]
    pub fn with_native_ordinary_mint_issuer_purpose_v1<T>(
        &self,
        tip: &CommittedBlock,
        authenticated_issuer: &AccountId,
        authenticated_signer: &PublicKey,
        asset: &AssetDefinitionId,
        required: &Permission,
        budget: &AllocationBudget,
        consume: impl FnOnce(
            &WorldStateSnapshotV1,
            &AccountValue,
            Option<&Permissions>,
            Option<&Role>,
        ) -> Result<T, String>,
    ) -> Result<T, WorldStateSnapshotError> {
        if authenticated_issuer.try_signatory() != Some(authenticated_signer)
            || authenticated_signer.algorithm() != Algorithm::Ed25519
        {
            return Err("ordinary Mint purpose authenticated issuer differs".into());
        }
        let token = CanAuthorizeKagemushaOrdinaryMint::try_from(required)
            .map_err(|_| "ordinary Mint purpose token differs")?;
        token.validate_scope()?;
        if token.issuer_policy.issuer_public_key != *authenticated_signer
            || token.issuer_policy.runtime.asset != *asset
            || token.issuer_policy.runtime.network_id != *self.network_id_ref()
        {
            return Err("ordinary Mint purpose exact runtime differs".into());
        }
        self.with_native_world_state_snapshot_cut_v1(tip, None, budget, |snapshot, world| {
            let issuer = world
                .accounts()
                .get(authenticated_issuer)
                .ok_or("ordinary Mint issuer registration absent")?;
            let definition = world
                .asset_definitions()
                .get(asset)
                .ok_or("ordinary Mint issuer asset absent")?;
            let incarnation = world
                .axt_asset_incarnations()
                .get(asset)
                .ok_or("ordinary Mint issuer incarnation absent")?;
            if *incarnation != token.issuer_policy.runtime.asset_incarnation {
                return Err("ordinary Mint issuer incarnation differs".into());
            }
            token
                .lineage_data_authority
                .require_asset_definition_metadata(definition)?;
            require_target(
                snapshot,
                "world.accounts",
                WorldStateElementKindV1::Table,
                Some(hash_value(authenticated_issuer)?),
                hash_value(issuer)?,
            )?;
            if let Some(permissions) = world
                .account_permissions()
                .get(authenticated_issuer)
                .filter(|p| p.contains(required))
            {
                if norito::canonical_frame_len(permissions).map_err(|e| e.to_string())? > 64 * 1024
                {
                    return Err("ordinary Mint direct original exceeds bound".into());
                }
                require_target(
                    snapshot,
                    "world.account_permissions",
                    WorldStateElementKindV1::Table,
                    Some(hash_value(authenticated_issuer)?),
                    hash_value(permissions)?,
                )?;
                return consume(snapshot, issuer, Some(permissions), None);
            }
            for role_id in world.account_roles_iter(authenticated_issuer) {
                let Some(role) = world
                    .roles()
                    .get(role_id)
                    .filter(|r| r.permissions.contains(required))
                else {
                    continue;
                };
                if norito::canonical_frame_len(role).map_err(|e| e.to_string())? > 64 * 1024 {
                    return Err("ordinary Mint role original exceeds bound".into());
                }
                let assignment =
                    RoleIdWithOwner::new(authenticated_issuer.clone(), role_id.clone());
                require_target(
                    snapshot,
                    "world.account_roles",
                    WorldStateElementKindV1::Table,
                    Some(hash_value(&assignment)?),
                    hash_value(&())?,
                )?;
                require_target(
                    snapshot,
                    "world.roles",
                    WorldStateElementKindV1::Table,
                    Some(hash_value(role_id)?),
                    hash_value(role)?,
                )?;
                return consume(snapshot, issuer, None, Some(role));
            }
            Err("ordinary Mint exact current purpose is revoked or absent".into())
        })
    }
}
