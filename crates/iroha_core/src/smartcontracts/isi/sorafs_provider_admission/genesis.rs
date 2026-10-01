//! Sole signed-genesis initialization from network-independent canonical material.
use super::*;
use crate::smartcontracts::Execute;
use iroha_data_model::{
    account::AccountId,
    isi::sorafs::{
        EstablishSorafsProviderOwnerV1, InitializeSorafsProviderAdmissionV1,
        SorafsProviderGovernanceActionV1,
    },
    sorafs::{
        capacity::ProviderId,
        provider_admission::governance::{
            PROVIDER_ADMISSION_GENESIS_MAX_BYTES_V1, PROVIDER_ADMISSION_GENESIS_MAX_PROVIDERS_V1,
        },
    },
};
use iroha_model_base::state_path::StatePath;
use sorafs_manifest::provider_admission::ProviderAdmissionGenesisMaterialV1;

impl Execute for InitializeSorafsProviderAdmissionV1 {
    fn execute(
        self,
        _authority: &AccountId,
        tx: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        let direct = std::mem::take(&mut tx.current_direct_sorafs_admission_initialization);
        if !direct || !crate::executor::is_initial_genesis_context(tx) {
            return Err(rejected(
                "provider admission initialization requires the exact direct signed genesis instruction",
            ));
        }
        let prefix: StatePath = "sorafs/provider_admission/"
            .parse()
            .expect("fixed native prefix");
        if tx
            .world()
            .smart_contract_state()
            .range(prefix.clone()..)
            .next()
            .is_some_and(|(path, _)| path.as_ref().starts_with(prefix.as_ref()))
            || self.providers.len() > PROVIDER_ADMISSION_GENESIS_MAX_PROVIDERS_V1
            || norito::canonical_frame_len(&self).map_err(rejected)?
                > PROVIDER_ADMISSION_GENESIS_MAX_BYTES_V1
        {
            return Err(rejected(
                "provider admission genesis journal is occupied or exceeds its bounds",
            ));
        }
        let policy = self
            .council
            .bind(*tx.network_id().as_bytes())
            .map_err(rejected)?;
        let policy_digest = policy.canonical_digest().map_err(rejected)?;
        let now = tx.block_unix_timestamp_ms();
        if now == 0 || now == u64::MAX {
            return Err(rejected("invalid genesis time"));
        }
        let origin = native::GenesisAdmissionOriginV1 {
            entrypoint_index: tx
                .current_entrypoint_index
                .and_then(|value| u32::try_from(value).ok())
                .ok_or_else(|| rejected("missing direct genesis entrypoint"))?,
            instruction_digest: *iroha_crypto::Hash::new(
                norito::encode_canonical(&self).map_err(rejected)?,
            )
            .as_ref(),
        };
        let mut providers = Vec::with_capacity(self.providers.len());
        for entry in &self.providers {
            let material: ProviderAdmissionGenesisMaterialV1 =
                decode_frame(&entry.material).map_err(rejected)?;
            let projection = material
                .project(policy.network_id, policy.policy_id, policy_digest)
                .map_err(rejected)?;
            let provider = ProviderId::new(projection.proposal.provider_id);
            if providers
                .last()
                .is_some_and(|(previous, _)| *previous >= provider)
                || now / 1000 < projection.issued_at
                || now / 1000 >= projection.retention_epoch
            {
                return Err(rejected(
                    "genesis entries must be sorted, unique and currently valid",
                ));
            }
            tx.world().account(&entry.owner)?;
            if tx
                .world()
                .provider_owners()
                .get(&provider)
                .is_some_and(|owner| owner != &entry.owner)
            {
                return Err(rejected(
                    "genesis provider owner conflicts with existing binding",
                ));
            }
            providers.push((provider, native::encode(&projection).map_err(rejected)?));
        }
        // No signed envelope can contain the hash of the genesis that contains itself. The
        // exact direct signed genesis authenticates these templates; later mutations require
        // the ordinary enacted Parliament/council path and cannot select this origin.
        if !apply_inner(
            Action::ConfigureCouncil(native::encode(&policy).map_err(rejected)?),
            tx,
            Some(origin),
        )
        .map_err(rejected)?
        {
            return Err(rejected("genesis council was not initialized"));
        }
        for ((provider_id, projection), entry) in providers.into_iter().zip(self.providers) {
            if tx.world().provider_owners().get(&provider_id).is_none()
                && !super::super::sorafs::apply_governed_provider_owner_action(
                    SorafsProviderGovernanceActionV1::Establish(EstablishSorafsProviderOwnerV1 {
                        provider_id,
                        owner: entry.owner,
                    }),
                    tx,
                )?
            {
                return Err(rejected("genesis provider owner was not established"));
            }
            if !apply_inner(Action::Admit(projection), tx, Some(origin)).map_err(rejected)? {
                return Err(rejected("genesis provider was not initialized"));
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests;
