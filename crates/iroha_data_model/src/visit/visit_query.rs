//! Visitor helper functions for queries.

#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};

use super::Visit;
// Alias the `query` module for ergonomic type references within this module.
use crate::query as query_mod;
use crate::{
    prelude::*,
    query::{AnyQueryBox, QueryWithParams, SingularQueryBox},
};

#[cfg(test)]
static SINGULAR_QUERY_FALLBACK_HIT: AtomicBool = AtomicBool::new(false);

fn payload_is_exact_query<Q>(payload: &[u8]) -> bool
where
    Q: norito::codec::Decode + norito::codec::Encode,
{
    let mut input = payload;
    let Ok(query) = <Q as norito::codec::Decode>::decode(&mut input) else {
        return false;
    };
    input.is_empty() && norito::codec::Encode::encode(&query) == payload
}

#[cfg(feature = "fast_dsl")]
fn decode_exact<T>(payload: &[u8]) -> Option<T>
where
    T: norito::codec::Decode + norito::codec::Encode,
{
    let mut input = payload;
    let value = <T as norito::codec::Decode>::decode(&mut input).ok()?;
    (input.is_empty() && norito::codec::Encode::encode(&value) == payload).then_some(value)
}

macro_rules! try_visit_singular_queries {
    ($visitor:expr, $query:expr; $($method:ident($variant:ident)),+ $(,)?) => {
        match $query {
            $(
                SingularQueryBox::$variant(query) => {
                    ($visitor).$method(query);
                    true
                }
            )+
            _ => false,
        }
    };
}

fn try_visit_non_sorafs_singular_query<V: Visit + ?Sized>(
    visitor: &mut V,
    query: &SingularQueryBox,
) -> bool {
    try_visit_singular_queries! {
        visitor, query;
        visit_find_executor_data_model(FindExecutorDataModel),
        visit_find_parameters(FindParameters),
        visit_find_account_by_id(FindAccountById),
        visit_find_account_by_alias(FindAccountByAlias),
        visit_find_aliases_by_account_id(FindAliasesByAccountId),
        visit_find_account_recovery_policy_by_alias(FindAccountRecoveryPolicyByAlias),
        visit_find_account_recovery_request_by_alias(FindAccountRecoveryRequestByAlias),
        visit_find_proof_record_by_id(FindProofRecordById),
        visit_find_contract_manifest_by_code_hash(FindContractManifestByCodeHash),
        visit_find_abi_version(FindAbiVersion),
        visit_find_asset_by_id(FindAssetById),
        visit_find_asset_definition_by_id(FindAssetDefinitionById),
        visit_find_nft_by_id(FindNftById),
        visit_find_trigger_by_id(FindTriggerById),
        visit_find_oracle_feed_by_id(FindOracleFeedById),
        visit_find_oracle_dispute_by_id(FindOracleDisputeById),
        visit_find_oracle_change_by_id(FindOracleChangeById),
        visit_find_oracle_provider_stats_by_key(FindOracleProviderStatsByKey),
        visit_find_twitter_binding_by_hash(FindTwitterBindingByHash),
        visit_find_domain_endorsements(FindDomainEndorsements),
        visit_find_domain_endorsement_policy(FindDomainEndorsementPolicy),
        visit_find_domain_committee(FindDomainCommittee),
        visit_find_da_pin_intent_by_ticket(FindDaPinIntentByTicket),
        visit_find_da_pin_intent_by_manifest(FindDaPinIntentByManifest),
        visit_find_da_pin_intent_by_alias(FindDaPinIntentByAlias),
        visit_find_da_pin_intent_by_lane_epoch_sequence(FindDaPinIntentByLaneEpochSequence),
        visit_find_lane_relay_envelope_by_ref(FindLaneRelayEnvelopeByRef),
        visit_find_dataspace_name_owner_by_id(FindDataspaceNameOwnerById),
        visit_find_musubi_exact_package_v1(FindMusubiExactPackageV1),
        visit_find_musubi_exact_release_v1(FindMusubiExactReleaseV1),
        visit_find_musubi_resolver_index_v1(FindMusubiResolverIndexV1),
        visit_find_musubi_versions_v1(FindMusubiVersionsV1),
        visit_find_musubi_maintainers_v1(FindMusubiMaintainersV1),
        visit_find_musubi_archive_locations_v1(FindMusubiArchiveLocationsV1),
        visit_find_musubi_archive_retention_v1(FindMusubiArchiveRetentionV1),
        visit_find_musubi_alias_v1(FindMusubiAliasV1),
        visit_find_musubi_alias_history_v1(FindMusubiAliasHistoryV1),
        visit_find_musubi_ordered_prefix_v1(FindMusubiOrderedPrefixV1),
        visit_find_domain_by_id(FindDomainById),
        visit_find_fee_sponsor_program_by_id(FindFeeSponsorProgramById),
    }
}

fn try_visit_sorafs_singular_query<V: Visit + ?Sized>(
    visitor: &mut V,
    query: &SingularQueryBox,
) -> bool {
    try_visit_singular_queries! {
        visitor, query;
        visit_find_sorafs_provider_owner(FindSorafsProviderOwner),
        visit_find_sorafs_orderbook_policy(FindSorafsOrderbookPolicy),
        visit_find_sorafs_orderbook_order_by_id(FindSorafsOrderbookOrderById),
        visit_find_sorafs_orderbook_cancellation_by_order_id(FindSorafsOrderbookCancellationByOrderId),
        visit_find_sorafs_orderbook_receipt_by_id(FindSorafsOrderbookReceiptById),
        visit_find_sorafs_orderbook_trade_by_id(FindSorafsOrderbookTradeById),
        visit_find_sorafs_orderbook_channel_by_id(FindSorafsOrderbookChannelById),
        visit_find_sorafs_orderbook_status(FindSorafsOrderbookStatus),
        visit_find_sorafs_orderbook_orders(FindSorafsOrderbookOrders),
        visit_find_sorafs_orderbook_receipts(FindSorafsOrderbookReceipts),
        visit_find_sorafs_orderbook_trades(FindSorafsOrderbookTrades),
        visit_find_sorafs_orderbook_channels(FindSorafsOrderbookChannels),
        visit_find_sorafs_orderbook_events(FindSorafsOrderbookEvents),
        visit_find_sorafs_reserve_policy(FindSorafsReservePolicy),
        visit_find_sorafs_reserve_provider_by_id(FindSorafsReserveProviderById),
        visit_find_sorafs_reserve_movement_by_id(FindSorafsReserveMovementById),
        visit_find_sorafs_reserve_appeal_by_id(FindSorafsReserveAppealById),
        visit_find_sorafs_reserve_providers(FindSorafsReserveProviders),
        visit_find_sorafs_reserve_movements(FindSorafsReserveMovements),
        visit_find_sorafs_reserve_appeals(FindSorafsReserveAppeals),
        visit_find_sorafs_reserve_events(FindSorafsReserveEvents),
        visit_find_sorafs_pop_issuer_policy(FindSorafsPopIssuerPolicy),
        visit_find_sorafs_pop_credential_commitment_by_digest(FindSorafsPopCredentialCommitmentByDigest),
        visit_find_sorafs_pop_commitment_root_by_version(FindSorafsPopCommitmentRootByVersion),
        visit_find_sorafs_pop_revocation_publication_by_version(FindSorafsPopRevocationPublicationByVersion),
        visit_find_sorafs_pop_revocation_by_nonce_commitment(FindSorafsPopRevocationByNonceCommitment),
        visit_find_sorafs_pop_audit_digest_by_sequence(FindSorafsPopAuditDigestBySequence),
        visit_find_sorafs_pop_registry_status(FindSorafsPopRegistryStatus),
        visit_find_sorafs_pin_manifest(FindSorafsPinManifest),
        visit_find_sorafs_repair_task(FindSorafsRepairTask),
        visit_find_sorafs_repair_tasks(FindSorafsRepairTasks),
        visit_find_sorafs_repair_status(FindSorafsRepairStatus),
        visit_find_sorafs_repair_events(FindSorafsRepairEvents),
        visit_find_sorafs_proof_outcome(FindSorafsProofOutcome),
        visit_find_sorafs_proof_outcome_events(FindSorafsProofOutcomeEvents),
        visit_find_sorafs_reputation_journal_authority_policy(FindSorafsReputationJournalAuthorityPolicy),
        visit_find_sorafs_reputation_journal_event_by_source_id(FindSorafsReputationJournalEventBySourceId),
        visit_find_sorafs_reputation_journal_events(FindSorafsReputationJournalEvents),
        visit_find_sorafs_moderation_policy(FindSorafsModerationPolicy),
        visit_find_sorafs_moderation_appeal(FindSorafsModerationAppeal),
        visit_find_sorafs_moderation_juror_eligibility(FindSorafsModerationJurorEligibility),
        visit_find_sorafs_moderation_case(FindSorafsModerationCase),
        visit_find_sorafs_moderation_commit(FindSorafsModerationCommit),
        visit_find_sorafs_moderation_reveal(FindSorafsModerationReveal),
        visit_find_sorafs_moderation_challenge(FindSorafsModerationChallenge),
        visit_find_sorafs_moderation_outcome(FindSorafsModerationOutcome),
        visit_find_sorafs_moderation_no_show(FindSorafsModerationNoShow),
        visit_find_sorafs_moderation_status(FindSorafsModerationStatus),
        visit_find_sorafs_moderation_snapshot(FindSorafsModerationSnapshot),
        visit_find_sorafs_moderation_events(FindSorafsModerationEvents),
    }
}

#[cfg(test)]
fn handle_unvisited_singular_query(visited: bool) {
    if !visited {
        SINGULAR_QUERY_FALLBACK_HIT.store(true, Ordering::Relaxed);
        panic!("singular query fallback matched a variant; add a visit_singular_query arm");
    }
}

#[cfg(not(test))]
fn handle_unvisited_singular_query(_: bool) {}

/// Dispatch a singular query to the matching visitor method.
pub fn visit_singular_query<V: Visit + ?Sized>(visitor: &mut V, query: &SingularQueryBox) {
    let visited = try_visit_non_sorafs_singular_query(visitor, query)
        || try_visit_sorafs_singular_query(visitor, query);
    handle_unvisited_singular_query(visited);
}

#[cfg(not(feature = "fast_dsl"))]
/// Dispatch an iterable query payload (type-erased) to the matching visitor method.
pub fn visit_iter_query<V: Visit + ?Sized>(visitor: &mut V, query_with_params: &QueryWithParams) {
    let any = query_with_params.query.erased_as_any();

    // Some iterable queries intentionally return the same item type. Their
    // erased wrapper therefore cannot be dispatched by `TypeId` alone. Use the
    // preserved, canonical concrete-query payload before falling back to the
    // unparameterized query for that result type.
    if let Some(query) = any.downcast_ref::<query_mod::ErasedIterQuery<crate::account::Account>>() {
        if payload_is_exact_query::<query_mod::account::FindAccountsWithAsset>(query.payload()) {
            return visitor.visit_find_accounts_with_asset(query);
        }
        if payload_is_exact_query::<query_mod::account::FindAccounts>(query.payload()) {
            return visitor.visit_find_accounts(query);
        }
        return;
    }
    if let Some(query) = any.downcast_ref::<query_mod::ErasedIterQuery<crate::role::RoleId>>() {
        if payload_is_exact_query::<query_mod::role::FindRolesByAccountId>(query.payload()) {
            return visitor.visit_find_roles_by_account_id(query);
        }
        if payload_is_exact_query::<query_mod::role::FindRoleIds>(query.payload()) {
            return visitor.visit_find_role_ids(query);
        }
        return;
    }

    macro_rules! try_visit_erased {
        ($($item:ty => $method:ident),+ $(,)?) => {
            $(
                if let Some(q) = any.downcast_ref::<query_mod::ErasedIterQuery<$item>>() {
                    return visitor.$method(q);
                }
            )+
        };
    }

    try_visit_erased! {
        crate::domain::Domain => visit_find_domains,
        crate::asset::value::Asset => visit_find_assets,
        crate::asset::definition::AssetDefinition => visit_find_assets_definitions,
        crate::nft::Nft => visit_find_nfts,
        crate::role::Role => visit_find_roles,
        crate::permission::Permission => visit_find_permissions_by_account_id,
        crate::peer::PeerId => visit_find_peers,
        crate::trigger::TriggerId => visit_find_active_trigger_ids,
        crate::trigger::Trigger => visit_find_triggers,
        crate::oracle::FeedConfig => visit_find_oracle_feeds,
        crate::events::data::oracle::FeedEventRecord => visit_find_oracle_history_by_feed_id,
        crate::oracle::OracleProviderStatsRecord => visit_find_oracle_provider_stats_by_feed_id,
        crate::oracle::OracleDispute => visit_find_oracle_disputes,
        crate::oracle::OracleChangeProposal => visit_find_oracle_changes,
        crate::oracle::TwitterBindingRecord => visit_find_twitter_bindings_by_uaid,
        crate::query::CommittedTransaction => visit_find_transactions,
        crate::block::BlockHeader => visit_find_block_headers,
        crate::block::SignedBlock => visit_find_blocks,
        crate::nexus::FeeSponsorProgram => visit_find_fee_sponsor_programs,
        crate::nexus::FeeSponsorProgramId => visit_find_fee_sponsor_program_ids,
    }
}

#[cfg(feature = "fast_dsl")]
/// Reconstruct and dispatch an iterable query from its fast-DSL components.
pub fn visit_iter_query<V: Visit + ?Sized>(visitor: &mut V, query_with_params: &QueryWithParams) {
    let Some((item, predicate_bytes, selector_bytes, query_payload)) =
        query_with_params.fast_dsl_parts()
    else {
        return;
    };

    macro_rules! visit_erased {
        ($item:ty, $method:ident) => {{
            let Some(predicate) =
                decode_exact::<query_mod::dsl::CompoundPredicate<$item>>(predicate_bytes)
            else {
                return;
            };
            let Some(selector) =
                decode_exact::<query_mod::dsl::SelectorTuple<$item>>(selector_bytes)
            else {
                return;
            };
            let query = query_mod::ErasedIterQuery::<$item>::new(
                predicate,
                selector,
                query_payload.to_vec(),
            );
            visitor.$method(&query);
        }};
    }

    match item {
        query_mod::QueryItemKind::Domain => {
            visit_erased!(crate::domain::Domain, visit_find_domains)
        }
        query_mod::QueryItemKind::Account => {
            if payload_is_exact_query::<query_mod::account::FindAccountsWithAsset>(query_payload) {
                visit_erased!(crate::account::Account, visit_find_accounts_with_asset)
            } else if payload_is_exact_query::<query_mod::account::FindAccounts>(query_payload) {
                visit_erased!(crate::account::Account, visit_find_accounts)
            }
        }
        query_mod::QueryItemKind::Asset => {
            visit_erased!(crate::asset::value::Asset, visit_find_assets)
        }
        query_mod::QueryItemKind::AssetDefinition => visit_erased!(
            crate::asset::definition::AssetDefinition,
            visit_find_assets_definitions
        ),
        query_mod::QueryItemKind::Nft => visit_erased!(crate::nft::Nft, visit_find_nfts),
        query_mod::QueryItemKind::Role => visit_erased!(crate::role::Role, visit_find_roles),
        query_mod::QueryItemKind::RoleId => {
            if payload_is_exact_query::<query_mod::role::FindRolesByAccountId>(query_payload) {
                visit_erased!(crate::role::RoleId, visit_find_roles_by_account_id)
            } else if payload_is_exact_query::<query_mod::role::FindRoleIds>(query_payload) {
                visit_erased!(crate::role::RoleId, visit_find_role_ids)
            }
        }
        query_mod::QueryItemKind::Permission => visit_erased!(
            crate::permission::Permission,
            visit_find_permissions_by_account_id
        ),
        query_mod::QueryItemKind::PeerId => {
            visit_erased!(crate::peer::PeerId, visit_find_peers)
        }
        query_mod::QueryItemKind::TriggerId => {
            visit_erased!(crate::trigger::TriggerId, visit_find_active_trigger_ids)
        }
        query_mod::QueryItemKind::Trigger => {
            visit_erased!(crate::trigger::Trigger, visit_find_triggers)
        }
        query_mod::QueryItemKind::OracleFeedConfig => {
            visit_erased!(crate::oracle::FeedConfig, visit_find_oracle_feeds)
        }
        query_mod::QueryItemKind::OracleFeedEventRecord => visit_erased!(
            crate::events::data::oracle::FeedEventRecord,
            visit_find_oracle_history_by_feed_id
        ),
        query_mod::QueryItemKind::OracleProviderStatsRecord => visit_erased!(
            crate::oracle::OracleProviderStatsRecord,
            visit_find_oracle_provider_stats_by_feed_id
        ),
        query_mod::QueryItemKind::OracleDispute => {
            visit_erased!(crate::oracle::OracleDispute, visit_find_oracle_disputes)
        }
        query_mod::QueryItemKind::OracleChangeProposal => visit_erased!(
            crate::oracle::OracleChangeProposal,
            visit_find_oracle_changes
        ),
        query_mod::QueryItemKind::TwitterBindingRecord => visit_erased!(
            crate::oracle::TwitterBindingRecord,
            visit_find_twitter_bindings_by_uaid
        ),
        query_mod::QueryItemKind::CommittedTransaction => {
            visit_erased!(crate::query::CommittedTransaction, visit_find_transactions)
        }
        query_mod::QueryItemKind::SignedBlock => {
            visit_erased!(crate::block::SignedBlock, visit_find_blocks)
        }
        query_mod::QueryItemKind::BlockHeader => {
            visit_erased!(crate::block::BlockHeader, visit_find_block_headers)
        }
        query_mod::QueryItemKind::FeeSponsorProgram => visit_erased!(
            crate::nexus::FeeSponsorProgram,
            visit_find_fee_sponsor_programs
        ),
        query_mod::QueryItemKind::FeeSponsorProgramId => visit_erased!(
            crate::nexus::FeeSponsorProgramId,
            visit_find_fee_sponsor_program_ids
        ),
        // These item kinds have no visitor hook in `Visit`. Keep this match
        // exhaustive so adding a new query item cannot silently restore the
        // former fast-DSL no-op behavior.
        query_mod::QueryItemKind::AccountId
        | query_mod::QueryItemKind::RepoAgreement
        | query_mod::QueryItemKind::Rwa
        | query_mod::QueryItemKind::ProofRecord
        | query_mod::QueryItemKind::DefiOracleAttestation
        | query_mod::QueryItemKind::AssetEscrowRecord
        | query_mod::QueryItemKind::AnonymousAssetEscrowRecord => {}
    }
}

/// Dispatch a query wrapper to either singular or iterable handlers.
pub fn visit_query<V: Visit + ?Sized>(visitor: &mut V, query: &AnyQueryBox) {
    match query {
        AnyQueryBox::Singular(query) => visitor.visit_singular_query(query),
        AnyQueryBox::Iterable(query) => visitor.visit_iter_query(query),
    }
}

/// Macro generating visitor method signatures for every query variant.
#[macro_export]
macro_rules! query_visitors {
    ($macro:ident) => {
        $macro! {
            // Singular Query visitors
            visit_find_executor_data_model(&FindExecutorDataModel),
            visit_find_parameters(&FindParameters),
            visit_find_account_by_id(&$crate::query::account::FindAccountById),
            visit_find_account_by_alias(&$crate::query::account::FindAccountByAlias),
            visit_find_aliases_by_account_id(&$crate::query::account::FindAliasesByAccountId),
            visit_find_account_recovery_policy_by_alias(
                &$crate::query::account::FindAccountRecoveryPolicyByAlias
            ),
            visit_find_account_recovery_request_by_alias(
                &$crate::query::account::FindAccountRecoveryRequestByAlias
            ),
            visit_find_proof_record_by_id(&$crate::query::proof::FindProofRecordById),
            visit_find_contract_manifest_by_code_hash(
                &$crate::query::smart_contract::FindContractManifestByCodeHash
            ),
            visit_find_abi_version(&$crate::query::runtime::prelude::FindAbiVersion),
            visit_find_asset_by_id(&$crate::query::asset::prelude::FindAssetById),
            visit_find_asset_definition_by_id(
                &$crate::query::asset::prelude::FindAssetDefinitionById
            ),
            visit_find_nft_by_id(&$crate::query::nft::prelude::FindNftById),
            visit_find_trigger_by_id(&$crate::query::trigger::prelude::FindTriggerById),
            visit_find_oracle_feed_by_id(
                &$crate::query::oracle::prelude::FindOracleFeedById
            ),
            visit_find_oracle_dispute_by_id(
                &$crate::query::oracle::prelude::FindOracleDisputeById
            ),
            visit_find_oracle_change_by_id(
                &$crate::query::oracle::prelude::FindOracleChangeById
            ),
            visit_find_oracle_provider_stats_by_key(
                &$crate::query::oracle::prelude::FindOracleProviderStatsByKey
            ),
            visit_find_twitter_binding_by_hash(
                &$crate::query::oracle::prelude::FindTwitterBindingByHash
            ),
            visit_find_domain_endorsements(
                &$crate::query::endorsement::prelude::FindDomainEndorsements
            ),
            visit_find_domain_endorsement_policy(
                &$crate::query::endorsement::prelude::FindDomainEndorsementPolicy
            ),
            visit_find_domain_committee(
                &$crate::query::endorsement::prelude::FindDomainCommittee
            ),
            visit_find_da_pin_intent_by_ticket(
                &$crate::query::da::prelude::FindDaPinIntentByTicket
            ),
            visit_find_da_pin_intent_by_manifest(
                &$crate::query::da::prelude::FindDaPinIntentByManifest
            ),
            visit_find_da_pin_intent_by_alias(
                &$crate::query::da::prelude::FindDaPinIntentByAlias
            ),
            visit_find_da_pin_intent_by_lane_epoch_sequence(
                &$crate::query::da::prelude::FindDaPinIntentByLaneEpochSequence
            ),
            visit_find_lane_relay_envelope_by_ref(
                &$crate::query::nexus::prelude::FindLaneRelayEnvelopeByRef
            ),
            visit_find_sorafs_provider_owner(
                &$crate::query::sorafs::prelude::FindSorafsProviderOwner
            ),
            visit_find_sorafs_orderbook_policy(
                &$crate::query::sorafs::prelude::FindSorafsOrderbookPolicy
            ),
            visit_find_sorafs_orderbook_order_by_id(
                &$crate::query::sorafs::prelude::FindSorafsOrderbookOrderById
            ),
            visit_find_sorafs_orderbook_cancellation_by_order_id(
                &$crate::query::sorafs::prelude::FindSorafsOrderbookCancellationByOrderId
            ),
            visit_find_sorafs_orderbook_receipt_by_id(
                &$crate::query::sorafs::prelude::FindSorafsOrderbookReceiptById
            ),
            visit_find_sorafs_orderbook_trade_by_id(
                &$crate::query::sorafs::prelude::FindSorafsOrderbookTradeById
            ),
            visit_find_sorafs_orderbook_channel_by_id(
                &$crate::query::sorafs::prelude::FindSorafsOrderbookChannelById
            ),
            visit_find_sorafs_orderbook_status(
                &$crate::query::sorafs::prelude::FindSorafsOrderbookStatus
            ),
            visit_find_sorafs_orderbook_orders(
                &$crate::query::sorafs::prelude::FindSorafsOrderbookOrders
            ),
            visit_find_sorafs_orderbook_receipts(
                &$crate::query::sorafs::prelude::FindSorafsOrderbookReceipts
            ),
            visit_find_sorafs_orderbook_trades(
                &$crate::query::sorafs::prelude::FindSorafsOrderbookTrades
            ),
            visit_find_sorafs_orderbook_channels(
                &$crate::query::sorafs::prelude::FindSorafsOrderbookChannels
            ),
            visit_find_sorafs_orderbook_events(
                &$crate::query::sorafs::prelude::FindSorafsOrderbookEvents
            ),
            visit_find_sorafs_reserve_policy(
                &$crate::query::sorafs::prelude::FindSorafsReservePolicy
            ),
            visit_find_sorafs_reserve_provider_by_id(
                &$crate::query::sorafs::prelude::FindSorafsReserveProviderById
            ),
            visit_find_sorafs_reserve_movement_by_id(
                &$crate::query::sorafs::prelude::FindSorafsReserveMovementById
            ),
            visit_find_sorafs_reserve_appeal_by_id(
                &$crate::query::sorafs::prelude::FindSorafsReserveAppealById
            ),
            visit_find_sorafs_reserve_providers(
                &$crate::query::sorafs::prelude::FindSorafsReserveProviders
            ),
            visit_find_sorafs_reserve_movements(
                &$crate::query::sorafs::prelude::FindSorafsReserveMovements
            ),
            visit_find_sorafs_reserve_appeals(
                &$crate::query::sorafs::prelude::FindSorafsReserveAppeals
            ),
            visit_find_sorafs_reserve_events(
                &$crate::query::sorafs::prelude::FindSorafsReserveEvents
            ),
            visit_find_sorafs_pop_issuer_policy(
                &$crate::query::sorafs::prelude::FindSorafsPopIssuerPolicy
            ),
            visit_find_sorafs_pop_credential_commitment_by_digest(
                &$crate::query::sorafs::prelude::FindSorafsPopCredentialCommitmentByDigest
            ),
            visit_find_sorafs_pop_commitment_root_by_version(
                &$crate::query::sorafs::prelude::FindSorafsPopCommitmentRootByVersion
            ),
            visit_find_sorafs_pop_revocation_publication_by_version(
                &$crate::query::sorafs::prelude::FindSorafsPopRevocationPublicationByVersion
            ),
            visit_find_sorafs_pop_revocation_by_nonce_commitment(
                &$crate::query::sorafs::prelude::FindSorafsPopRevocationByNonceCommitment
            ),
            visit_find_sorafs_pop_audit_digest_by_sequence(
                &$crate::query::sorafs::prelude::FindSorafsPopAuditDigestBySequence
            ),
            visit_find_sorafs_pop_registry_status(
                &$crate::query::sorafs::prelude::FindSorafsPopRegistryStatus
            ),
            visit_find_sorafs_pin_manifest(
                &$crate::query::sorafs::prelude::FindSorafsPinManifest
            ),
            visit_find_sorafs_repair_task(
                &$crate::query::sorafs::prelude::FindSorafsRepairTask
            ),
            visit_find_sorafs_repair_tasks(
                &$crate::query::sorafs::prelude::FindSorafsRepairTasks
            ),
            visit_find_sorafs_repair_status(
                &$crate::query::sorafs::prelude::FindSorafsRepairStatus
            ),
            visit_find_sorafs_repair_events(
                &$crate::query::sorafs::prelude::FindSorafsRepairEvents
            ),
            visit_find_sorafs_proof_outcome(
                &$crate::query::sorafs::prelude::FindSorafsProofOutcome
            ),
            visit_find_sorafs_proof_outcome_events(
                &$crate::query::sorafs::prelude::FindSorafsProofOutcomeEvents
            ),
            visit_find_sorafs_reputation_journal_authority_policy(
                &$crate::query::sorafs::prelude::FindSorafsReputationJournalAuthorityPolicy
            ),
            visit_find_sorafs_reputation_journal_event_by_source_id(
                &$crate::query::sorafs::prelude::FindSorafsReputationJournalEventBySourceId
            ),
            visit_find_sorafs_reputation_journal_events(
                &$crate::query::sorafs::prelude::FindSorafsReputationJournalEvents
            ),
            visit_find_sorafs_moderation_policy(
                &$crate::query::sorafs::prelude::FindSorafsModerationPolicy
            ),
            visit_find_sorafs_moderation_appeal(
                &$crate::query::sorafs::prelude::FindSorafsModerationAppeal
            ),
            visit_find_sorafs_moderation_juror_eligibility(
                &$crate::query::sorafs::prelude::FindSorafsModerationJurorEligibility
            ),
            visit_find_sorafs_moderation_case(
                &$crate::query::sorafs::prelude::FindSorafsModerationCase
            ),
            visit_find_sorafs_moderation_commit(
                &$crate::query::sorafs::prelude::FindSorafsModerationCommit
            ),
            visit_find_sorafs_moderation_reveal(
                &$crate::query::sorafs::prelude::FindSorafsModerationReveal
            ),
            visit_find_sorafs_moderation_challenge(
                &$crate::query::sorafs::prelude::FindSorafsModerationChallenge
            ),
            visit_find_sorafs_moderation_outcome(
                &$crate::query::sorafs::prelude::FindSorafsModerationOutcome
            ),
            visit_find_sorafs_moderation_no_show(
                &$crate::query::sorafs::prelude::FindSorafsModerationNoShow
            ),
            visit_find_sorafs_moderation_status(
                &$crate::query::sorafs::prelude::FindSorafsModerationStatus
            ),
            visit_find_sorafs_moderation_snapshot(
                &$crate::query::sorafs::prelude::FindSorafsModerationSnapshot
            ),
            visit_find_sorafs_moderation_events(
                &$crate::query::sorafs::prelude::FindSorafsModerationEvents
            ),
            visit_find_dataspace_name_owner_by_id(
                &$crate::query::sns::prelude::FindDataspaceNameOwnerById
            ),
            visit_find_musubi_exact_package_v1(
                &$crate::query::musubi::prelude::FindMusubiExactPackageV1
            ),
            visit_find_musubi_exact_release_v1(
                &$crate::query::musubi::prelude::FindMusubiExactReleaseV1
            ),
            visit_find_musubi_resolver_index_v1(
                &$crate::query::musubi::prelude::FindMusubiResolverIndexV1
            ),
            visit_find_musubi_versions_v1(
                &$crate::query::musubi::prelude::FindMusubiVersionsV1
            ),
            visit_find_musubi_maintainers_v1(
                &$crate::query::musubi::prelude::FindMusubiMaintainersV1
            ),
            visit_find_musubi_archive_locations_v1(
                &$crate::query::musubi::prelude::FindMusubiArchiveLocationsV1
            ),
            visit_find_musubi_archive_retention_v1(
                &$crate::query::musubi::prelude::FindMusubiArchiveRetentionV1
            ),
            visit_find_musubi_alias_v1(
                &$crate::query::musubi::prelude::FindMusubiAliasV1
            ),
            visit_find_musubi_alias_history_v1(
                &$crate::query::musubi::prelude::FindMusubiAliasHistoryV1
            ),
            visit_find_musubi_ordered_prefix_v1(
                &$crate::query::musubi::prelude::FindMusubiOrderedPrefixV1
            ),
            visit_find_domain_by_id(&$crate::query::domain::FindDomainById),
            visit_find_fee_sponsor_program_by_id(
                &$crate::query::nexus::prelude::FindFeeSponsorProgramById
            ),

            // Iterable Query visitors
            visit_find_domains(&$crate::query::ErasedIterQuery<$crate::domain::Domain>),
            visit_find_accounts(&$crate::query::ErasedIterQuery<$crate::account::Account>),
            visit_find_assets(&$crate::query::ErasedIterQuery<$crate::asset::value::Asset>),
            visit_find_assets_definitions(&$crate::query::ErasedIterQuery<$crate::asset::definition::AssetDefinition>),
            visit_find_nfts(&$crate::query::ErasedIterQuery<$crate::nft::Nft>),
            visit_find_roles(&$crate::query::ErasedIterQuery<$crate::role::Role>),
            visit_find_role_ids(&$crate::query::ErasedIterQuery<$crate::role::RoleId>),
            visit_find_permissions_by_account_id(&$crate::query::ErasedIterQuery<$crate::permission::Permission>),
            visit_find_roles_by_account_id(&$crate::query::ErasedIterQuery<$crate::role::RoleId>),
            visit_find_accounts_with_asset(&$crate::query::ErasedIterQuery<$crate::account::Account>),
            visit_find_peers(&$crate::query::ErasedIterQuery<$crate::peer::PeerId>),
            visit_find_active_trigger_ids(&$crate::query::ErasedIterQuery<$crate::trigger::TriggerId>),
            visit_find_triggers(&$crate::query::ErasedIterQuery<$crate::trigger::Trigger>),
            visit_find_oracle_feeds(&$crate::query::ErasedIterQuery<$crate::oracle::FeedConfig>),
            visit_find_oracle_history_by_feed_id(&$crate::query::ErasedIterQuery<$crate::events::data::oracle::FeedEventRecord>),
            visit_find_oracle_provider_stats_by_feed_id(&$crate::query::ErasedIterQuery<$crate::oracle::OracleProviderStatsRecord>),
            visit_find_oracle_disputes(&$crate::query::ErasedIterQuery<$crate::oracle::OracleDispute>),
            visit_find_oracle_changes(&$crate::query::ErasedIterQuery<$crate::oracle::OracleChangeProposal>),
            visit_find_twitter_bindings_by_uaid(&$crate::query::ErasedIterQuery<$crate::oracle::TwitterBindingRecord>),
            visit_find_transactions(&$crate::query::ErasedIterQuery<$crate::query::CommittedTransaction>),
            visit_find_blocks(&$crate::query::ErasedIterQuery<$crate::block::SignedBlock>),
            visit_find_block_headers(&$crate::query::ErasedIterQuery<$crate::block::BlockHeader>),
            visit_find_fee_sponsor_programs(&$crate::query::ErasedIterQuery<$crate::nexus::FeeSponsorProgram>),
            visit_find_fee_sponsor_program_ids(&$crate::query::ErasedIterQuery<$crate::nexus::FeeSponsorProgramId>),
        }
    };
}

macro_rules! define_query_visitors {
    ( $( $visitor:ident($operation:ty) ),+ $(,)? ) => { $(
        #[doc = concat!("Visit ", stringify!($operation), ".")]
        pub fn $visitor<V: Visit + ?Sized>(_visitor: &mut V, _operation: $operation) {}
    )+ };
}

query_visitors!(define_query_visitors);

#[cfg(test)]
mod tests {
    use std::{
        panic::{AssertUnwindSafe, catch_unwind},
        sync::{Mutex, OnceLock},
    };

    use super::*;
    use crate::{asset::AssetId, prelude::*, query as query_mod, query::parameters::QueryParams};

    fn reset_singular_query_fallback_guard() {
        SINGULAR_QUERY_FALLBACK_HIT.store(false, Ordering::Relaxed);
    }

    fn singular_query_fallback_triggered() -> bool {
        SINGULAR_QUERY_FALLBACK_HIT.load(Ordering::Relaxed)
    }

    fn singular_query_tests_guard() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn assert_singular_query_variant(query: &SingularQueryBox) {
        match query {
            SingularQueryBox::FindExecutorDataModel(_) => {}
            SingularQueryBox::FindParameters(_) => {}
            SingularQueryBox::FindAccountById(_) => {}
            SingularQueryBox::FindAccountByAlias(_) => {}
            SingularQueryBox::FindAliasesByAccountId(_) => {}
            SingularQueryBox::FindAccountRecoveryPolicyByAlias(_) => {}
            SingularQueryBox::FindAccountRecoveryRequestByAlias(_) => {}
            SingularQueryBox::FindProofRecordById(_) => {}
            SingularQueryBox::FindContractManifestByCodeHash(_) => {}
            SingularQueryBox::FindAbiVersion(_) => {}
            SingularQueryBox::FindAssetById(_) => {}
            SingularQueryBox::FindAssetDefinitionById(_) => {}
            SingularQueryBox::FindNftById(_) => {}
            SingularQueryBox::FindAssetEscrowById(_) => {}
            SingularQueryBox::FindAnonymousAssetEscrowById(_) => {}
            SingularQueryBox::FindTriggerById(_) => {}
            SingularQueryBox::FindOracleFeedById(_) => {}
            SingularQueryBox::FindOracleDisputeById(_) => {}
            SingularQueryBox::FindOracleChangeById(_) => {}
            SingularQueryBox::FindOracleProviderStatsByKey(_) => {}
            SingularQueryBox::FindLatestDefiOracleAttestation(_) => {}
            SingularQueryBox::FindTwitterBindingByHash(_) => {}
            SingularQueryBox::FindDaPinIntentByTicket(_) => {}
            SingularQueryBox::FindDaPinIntentByManifest(_) => {}
            SingularQueryBox::FindDaPinIntentByAlias(_) => {}
            SingularQueryBox::FindDaPinIntentByLaneEpochSequence(_) => {}
            SingularQueryBox::FindLaneRelayEnvelopeByRef(_) => {}
            SingularQueryBox::FindSorafsProviderOwner(_) => {}
            SingularQueryBox::FindSorafsOrderbookPolicy(_) => {}
            SingularQueryBox::FindSorafsOrderbookOrderById(_) => {}
            SingularQueryBox::FindSorafsOrderbookCancellationByOrderId(_) => {}
            SingularQueryBox::FindSorafsOrderbookReceiptById(_) => {}
            SingularQueryBox::FindSorafsOrderbookTradeById(_) => {}
            SingularQueryBox::FindSorafsOrderbookChannelById(_) => {}
            SingularQueryBox::FindSorafsOrderbookStatus(_) => {}
            SingularQueryBox::FindSorafsOrderbookOrders(_) => {}
            SingularQueryBox::FindSorafsOrderbookReceipts(_) => {}
            SingularQueryBox::FindSorafsOrderbookTrades(_) => {}
            SingularQueryBox::FindSorafsOrderbookChannels(_) => {}
            SingularQueryBox::FindSorafsOrderbookEvents(_) => {}
            SingularQueryBox::FindSorafsReservePolicy(_) => {}
            SingularQueryBox::FindSorafsReserveProviderById(_) => {}
            SingularQueryBox::FindSorafsReserveMovementById(_) => {}
            SingularQueryBox::FindSorafsReserveAppealById(_) => {}
            SingularQueryBox::FindSorafsReserveProviders(_) => {}
            SingularQueryBox::FindSorafsReserveMovements(_) => {}
            SingularQueryBox::FindSorafsReserveAppeals(_) => {}
            SingularQueryBox::FindSorafsReserveEvents(_) => {}
            SingularQueryBox::FindSorafsPopIssuerPolicy(_) => {}
            SingularQueryBox::FindSorafsPopCredentialCommitmentByDigest(_) => {}
            SingularQueryBox::FindSorafsPopCommitmentRootByVersion(_) => {}
            SingularQueryBox::FindSorafsPopRevocationPublicationByVersion(_) => {}
            SingularQueryBox::FindSorafsPopRevocationByNonceCommitment(_) => {}
            SingularQueryBox::FindSorafsPopAuditDigestBySequence(_) => {}
            SingularQueryBox::FindSorafsPopRegistryStatus(_) => {}
            SingularQueryBox::FindSorafsPinManifest(_) => {}
            SingularQueryBox::FindSorafsRepairTask(_) => {}
            SingularQueryBox::FindSorafsRepairTasks(_) => {}
            SingularQueryBox::FindSorafsRepairStatus(_) => {}
            SingularQueryBox::FindSorafsRepairEvents(_) => {}
            SingularQueryBox::FindSorafsProofOutcome(_) => {}
            SingularQueryBox::FindSorafsProofOutcomeEvents(_) => {}
            SingularQueryBox::FindSorafsReputationJournalAuthorityPolicy(_) => {}
            SingularQueryBox::FindSorafsReputationJournalEventBySourceId(_) => {}
            SingularQueryBox::FindSorafsReputationJournalEvents(_) => {}
            SingularQueryBox::FindSorafsModerationPolicy(_) => {}
            SingularQueryBox::FindSorafsModerationAppeal(_) => {}
            SingularQueryBox::FindSorafsModerationJurorEligibility(_) => {}
            SingularQueryBox::FindSorafsModerationCase(_) => {}
            SingularQueryBox::FindSorafsModerationCommit(_) => {}
            SingularQueryBox::FindSorafsModerationReveal(_) => {}
            SingularQueryBox::FindSorafsModerationChallenge(_) => {}
            SingularQueryBox::FindSorafsModerationOutcome(_) => {}
            SingularQueryBox::FindSorafsModerationNoShow(_) => {}
            SingularQueryBox::FindSorafsModerationStatus(_) => {}
            SingularQueryBox::FindSorafsModerationSnapshot(_) => {}
            SingularQueryBox::FindSorafsModerationEvents(_) => {}
            SingularQueryBox::FindDataspaceNameOwnerById(_) => {}
            SingularQueryBox::FindMusubiExactPackageV1(_) => {}
            SingularQueryBox::FindMusubiExactReleaseV1(_) => {}
            SingularQueryBox::FindMusubiResolverIndexV1(_) => {}
            SingularQueryBox::FindMusubiVersionsV1(_) => {}
            SingularQueryBox::FindMusubiMaintainersV1(_) => {}
            SingularQueryBox::FindMusubiArchiveLocationsV1(_) => {}
            SingularQueryBox::FindMusubiArchiveRetentionV1(_) => {}
            SingularQueryBox::FindMusubiAliasV1(_) => {}
            SingularQueryBox::FindMusubiAliasHistoryV1(_) => {}
            SingularQueryBox::FindMusubiOrderedPrefixV1(_) => {}
            SingularQueryBox::FindDomainById(_) => {}
            SingularQueryBox::FindFeeSponsorProgramById(_) => {}
            SingularQueryBox::FindFxCorridorPolicyRegistry(_) => {}
            SingularQueryBox::FindFxCorridorPolicyById(_) => {}
            SingularQueryBox::FindDomainEndorsements(_) => {}
            SingularQueryBox::FindDomainEndorsementPolicy(_) => {}
            SingularQueryBox::FindDomainCommittee(_) => {}
            #[cfg(test)]
            SingularQueryBox::__TestFallback => {}
        }
    }

    struct CountingVisitor {
        params: usize,
        domains: usize,
        roles_by_account: usize,
        accounts_with_asset: usize,
    }

    impl Visit for CountingVisitor {
        fn visit_find_parameters(&mut self, _: &FindParameters) {
            self.params += 1;
        }

        fn visit_find_domains(&mut self, _: &query_mod::ErasedIterQuery<crate::domain::Domain>) {
            self.domains += 1;
        }

        fn visit_find_roles_by_account_id(
            &mut self,
            _: &query_mod::ErasedIterQuery<crate::role::RoleId>,
        ) {
            self.roles_by_account += 1;
        }

        fn visit_find_accounts_with_asset(
            &mut self,
            _: &query_mod::ErasedIterQuery<crate::account::Account>,
        ) {
            self.accounts_with_asset += 1;
        }
    }

    struct NoopVisitor;

    impl Visit for NoopVisitor {}

    #[derive(Default)]
    struct MusubiVisitor {
        seen: [bool; 10],
    }

    impl Visit for MusubiVisitor {
        fn visit_find_musubi_exact_package_v1(
            &mut self,
            _: &query_mod::musubi::FindMusubiExactPackageV1,
        ) {
            self.seen[0] = true;
        }

        fn visit_find_musubi_exact_release_v1(
            &mut self,
            _: &query_mod::musubi::FindMusubiExactReleaseV1,
        ) {
            self.seen[1] = true;
        }

        fn visit_find_musubi_resolver_index_v1(
            &mut self,
            _: &query_mod::musubi::FindMusubiResolverIndexV1,
        ) {
            self.seen[2] = true;
        }

        fn visit_find_musubi_versions_v1(&mut self, _: &query_mod::musubi::FindMusubiVersionsV1) {
            self.seen[3] = true;
        }

        fn visit_find_musubi_maintainers_v1(
            &mut self,
            _: &query_mod::musubi::FindMusubiMaintainersV1,
        ) {
            self.seen[4] = true;
        }

        fn visit_find_musubi_archive_locations_v1(
            &mut self,
            _: &query_mod::musubi::FindMusubiArchiveLocationsV1,
        ) {
            self.seen[5] = true;
        }

        fn visit_find_musubi_archive_retention_v1(
            &mut self,
            _: &query_mod::musubi::FindMusubiArchiveRetentionV1,
        ) {
            self.seen[6] = true;
        }

        fn visit_find_musubi_alias_v1(&mut self, _: &query_mod::musubi::FindMusubiAliasV1) {
            self.seen[7] = true;
        }

        fn visit_find_musubi_alias_history_v1(
            &mut self,
            _: &query_mod::musubi::FindMusubiAliasHistoryV1,
        ) {
            self.seen[8] = true;
        }

        fn visit_find_musubi_ordered_prefix_v1(
            &mut self,
            _: &query_mod::musubi::FindMusubiOrderedPrefixV1,
        ) {
            self.seen[9] = true;
        }
    }

    fn musubi_v1_singular_queries() -> Vec<SingularQueryBox> {
        use crate::musubi::{
            ArchiveId, MusubiAliasNameV1, MusubiAliasQueryV1, MusubiArchiveLocationQueryV1,
            MusubiArchiveRetentionQueryV1, MusubiExactPackageQueryV1, MusubiExactReleaseQueryV1,
            MusubiOrderedPrefixQueryV1, MusubiOrderedPrefixV1, MusubiPackageIdV1,
            MusubiPackageNameV1, MusubiPackagePageQueryV1, MusubiPackageScopeV1,
            MusubiPageRequestV1, MusubiReleaseIdV1, MusubiResolverIndexQueryV1, MusubiVersionV1,
        };

        let package = MusubiPackageIdV1::new(
            DataSpaceId::new(7),
            MusubiPackageScopeV1::DataspaceRoot,
            MusubiPackageNameV1::new("ledger-tools").expect("package name"),
        );
        let release = MusubiReleaseIdV1::new(
            package.clone(),
            "1.2.3".parse::<MusubiVersionV1>().expect("version"),
        );
        let alias = "ledger".parse::<MusubiAliasNameV1>().expect("alias");
        let page = || MusubiPageRequestV1 {
            limit: 50,
            cursor: None,
        };

        vec![
            query_mod::musubi::FindMusubiExactPackageV1::new(MusubiExactPackageQueryV1 {
                package: package.clone(),
            })
            .into(),
            query_mod::musubi::FindMusubiExactReleaseV1::new(MusubiExactReleaseQueryV1 { release })
                .into(),
            query_mod::musubi::FindMusubiResolverIndexV1::new(MusubiResolverIndexQueryV1 {
                package: package.clone(),
                requirement: None,
                page: page(),
            })
            .into(),
            query_mod::musubi::FindMusubiVersionsV1::new(MusubiPackagePageQueryV1 {
                package: package.clone(),
                page: page(),
            })
            .into(),
            query_mod::musubi::FindMusubiMaintainersV1::new(MusubiPackagePageQueryV1 {
                package,
                page: page(),
            })
            .into(),
            query_mod::musubi::FindMusubiArchiveLocationsV1::new(MusubiArchiveLocationQueryV1 {
                archive_id: ArchiveId::new([0xA5; 32]),
                page: page(),
            })
            .into(),
            query_mod::musubi::FindMusubiArchiveRetentionV1::new(MusubiArchiveRetentionQueryV1 {
                archive_ids: vec![ArchiveId::new([0xA5; 32])],
                expected_snapshot: None,
            })
            .into(),
            query_mod::musubi::FindMusubiAliasV1::new(MusubiAliasQueryV1 {
                alias: alias.clone(),
                page: page(),
            })
            .into(),
            query_mod::musubi::FindMusubiAliasHistoryV1::new(MusubiAliasQueryV1 {
                alias,
                page: page(),
            })
            .into(),
            query_mod::musubi::FindMusubiOrderedPrefixV1::new(MusubiOrderedPrefixQueryV1 {
                prefix: MusubiOrderedPrefixV1::new("7/").expect("ordered prefix"),
                page: page(),
            })
            .into(),
        ]
    }

    const ALICE_ACCOUNT_ID_STR: &str = "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE";

    fn query_with_default_params(
        query: QueryBox<query_mod::QueryOutputBatchBox>,
    ) -> QueryWithParams {
        #[cfg(feature = "fast_dsl")]
        {
            QueryWithParams::new(&query, QueryParams::default())
        }
        #[cfg(not(feature = "fast_dsl"))]
        {
            QueryWithParams::new(query, QueryParams::default())
        }
    }

    #[test]
    fn visit_find_parameters_dispatches() {
        let mut visitor = CountingVisitor {
            params: 0,
            domains: 0,
            roles_by_account: 0,
            accounts_with_asset: 0,
        };
        let query = AnyQueryBox::Singular(SingularQueryBox::FindParameters(FindParameters));
        visit_query(&mut visitor, &query);
        assert_eq!(visitor.params, 1);
    }

    #[test]
    fn musubi_v1_singular_query_inventory_dispatches_every_typed_hook() {
        let queries = musubi_v1_singular_queries();
        assert_eq!(queries.len(), 10);

        let mut visitor = MusubiVisitor::default();
        for query in &queries {
            assert_singular_query_variant(query);
            visit_singular_query(&mut visitor, query);
        }

        assert_eq!(visitor.seen, [true; 10]);
    }

    #[test]
    fn visit_find_domains_dispatches() {
        let mut visitor = CountingVisitor {
            params: 0,
            domains: 0,
            roles_by_account: 0,
            accounts_with_asset: 0,
        };
        let boxed: QueryBox<query_mod::QueryOutputBatchBox> =
            Box::new(query_mod::ErasedIterQuery::<crate::domain::Domain>::new(
                CompoundPredicate::<crate::domain::Domain>::PASS,
                SelectorTuple::<crate::domain::Domain>::default(),
                norito::codec::Encode::encode(&FindDomains),
            ));
        let query = AnyQueryBox::Iterable(query_with_default_params(boxed));
        visit_query(&mut visitor, &query);
        assert_eq!(visitor.domains, 1);
    }

    #[test]
    fn visit_parameterized_iterable_queries_dispatches_distinct_policy_hooks() {
        let account_id = AccountId::parse_encoded(ALICE_ACCOUNT_ID_STR)
            .map(crate::account::ParsedAccountId::into_account_id)
            .expect("valid account id");
        let asset_definition = crate::asset::AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").expect("valid domain id"),
            "rose".parse().expect("valid asset name"),
        );
        let roles_payload = norito::codec::Encode::encode(&query_mod::role::FindRolesByAccountId {
            id: account_id,
        });
        let accounts_payload =
            norito::codec::Encode::encode(&query_mod::account::FindAccountsWithAsset {
                asset_definition,
            });
        let roles = AnyQueryBox::Iterable(query_with_default_params(Box::new(
            query_mod::ErasedIterQuery::<crate::role::RoleId>::new(
                CompoundPredicate::PASS,
                SelectorTuple::default(),
                roles_payload,
            ),
        )));
        let accounts = AnyQueryBox::Iterable(query_with_default_params(Box::new(
            query_mod::ErasedIterQuery::<crate::account::Account>::new(
                CompoundPredicate::PASS,
                SelectorTuple::default(),
                accounts_payload,
            ),
        )));
        let mut visitor = CountingVisitor {
            params: 0,
            domains: 0,
            roles_by_account: 0,
            accounts_with_asset: 0,
        };

        visit_query(&mut visitor, &roles);
        visit_query(&mut visitor, &accounts);

        assert_eq!(visitor.roles_by_account, 1);
        assert_eq!(visitor.accounts_with_asset, 1);
    }

    #[cfg(feature = "fast_dsl")]
    #[test]
    fn fast_dsl_iterable_visitor_rejects_noncanonical_components() {
        let query = AnyQueryBox::Iterable(QueryWithParams {
            query: (),
            query_payload: norito::codec::Encode::encode(&query_mod::domain::FindDomains),
            item: query_mod::QueryItemKind::Domain,
            predicate_bytes: vec![0xFF; 4],
            selector_bytes: norito::codec::Encode::encode(
                &SelectorTuple::<crate::domain::Domain>::default(),
            ),
            params: QueryParams::default(),
        });
        let mut visitor = CountingVisitor {
            params: 0,
            domains: 0,
            roles_by_account: 0,
            accounts_with_asset: 0,
        };

        visit_query(&mut visitor, &query);

        assert_eq!(visitor.domains, 0);
    }

    #[test]
    fn singular_query_fallback_never_triggers_for_known_variants() {
        let _guard = singular_query_tests_guard();
        reset_singular_query_fallback_guard();
        let mut visitor = NoopVisitor;

        let proof_id = crate::proof::ProofId {
            backend: "test.backend".into(),
            proof_hash: [0x11; 32],
        };
        let manifest_hash = iroha_crypto::Hash::prehashed([0u8; iroha_crypto::Hash::LENGTH]);
        let account_id = AccountId::parse_encoded(ALICE_ACCOUNT_ID_STR)
            .map(crate::account::ParsedAccountId::into_account_id)
            .expect("valid account id");
        let asset_definition: crate::asset::AssetDefinitionId =
            iroha_data_model::asset::AssetDefinitionId::derive_from_components(
                DomainId::try_new("wonderland", "universal").unwrap(),
                "rose".parse().unwrap(),
            );
        let asset_id = AssetId::new(asset_definition, account_id.clone());
        let nft_id: NftId = "ticket$wonderland.universal".parse().expect("valid NFT id");
        let queries = vec![
            SingularQueryBox::FindExecutorDataModel(FindExecutorDataModel),
            SingularQueryBox::FindParameters(FindParameters),
            SingularQueryBox::FindAccountById(
                crate::query::account::prelude::FindAccountById::new(account_id.clone()),
            ),
            SingularQueryBox::FindAccountByAlias(
                crate::query::account::prelude::FindAccountByAlias::new(
                    crate::account::AccountAlias::domainless(
                        "alice".parse().expect("alias label"),
                        crate::nexus::DataSpaceId::UNIVERSAL,
                    ),
                ),
            ),
            SingularQueryBox::FindAliasesByAccountId(
                crate::query::account::prelude::FindAliasesByAccountId::new(
                    account_id.clone(),
                    Some("centralbank".to_owned()),
                    Some("banka".to_owned()),
                ),
            ),
            SingularQueryBox::FindProofRecordById(
                crate::query::proof::prelude::FindProofRecordById { id: proof_id },
            ),
            SingularQueryBox::FindContractManifestByCodeHash(
                crate::query::smart_contract::prelude::FindContractManifestByCodeHash {
                    code_hash: manifest_hash,
                },
            ),
            SingularQueryBox::FindAbiVersion(crate::query::runtime::prelude::FindAbiVersion),
            SingularQueryBox::FindAssetById(crate::query::asset::prelude::FindAssetById::new(
                asset_id.clone(),
            )),
            SingularQueryBox::FindAssetDefinitionById(
                crate::query::asset::prelude::FindAssetDefinitionById::new(
                    asset_id.definition().clone(),
                ),
            ),
            SingularQueryBox::FindNftById(crate::query::nft::prelude::FindNftById::new(nft_id)),
            SingularQueryBox::FindTriggerById(
                crate::query::trigger::prelude::FindTriggerById::new(
                    "demo_trigger".parse().expect("valid trigger id"),
                ),
            ),
            SingularQueryBox::FindSorafsProofOutcome(
                crate::query::sorafs::prelude::FindSorafsProofOutcome::new(
                    crate::sorafs::proof_ledger::ProofOutcomeKindV1::Pdp,
                    [0x51; 32],
                    None,
                ),
            ),
            SingularQueryBox::FindSorafsPinManifest(
                crate::query::sorafs::prelude::FindSorafsPinManifest::new(
                    crate::sorafs::pin_registry::ManifestDigest::new([0x52; 32]),
                    None,
                ),
            ),
            SingularQueryBox::FindSorafsReputationJournalEventBySourceId(
                crate::query::sorafs::prelude::FindSorafsReputationJournalEventBySourceId::new(
                    crate::sorafs::reputation::ReputationJournalSourceIdV1([0x53; 32]),
                    None,
                ),
            ),
            SingularQueryBox::FindDomainById(crate::query::domain::prelude::FindDomainById::new(
                DomainId::try_new("wonderland", "universal").expect("valid domain id"),
            )),
        ];

        for query in &queries {
            assert_singular_query_variant(query);
            visit_singular_query(&mut visitor, query);
        }
        assert!(
            !singular_query_fallback_triggered(),
            "singular query fallback matched a variant; add a visitor arm",
        );
    }

    #[test]
    fn singular_query_fallback_panics_for_missing_visitor() {
        let _guard = singular_query_tests_guard();
        reset_singular_query_fallback_guard();
        let mut visitor = NoopVisitor;

        let panic_payload = catch_unwind(AssertUnwindSafe(|| {
            visit_singular_query(&mut visitor, &SingularQueryBox::__TestFallback);
        }))
        .expect_err("singular query fallback should panic when visitor is missing");

        let panic_message = panic_payload
            .downcast_ref::<String>()
            .map(String::as_str)
            .or_else(|| panic_payload.downcast_ref::<&'static str>().copied())
            .unwrap_or_default();

        assert!(
            panic_message.contains(
                "singular query fallback matched a variant; add a visit_singular_query arm"
            ),
            "unexpected panic message: {panic_message}"
        );
        assert!(
            singular_query_fallback_triggered(),
            "singular query fallback flag was not set"
        );
        reset_singular_query_fallback_guard();
    }
}
