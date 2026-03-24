//! Visitor helper functions for queries.

#[cfg(test)]
use std::sync::atomic::{AtomicBool, Ordering};

use super::Visit;
// Alias the `query` module for ergonomic type references within this module.
#[cfg(not(feature = "fast_dsl"))]
use crate::query as query_mod;
use crate::{
    prelude::*,
    query::{AnyQueryBox, QueryWithParams, SingularQueryBox},
};

#[cfg(test)]
static SINGULAR_QUERY_FALLBACK_HIT: AtomicBool = AtomicBool::new(false);

/// Dispatch a singular query to the matching visitor method.
pub fn visit_singular_query<V: Visit + ?Sized>(visitor: &mut V, query: &SingularQueryBox) {
    macro_rules! singular_query_visitors {
        ( $($visitor:ident($query:ident)),+ $(,)? ) => {
            #[allow(unreachable_patterns)]
            match query {
                $( SingularQueryBox::$query(query) => visitor.$visitor(&query), )+
                // Fallback for newly added singular queries to keep compilation stable in tests
                _ => {
                    #[cfg(test)]
                    {
                        SINGULAR_QUERY_FALLBACK_HIT.store(true, Ordering::Relaxed);
                        panic!(
                            "singular query fallback matched a variant; add a visit_singular_query arm"
                        );
                    }
                }
            }
        };
    }

    singular_query_visitors! {
        visit_find_executor_data_model(FindExecutorDataModel),
        visit_find_parameters(FindParameters),
        visit_find_domains_by_account_id(FindDomainsByAccountId),
        visit_find_account_ids_by_domain_id(FindAccountIdsByDomainId),
        visit_find_proof_record_by_id(FindProofRecordById),
        visit_find_contract_manifest_by_code_hash(FindContractManifestByCodeHash),
        visit_find_abi_version(FindAbiVersion),
        visit_find_asset_by_id(FindAssetById),
        visit_find_asset_definition_by_id(FindAssetDefinitionById),
        visit_find_twitter_binding_by_hash(FindTwitterBindingByHash),
        visit_find_da_pin_intent_by_ticket(FindDaPinIntentByTicket),
        visit_find_da_pin_intent_by_manifest(FindDaPinIntentByManifest),
        visit_find_da_pin_intent_by_alias(FindDaPinIntentByAlias),
        visit_find_da_pin_intent_by_lane_epoch_sequence(FindDaPinIntentByLaneEpochSequence),
        visit_find_sorafs_provider_owner(FindSorafsProviderOwner),
    }
}

#[cfg(not(feature = "fast_dsl"))]
/// Dispatch an iterable query payload (type-erased) to the matching visitor method.
pub fn visit_iter_query<V: Visit + ?Sized>(visitor: &mut V, query_with_params: &QueryWithParams) {
    let any = query_with_params.query.erased_as_any();

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
        crate::account::Account => visit_find_accounts,
        crate::asset::value::Asset => visit_find_assets,
        crate::asset::definition::AssetDefinition => visit_find_assets_definitions,
        crate::nft::Nft => visit_find_nfts,
        crate::role::Role => visit_find_roles,
        crate::role::RoleId => visit_find_role_ids,
        crate::permission::Permission => visit_find_permissions_by_account_id,
        crate::role::RoleId => visit_find_roles_by_account_id,
        crate::account::Account => visit_find_accounts_with_asset,
        crate::peer::PeerId => visit_find_peers,
        crate::trigger::TriggerId => visit_find_active_trigger_ids,
        crate::trigger::Trigger => visit_find_triggers,
        crate::query::CommittedTransaction => visit_find_transactions,
        crate::block::BlockHeader => visit_find_block_headers,
        crate::block::SignedBlock => visit_find_blocks,
    }
}

#[cfg(feature = "fast_dsl")]
/// No-op iterable visitor used when the fast DSL omits query payloads.
pub fn visit_iter_query<V: Visit + ?Sized>(_visitor: &mut V, _query_with_params: &QueryWithParams) {
    // In fast DSL mode, the iterable query payload is not carried in `QueryWithParams`.
    // We cannot perform type-based dispatch here, so this is a no-op.
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
            visit_find_domains_by_account_id(&$crate::query::account::FindDomainsByAccountId),
            visit_find_account_ids_by_domain_id(
                &$crate::query::domain::FindAccountIdsByDomainId
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
            visit_find_twitter_binding_by_hash(
                &$crate::query::oracle::prelude::FindTwitterBindingByHash
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
            visit_find_sorafs_provider_owner(
                &$crate::query::sorafs::prelude::FindSorafsProviderOwner
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
            visit_find_transactions(&$crate::query::ErasedIterQuery<$crate::query::CommittedTransaction>),
            visit_find_blocks(&$crate::query::ErasedIterQuery<$crate::block::SignedBlock>),
            visit_find_block_headers(&$crate::query::ErasedIterQuery<$crate::block::BlockHeader>),
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

#[cfg(all(test, not(feature = "fast_dsl")))]
mod tests {
    use std::{
        panic::{AssertUnwindSafe, catch_unwind},
        sync::{Mutex, OnceLock},
    };

    use super::*;
    use crate::{
        QueryWithFilter, asset::AssetId, prelude::*, query as query_mod,
        query::parameters::QueryParams,
    };

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
            SingularQueryBox::FindDomainsByAccountId(_) => {}
            SingularQueryBox::FindAccountIdsByDomainId(_) => {}
            SingularQueryBox::FindProofRecordById(_) => {}
            SingularQueryBox::FindContractManifestByCodeHash(_) => {}
            SingularQueryBox::FindAbiVersion(_) => {}
            SingularQueryBox::FindAssetById(_) => {}
            SingularQueryBox::FindAssetDefinitionById(_) => {}
            SingularQueryBox::FindTwitterBindingByHash(_) => {}
            SingularQueryBox::FindDaPinIntentByTicket(_) => {}
            SingularQueryBox::FindDaPinIntentByManifest(_) => {}
            SingularQueryBox::FindDaPinIntentByAlias(_) => {}
            SingularQueryBox::FindDaPinIntentByLaneEpochSequence(_) => {}
            SingularQueryBox::FindSorafsProviderOwner(_) => {}
            SingularQueryBox::FindDataspaceNameOwnerById(_) => {}
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
    }

    impl Visit for CountingVisitor {
        fn visit_find_parameters(&mut self, _: &FindParameters) {
            self.params += 1;
        }

        fn visit_find_domains(&mut self, _: &query_mod::ErasedIterQuery<crate::domain::Domain>) {
            self.domains += 1;
        }
    }

    struct NoopVisitor;

    impl Visit for NoopVisitor {}

    const ALICE_ACCOUNT_ID_STR: &str = "6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw";

    #[test]
    fn visit_find_parameters_dispatches() {
        let mut visitor = CountingVisitor {
            params: 0,
            domains: 0,
        };
        let query = AnyQueryBox::Singular(SingularQueryBox::FindParameters(FindParameters));
        visit_query(&mut visitor, &query);
        assert_eq!(visitor.params, 1);
    }

    #[test]
    fn visit_find_domains_dispatches() {
        let mut visitor = CountingVisitor {
            params: 0,
            domains: 0,
        };
        let query = AnyQueryBox::Iterable(QueryWithParams::new(
            {
                let with_filter = QueryWithFilter::new(
                    Box::new(FindDomains),
                    CompoundPredicate::<crate::domain::Domain>::PASS,
                    SelectorTuple::<crate::domain::Domain>::default(),
                );
                let boxed: QueryBox<_> = with_filter.into();
                boxed
            },
            QueryParams::default(),
        ));
        visit_query(&mut visitor, &query);
        assert_eq!(visitor.domains, 1);
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
            iroha_data_model::asset::AssetDefinitionId::new(
                "wonderland".parse().unwrap(),
                "rose".parse().unwrap(),
            );
        let asset_id = AssetId::new(asset_definition, account_id.clone());
        let domain_id: crate::domain::DomainId = "wonderland".parse().expect("valid domain");

        let queries = vec![
            SingularQueryBox::FindExecutorDataModel(FindExecutorDataModel),
            SingularQueryBox::FindParameters(FindParameters),
            SingularQueryBox::FindDomainsByAccountId(
                crate::query::account::prelude::FindDomainsByAccountId::new(account_id.clone()),
            ),
            SingularQueryBox::FindAccountIdsByDomainId(
                crate::query::domain::prelude::FindAccountIdsByDomainId::new(domain_id),
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
