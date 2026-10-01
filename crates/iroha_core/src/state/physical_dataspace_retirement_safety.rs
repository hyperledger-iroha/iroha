//! Read-only obligations that must drain before an authorized physical retirement.
//!
//! The native transition owner separately authenticates closure, custody release,
//! target geometry, and the immutable historical tombstone. This module never
//! changes world state and never erases completed receipts, replay ratchets,
//! contract provenance, or signed consensus evidence.

use std::collections::BTreeSet;

use iroha_config::parameters::actual::Nexus;
use iroha_data_model::{
    account::AccountId,
    asset::{AssetBalanceScope, AssetDefinitionId, AssetId},
};
use iroha_model_base::topology::{DataSpaceId, LaneId};

use super::{StorageReadOnly, WorldReadOnly};

/// Refuse retained mutable authority or custody before a physical catalog removal.
///
/// All inputs belong to the authenticated transition owner; a public request must
/// never call this check as a substitute for native closure/release authorization.
/// The first unsafe reference is reported in canonical storage order.
pub(crate) fn ensure_physical_dataspace_retirement_safe(
    target_dataspaces: &BTreeSet<DataSpaceId>,
    target_lanes: &BTreeSet<LaneId>,
    world: &impl WorldReadOnly,
    nexus: &Nexus,
    block_height: u64,
) -> Result<(), String> {
    if target_dataspaces.is_empty() || target_lanes.is_empty() || block_height == 0 {
        return Err(
            "physical retirement requires nonempty targets at a positive block height".into(),
        );
    }
    world.ensure_physical_retirement_privacy_safe(target_dataspaces)?;
    let mut aliases = BTreeSet::new();
    for dataspace in target_dataspaces {
        let entry = nexus.dataspace_catalog.by_id(*dataspace).ok_or_else(|| {
            format!("physical retirement references unknown dataspace {dataspace}")
        })?;
        aliases.insert(entry.alias.as_str());
    }
    let domain_touches =
        |domain: &iroha_model_base::domain::DomainId| aliases.contains(domain.dataspace().as_ref());
    let account_touches = |account: &AccountId| -> Result<bool, String> {
        let entry = world.account_scope_entry(account).map_err(|error| {
            format!("physical retirement cannot resolve account {account}: {error}")
        })?;
        Ok(entry.is_some_and(|entry| entry.iter().any(|(id, _)| target_dataspaces.contains(id))))
    };
    let asset_touches = |asset: &AssetId| matches!(asset.scope(), AssetBalanceScope::Dataspace(id) if target_dataspaces.contains(id));

    // Exact balance scopes remain meaningful even when their owner's aliases
    // have already been cleared during draining.
    for (asset, value) in world.assets().iter() {
        if asset_touches(asset) && !value.as_ref().is_zero() {
            return Err(format!(
                "physical retirement retains nonzero scoped balance {asset}"
            ));
        }
    }
    for (asset, metadata) in world.asset_metadata().iter() {
        if asset_touches(asset) && !metadata.is_empty() {
            return Err(format!(
                "physical retirement retains scoped asset metadata {asset}"
            ));
        }
    }
    for (account, _) in world.accounts().iter() {
        if account_touches(account)? {
            return Err(format!(
                "physical retirement retains account scope {account}"
            ));
        }
    }
    for (alias, _) in world.account_aliases().iter() {
        if target_dataspaces.contains(&alias.dataspace) {
            return Err(format!(
                "physical retirement retains account alias {alias:?}"
            ));
        }
    }
    for (account, entry) in world.account_scope_directory().iter() {
        if entry.iter().any(|(id, _)| target_dataspaces.contains(id)) {
            return Err(format!(
                "physical retirement retains account directory scope {account}"
            ));
        }
    }
    for (uaid, bindings) in world.uaid_dataspaces().iter() {
        if bindings
            .iter()
            .any(|(id, _)| target_dataspaces.contains(id))
        {
            return Err(format!("physical retirement retains UAID binding {uaid}"));
        }
    }
    for (uaid, manifests) in world.space_directory_manifests().iter() {
        if manifests.iter().any(|(id, record)| {
            (target_dataspaces.contains(id) || target_dataspaces.contains(&record.dataspace()))
                && record.lifecycle.expired_epoch.is_none()
                && record.lifecycle.revocation.is_none()
        }) {
            return Err(format!(
                "physical retirement retains active or staged Space Directory manifest {uaid}"
            ));
        }
    }
    for (domain, _) in world.domains().iter() {
        if domain_touches(domain) {
            return Err(format!(
                "physical retirement retains registered domain {domain}"
            ));
        }
    }
    for (domain, _) in world.domain_endorsement_policies().iter() {
        if domain_touches(domain) {
            return Err(format!(
                "physical retirement retains domain endorsement policy {domain}"
            ));
        }
    }
    for (id, _) in world.nfts().iter() {
        if domain_touches(id.domain()) {
            return Err(format!("physical retirement retains registered NFT {id}"));
        }
    }
    for (id, _) in world.rwas().iter() {
        if domain_touches(id.domain()) {
            return Err(format!("physical retirement retains registered RWA {id}"));
        }
    }
    for (namespace, binding) in world.musubi_namespace_bindings().iter() {
        if target_dataspaces.contains(&binding.home_dataspace) {
            return Err(format!(
                "physical retirement retains Musubi namespace binding {namespace}"
            ));
        }
    }
    for (key, record) in world.musubi_packages().iter() {
        if target_dataspaces.contains(&key.home_dataspace)
            || target_dataspaces.contains(&record.package.home_dataspace)
        {
            return Err("physical retirement retains owned Musubi package".into());
        }
    }
    for (alias, _) in world.account_recovery_policies().iter() {
        if target_dataspaces.contains(&alias.dataspace) {
            return Err(format!(
                "physical retirement retains recovery policy {alias:?}"
            ));
        }
    }
    for (alias, request) in world.account_recovery_requests().iter() {
        if (target_dataspaces.contains(&alias.dataspace)
            || target_dataspaces.contains(&request.alias.dataspace))
            && request.status == iroha_data_model::account::AccountRecoveryStatus::Pending
        {
            return Err(format!(
                "physical retirement retains pending recovery {alias:?}"
            ));
        }
    }
    for (alias, record) in world.account_rekey_records().iter() {
        // Previous controllers are historical evidence. A currently resolving
        // stable label is a live reference, including a malformed map binding.
        if (target_dataspaces.contains(&alias.dataspace)
            || target_dataspaces.contains(&record.label.dataspace))
            && (world.account_aliases().get(alias).is_some()
                || world.account_aliases().get(&record.label).is_some())
        {
            return Err(format!(
                "physical retirement retains active rekey label {alias:?}"
            ));
        }
    }

    let mut definitions = BTreeSet::<AssetDefinitionId>::new();
    for (definition, domain) in world.asset_definition_domains().iter() {
        if domain_touches(domain) {
            definitions.insert(definition.clone());
            return Err(format!(
                "physical retirement retains asset domain binding {definition}"
            ));
        }
    }
    for (alias, definition) in world.asset_definition_aliases().iter() {
        if aliases.contains(alias.dataspace_segment()) {
            definitions.insert(definition.clone());
            return Err(format!("physical retirement retains asset alias {alias}"));
        }
    }
    for (definition, binding) in world.asset_definition_alias_bindings().iter() {
        if aliases.contains(binding.alias.dataspace_segment()) {
            return Err(format!(
                "physical retirement retains asset alias binding {definition}"
            ));
        }
    }
    for (alias, _) in world.contract_aliases().iter() {
        if aliases.contains(alias.dataspace_segment()) {
            return Err(format!(
                "physical retirement retains contract alias {alias}"
            ));
        }
    }
    for (address, binding) in world.contract_alias_bindings().iter() {
        if aliases.contains(binding.alias.dataspace_segment()) {
            return Err(format!(
                "physical retirement retains contract alias binding {address}"
            ));
        }
    }
    for (address, _) in world.contract_instances().iter() {
        let dataspace = address.dataspace_id().map_err(|error| {
            format!("physical retirement cannot decode active contract {address}: {error}")
        })?;
        if target_dataspaces.contains(&dataspace) {
            return Err(format!(
                "physical retirement retains active contract {address}"
            ));
        }
    }
    let mut retained_contract_digests = BTreeSet::new();
    for (address, binding) in world.contract_subject_bindings().iter() {
        retained_contract_digests.insert(hex::encode(
            iroha_crypto::Hash::new(address.to_string().as_bytes()).as_ref(),
        ));
        let dataspace = address.dataspace_id().map_err(|error| {
            format!("physical retirement cannot decode retained contract {address}: {error}")
        })?;
        if !target_dataspaces.contains(&dataspace) {
            continue;
        }
        binding
            .validate_for(address)
            .map_err(|error| format!("physical retirement invalid contract binding: {error}"))?;
        if binding.lifecycle.active_code_hash.is_some()
            || binding.lifecycle.pending_owner.is_some()
            || super::super::smartcontracts::code::pending_contract_lifecycle(world, address)
                .map_err(|error| {
                    format!("physical retirement invalid pending contract state: {error}")
                })?
                .is_some()
        {
            return Err(format!(
                "physical retirement retains contract lifecycle work {address}"
            ));
        }
        let digest = hex::encode(iroha_crypto::Hash::new(address.to_string().as_bytes()).as_ref());
        let prefix = format!("sc/{digest}/");
        if world
            .smart_contract_state()
            .iter()
            .any(|(path, _)| path.as_ref().starts_with(&prefix))
        {
            return Err(format!(
                "physical retirement retains mutable contract state {address}"
            ));
        }
    }
    for (path, _) in world.smart_contract_state().iter() {
        let raw: &str = path.as_ref();
        let digest = raw
            .strip_prefix("sc/")
            .and_then(|rest| rest.split_once('/').map(|(digest, _)| digest))
            .or_else(|| raw.strip_prefix("lc/"));
        if let Some(digest) = digest {
            if !retained_contract_digests.contains(digest) {
                return Err(
                    "physical retirement cannot classify orphaned native contract state".into(),
                );
            }
        }
    }
    for (key, _) in world.contract_code_uploads().iter() {
        if target_dataspaces.contains(&key.artifact_id.dataspace_id)
            || account_touches(&key.authority)?
        {
            return Err(format!(
                "physical retirement retains pending contract upload {:?}",
                key.artifact_id
            ));
        }
    }
    for (key, _) in world.contract_code_upload_chunks().iter() {
        if target_dataspaces.contains(&key.upload.artifact_id.dataspace_id)
            || account_touches(&key.upload.authority)?
        {
            return Err(format!(
                "physical retirement retains contract upload chunk {:?}",
                key.upload.artifact_id
            ));
        }
    }
    for (dataspace, policy) in world.axt_policies().iter() {
        if target_dataspaces.contains(dataspace) || target_lanes.contains(&policy.target_lane) {
            return Err(format!(
                "physical retirement retains active AXT policy {dataspace}"
            ));
        }
    }
    // Permanent AXT counters, replay entries and consumed family budgets are
    // retained. No active policy may authorize their reuse after retirement.
    for (dataspace, _) in &nexus.dataspace_fee_sponsor_program_ids {
        if target_dataspaces.contains(dataspace) {
            return Err(format!(
                "physical retirement retains configured fee sponsor for dataspace {dataspace}"
            ));
        }
    }
    for (_, policy) in world.ram_lfe_program_policies().iter() {
        if policy.active && account_touches(&policy.owner)? {
            return Err("physical retirement retains active RAM-LFE policy".into());
        }
    }
    for (_, policy) in world.identifier_policies().iter() {
        if policy.active && account_touches(&policy.owner)? {
            return Err("physical retirement retains active identifier policy".into());
        }
    }
    for (key, program) in world.fee_sponsor_programs().iter() {
        if account_touches(&key.sponsor)? || account_touches(&program.payout_account)? {
            return Err(format!(
                "physical retirement retains fee sponsor program {key}"
            ));
        }
    }
    for (key, _) in world.fee_sponsor_enrollments().iter() {
        if account_touches(&key.beneficiary)? || account_touches(&key.program_id.sponsor)? {
            return Err("physical retirement retains fee sponsor enrollment".into());
        }
    }
    for (key, vault) in world.fee_sponsor_vaults().iter() {
        if !vault.balance.is_zero()
            && (account_touches(&key.program_id.sponsor)?
                || definitions.contains(&key.asset_definition_id))
        {
            return Err("physical retirement retains funded fee sponsor vault".into());
        }
    }

    // A completed record is retained, while every live custody obligation is
    // checked by both the map key and its canonical payload identifiers.
    for (key, record) in world.public_lane_validators().iter() {
        if (target_lanes.contains(&key.0) || target_lanes.contains(&record.lane_id))
            && (record
                .deactivation_height
                .is_none_or(|height| height < record.activation_height || block_height < height)
                || !record.total_stake.is_zero()
                || !record.self_stake.is_zero())
        {
            return Err(format!(
                "physical retirement retains validator tenure on lane {}",
                key.0
            ));
        }
    }
    for (key, share) in world.public_lane_stake_shares().iter() {
        if (target_lanes.contains(&key.0) || target_lanes.contains(&share.lane_id))
            && (!share.bonded.is_zero() || !share.pending_unbonds.is_empty())
        {
            return Err(format!(
                "physical retirement retains stake or unbond on lane {}",
                key.0
            ));
        }
    }
    for ((lane, _), (asset, amount)) in world.public_lane_stake_custody().iter() {
        if !amount.is_zero() && (target_lanes.contains(lane) || asset_touches(asset)) {
            return Err(format!(
                "physical retirement retains exact stake custody on lane {lane}"
            ));
        }
    }
    for ((lane, _, asset), amount) in world.public_lane_reward_accruals().iter() {
        if !amount.is_zero() && (target_lanes.contains(lane) || asset_touches(asset)) {
            return Err(format!(
                "physical retirement retains unpaid reward accrual on lane {lane}"
            ));
        }
    }
    for (key, reward) in world.public_lane_rewards().iter() {
        if !(target_lanes.contains(&key.0)
            || target_lanes.contains(&reward.lane_id)
            || asset_touches(&reward.asset))
        {
            continue;
        }
        if key.0 != reward.lane_id
            || key.1 != reward.epoch
            || reward.shares.iter().any(|share| {
                !share.amount.is_zero()
                    && world
                        .public_lane_reward_claims()
                        .get(&(key.0, share.account.clone()))
                        .and_then(|claim| claim.through_epoch)
                        .is_none_or(|through| through < key.1)
            })
        {
            return Err(format!(
                "physical retirement retains unpaid or malformed reward on lane {}",
                key.0
            ));
        }
    }
    for (asset, amount) in world.public_lane_stake_reserves().iter() {
        if asset_touches(asset) && !amount.is_zero() {
            return Err("physical retirement retains scoped stake reserve".into());
        }
    }
    for (asset, amount) in world.public_lane_reward_reserves().iter() {
        if asset_touches(asset) && !amount.is_zero() {
            return Err("physical retirement retains scoped reward reserve".into());
        }
    }
    let pin_is_released = |pin: &iroha_data_model::da::pin_intent::DaPinIntentWithLocation| {
        world
            .pin_manifests()
            .get(&pin.intent.manifest_hash)
            .is_some_and(|manifest| {
                matches!(
                    manifest.status,
                    iroha_data_model::sorafs::pin_registry::PinStatus::Retired(_)
                )
            })
            && !world.replication_orders().iter().any(|(_, order)| {
                order.manifest_digest == pin.intent.manifest_hash && order.status.is_pending()
            })
    };
    for (_, pin) in world.da_pin_intents_by_ticket().iter() {
        if target_lanes.contains(&pin.intent.lane_id) && !pin_is_released(pin) {
            return Err(format!(
                "physical retirement retains unreleased or unknown DA pin authority on lane {}",
                pin.intent.lane_id
            ));
        }
    }
    for ((lane, epoch, sequence), ticket) in world.da_pin_intents_by_lane_epoch().iter() {
        if target_lanes.contains(lane) {
            let pin = world
                .da_pin_intents_by_ticket()
                .get(ticket)
                .ok_or_else(|| {
                    format!("physical retirement cannot resolve DA pin index on lane {lane}")
                })?;
            if pin.intent.lane_id != *lane
                || pin.intent.epoch != *epoch
                || pin.intent.sequence != *sequence
                || !pin_is_released(pin)
            {
                return Err(format!(
                    "physical retirement retains unreleased or inconsistent DA pin index on lane {lane}"
                ));
            }
        }
    }
    for (_, agreement) in world.repo_agreements().iter() {
        if agreement.settlement_timestamp_ms.is_none()
            && (asset_touches(&agreement.cash_source)
                || asset_touches(&agreement.collateral_custody_asset))
        {
            return Err("physical retirement retains an unsettled scoped repo agreement".into());
        }
    }
    for (_, escrow) in world.asset_escrows().iter() {
        if !escrow.remaining_amount.is_zero()
            && (definitions.contains(&escrow.asset_definition) || account_touches(&escrow.custody)?)
        {
            return Err("physical retirement retains asset escrow custody".into());
        }
    }
    for (_, session) in world.game_sessions().iter() {
        if !session.liability.is_zero()
            && (definitions.contains(&session.asset_definition)
                || account_touches(&session.custody)?)
        {
            return Err("physical retirement retains funded game custody".into());
        }
    }
    Ok(())
}

/// Check typed privacy pool and public-reserve owners without exposing private state keys.
pub(super) fn ensure_privacy_retirement_safe(
    commitments: &impl StorageReadOnly<
        crate::privacy_state::PrivacyCommitmentKeyV1,
        crate::privacy_state::PrivacyStateItemRecordV1,
    >,
    targets: &BTreeSet<DataSpaceId>,
) -> Result<(), String> {
    use crate::privacy_state::{PrivacyCommitmentKeyV1 as Key, PrivacyStateItemRecordV1 as Record};
    for (key, record) in commitments.iter() {
        record.validate().map_err(|error| {
            format!("physical retirement cannot validate retained privacy state: {error}")
        })?;
        if let Some((asset, _)) = record.public_reserve_custody_ref() {
            if matches!(asset.scope(), AssetBalanceScope::Dataspace(id) if targets.contains(id)) {
                return Err(
                    "physical retirement retains privacy public-reserve custody authority".into(),
                );
            }
        }
        match (key, record) {
            (Key::OrchardPoolState { namespace }, Record::OrchardPoolState { state }) => {
                state.validate_bootstrap_binding(*namespace)?;
                if matches!(state.public_balance_scope(), AssetBalanceScope::Dataspace(id) if targets.contains(&id))
                {
                    return Err(
                        "physical retirement retains governed Orchard pool authority".into(),
                    );
                }
            }
            (
                Key::ProofManagedPoolConfig { namespace },
                Record::ProofManagedPoolBootstrap {
                    bootstrap,
                    bootstrap_digest,
                    ..
                },
            ) => {
                if bootstrap.namespace() != *namespace
                    || bootstrap.digest().map_err(|error| {
                        format!(
                            "physical retirement cannot validate private pool bootstrap: {error}"
                        )
                    })? != *bootstrap_digest
                {
                    return Err(
                        "physical retirement private pool bootstrap digest is inconsistent".into(),
                    );
                }
                if matches!(bootstrap.public_balance_scope(), Some(AssetBalanceScope::Dataspace(id)) if targets.contains(&id))
                {
                    return Err(
                        "physical retirement retains governed private-IVM pool authority".into(),
                    );
                }
            }
            (_, Record::OrchardPoolState { .. } | Record::ProofManagedPoolBootstrap { .. }) => {
                return Err(
                    "physical retirement cannot classify a mismatched typed privacy pool key"
                        .into(),
                );
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{World, checked_keypair};
    use iroha_data_model::{
        asset::AssetId,
        nexus::{DataSpaceCatalog, DataSpaceMetadata},
    };
    use iroha_model_base::domain::DomainId;
    use iroha_primitives::numeric::Quantity;

    fn targets() -> (BTreeSet<DataSpaceId>, BTreeSet<LaneId>, Nexus) {
        let id = DataSpaceId::new(7);
        let mut nexus = Nexus::default();
        nexus.dataspace_catalog = DataSpaceCatalog::new(vec![
            DataSpaceMetadata {
                id: DataSpaceId::UNIVERSAL,
                alias: "universal".into(),
                description: None,
                fault_tolerance: 1,
            },
            DataSpaceMetadata {
                id,
                alias: "is".into(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("catalog");
        (
            BTreeSet::from([id]),
            BTreeSet::from([LaneId::new(7)]),
            nexus,
        )
    }

    #[test]
    fn empty_world_is_safe_but_unknown_targets_fail_closed() {
        let (dataspaces, lanes, nexus) = targets();
        let world = World::default();
        assert!(
            ensure_physical_dataspace_retirement_safe(
                &dataspaces,
                &lanes,
                &world.view(),
                &nexus,
                10
            )
            .is_ok()
        );
        assert!(
            ensure_physical_dataspace_retirement_safe(
                &BTreeSet::from([DataSpaceId::new(8)]),
                &lanes,
                &world.view(),
                &nexus,
                10
            )
            .is_err()
        );
    }

    #[test]
    fn scoped_balance_blocks_retirement_without_mutation() {
        let (dataspaces, lanes, nexus) = targets();
        let mut world = World::default();
        let account = AccountId::new(checked_keypair().public_key().clone());
        let definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("issuer", "universal").unwrap(),
            "coin".parse().unwrap(),
        );
        let asset = AssetId::with_scope(
            definition,
            account,
            AssetBalanceScope::Dataspace(DataSpaceId::new(7)),
        );
        world.assets.insert(
            asset.clone(),
            iroha_data_model::common::Owned::new(Quantity::from(3_u32)),
        );
        let error = ensure_physical_dataspace_retirement_safe(
            &dataspaces,
            &lanes,
            &world.view(),
            &nexus,
            10,
        )
        .unwrap_err();
        assert!(error.contains("nonzero scoped balance"));
        assert_eq!(
            world.assets.view().get(&asset).unwrap().as_ref(),
            &Quantity::from(3_u32)
        );
    }

    #[test]
    fn active_policy_is_refused_and_permanent_counter_is_retained() {
        let (dataspaces, lanes, nexus) = targets();
        let mut world = World::default();
        world.axt_policies.insert(
            DataSpaceId::new(7),
            iroha_data_model::nexus::AxtPolicyEntry {
                manifest_root: [1; 32],
                target_lane: LaneId::new(7),
                active_handle_era: 1,
                next_handle_counter: 1,
                current_slot: 1,
            },
        );
        assert!(
            ensure_physical_dataspace_retirement_safe(
                &dataspaces,
                &lanes,
                &world.view(),
                &nexus,
                10
            )
            .unwrap_err()
            .contains("AXT policy")
        );
        {
            let mut block = world.block();
            block.axt_policies.remove(DataSpaceId::new(7));
            block.commit();
        }
        world.axt_handle_counters.insert(
            DataSpaceId::new(7),
            iroha_data_model::nexus::AxtHandleCounterRecord::initial(1),
        );
        assert!(
            ensure_physical_dataspace_retirement_safe(
                &dataspaces,
                &lanes,
                &world.view(),
                &nexus,
                10
            )
            .is_ok()
        );
        assert!(
            world
                .axt_handle_counters
                .view()
                .get(&DataSpaceId::new(7))
                .is_some()
        );
    }
    #[test]
    fn paid_alias_storage_and_historical_artifact_are_preserved() {
        let (dataspaces, lanes, nexus) = targets();
        let mut world = World::default();
        let paid_alias_key = "sns/retained-paid-dataspace-alias".parse().unwrap();
        world
            .smart_contract_state
            .insert(paid_alias_key, vec![1, 2, 3]);
        let artifact = iroha_data_model::smart_contract::ContractArtifactId::new(
            DataSpaceId::new(7),
            iroha_crypto::Hash::new(b"retained-compiled-artifact"),
        );
        world.contract_code.insert(artifact, vec![4, 5, 6]);
        assert!(
            ensure_physical_dataspace_retirement_safe(
                &dataspaces,
                &lanes,
                &world.view(),
                &nexus,
                10
            )
            .is_ok()
        );
        assert_eq!(world.smart_contract_state.view().len(), 1);
        assert_eq!(world.contract_code.view().len(), 1);
    }

    #[test]
    fn orphaned_contract_state_fails_closed_without_erasing_it() {
        let (dataspaces, lanes, nexus) = targets();
        let mut world = World::default();
        let key = format!("sc/{}/balance", "01".repeat(32)).parse().unwrap();
        world.smart_contract_state.insert(key, vec![1]);
        assert!(
            ensure_physical_dataspace_retirement_safe(
                &dataspaces,
                &lanes,
                &world.view(),
                &nexus,
                10
            )
            .unwrap_err()
            .contains("orphaned")
        );
        assert_eq!(world.smart_contract_state.view().len(), 1);
    }
}
