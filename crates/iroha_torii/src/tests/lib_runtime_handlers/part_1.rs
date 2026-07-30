    use std::{
        collections::HashSet,
        net::SocketAddr,
        num::{NonZeroU32, NonZeroU64, NonZeroUsize},
        sync::{
            Arc, LazyLock, Mutex, MutexGuard,
            atomic::{AtomicUsize, Ordering},
        },
        time::{Duration, Instant},
    };

    use axum::{
        extract::State,
        http::{HeaderMap, HeaderValue, StatusCode},
        response::IntoResponse,
        routing::any,
    };
    use base64::Engine as _;
    use futures::executor;
    use iroha_config::{
        client_api::ConfigGetDTO,
        parameters::{
            actual::{NoritoRpcStage, NoritoRpcTransport, TelemetryProfile},
            defaults,
        },
    };
    use iroha_core::{
        kiso::KisoHandle,
        kura::Kura,
        query::store::LiveQueryStore,
        queue::{LaneRouter, Queue, RoutingDecision, RoutingResolveError, TransactionRoutingView},
        smartcontracts::Execute,
        state::{State as IrohaState, World},
        sumeragi::{
            consensus::{PERMISSIONED_TAG, Phase, Vote, vote_preimage},
            status::record_commit_qc_for_tests,
        },
        tx::AcceptedTransaction,
    };
    use iroha_crypto::{Algorithm, Hash, KeyPair, Signature, SignatureOf};
    use iroha_data_model::{
        ChainId, Registrable, ValidationFail,
        account::{Account, AccountAlias, AccountId, OpaqueAccountId},
        asset::{Asset, AssetDefinition, AssetDefinitionId, AssetId},
        block::{
            BlockHeader, BlockSignature, SignedBlock,
            consensus_v2::{
                BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
                ExecutionCommitment, GlobalPhase, HeightContext, PROTOCOL_VERSION, PayloadEncoding,
                QuorumCertificate, ValidatorPower, finality::V2FinalityArtifact,
            },
        },
        consensus::{
            ConsensusKeyId, ConsensusKeyRecord, ConsensusKeyRole, ConsensusKeyStatus, Qc,
            QcAggregate, VALIDATOR_SET_HASH_VERSION_V1,
        },
        domain::{Domain, DomainId},
        events::{
            pipeline::{BlockEvent, BlockStatus, TransactionEvent, TransactionStatus},
            trigger_completed::{TriggerCompletedEvent, TriggerCompletedOutcome},
        },
        isi::{Grant, Log, Register, RegisterPeerWithPop, consensus_keys::RegisterConsensusKey},
        level::Level,
        name::Name,
        nexus::{AxtPolicySnapshot, AxtRejectReason, DataSpaceId, LaneId, UniversalAccountId},
        parameter::{Parameter, system::SumeragiNposParameters},
        peer::{Peer, PeerId},
        permission::Permission,
        soranet::privacy_metrics::{
            SoranetPrivacyEventHandshakeSuccessV1, SoranetPrivacyEventKindV1,
            SoranetPrivacyEventV1, SoranetPrivacyModeV1, SoranetPrivacyPrioShareV1,
        },
        transaction::{
            Executable, ExecutionStep, IvmBytecode, IvmProved,
            error::TransactionRejectionReason,
            signed::{
                SealedTransactionReveal, SignedTransaction, TransactionBuilder,
                TransactionEntrypoint, TransactionResultInner, TransactionSignature,
                compute_sealed_transaction_commitment,
            },
        },
        trigger::{DataTriggerSequence, DataTriggerStep, TimeTriggerEntrypoint, TriggerId},
    };
    use iroha_executor_data_model::permission::account::{
        AccountAliasPermissionScope, CanManageAccountAlias, CanResolveAccountAlias,
    };
    use iroha_primitives::{const_vec::ConstVec, json::Json, numeric::Quantity};
    use iroha_test_samples::ALICE_ID;
    use norito::codec::Encode;
    use tower::ServiceExt as _;

    use super::*;
    #[cfg(feature = "telemetry")]
    use crate::{RecordSoranetPrivacyEventDto, RecordSoranetPrivacyShareDto};
    use crate::{routing::handle_v1_sumeragi_commit_qcs, utils::extractors::NoritoJson};

    fn query_conversion_message(err: &Error) -> Option<&str> {
        match err {
            Error::Query(ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(message),
            )) => Some(message.as_str()),
            _ => None,
        }
    }

    pub fn mk_app_state_for_tests() -> SharedAppState {
        mk_app_state_for_tests_with_world_and_options(World::default(), None, None, None, None)
    }

    fn mk_app_state_for_tests_with_chain_id(chain_id: ChainId) -> SharedAppState {
        mk_app_state_for_tests_with_world_and_options_and_chain_id(
            World::default(),
            None,
            None,
            None,
            None,
            chain_id,
        )
    }

    pub fn mk_app_state_for_tests_with_iso_bridge(
        iso: Option<iroha_config::parameters::actual::IsoBridge>,
    ) -> SharedAppState {
        mk_app_state_for_tests_with_world_and_options(World::default(), iso, None, None, None)
    }

    pub fn mk_app_state_for_tests_with_options(
        iso: Option<iroha_config::parameters::actual::IsoBridge>,
        deploy_limit: Option<(u32, u32)>,
        norito_rpc: Option<iroha_config::parameters::actual::NoritoRpcTransport>,
        push: Option<iroha_config::parameters::actual::Push>,
    ) -> SharedAppState {
        mk_app_state_for_tests_with_world_and_options(
            World::default(),
            iso,
            deploy_limit,
            norito_rpc,
            push,
        )
    }

    pub fn mk_app_state_for_tests_with_world(world: World) -> SharedAppState {
        mk_app_state_for_tests_with_world_and_options(world, None, None, None, None)
    }

    #[cfg(feature = "push")]
    pub fn mk_app_state_for_tests_with_world_and_push(
        world: World,
        push: iroha_config::parameters::actual::Push,
    ) -> SharedAppState {
        mk_app_state_for_tests_with_world_and_options(world, None, None, None, Some(push))
    }

    #[cfg(feature = "app_api")]
    pub fn reconfigure_sorafs_runtime_for_tests(
        app: SharedAppState,
        sorafs_cache: Option<Arc<RwLock<sorafs::ProviderAdvertCache>>>,
        sorafs_node: sorafs_node::NodeHandle,
    ) -> SharedAppState {
        let mut inner =
            Arc::try_unwrap(app).unwrap_or_else(|_| panic!("unique app state for reconfigure"));
        inner.sorafs_cache = sorafs_cache;
        inner.sorafs_node = sorafs_node;
        Arc::new(inner)
    }

    pub(crate) fn app_auth_test_guard(
        config: crate::app_auth::CanonicalRequestAuthConfig,
    ) -> impl Drop {
        static TEST_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
        struct Guard(MutexGuard<'static, ()>);
        impl Drop for Guard {
            fn drop(&mut self) {
                crate::app_auth::configure(crate::app_auth::CanonicalRequestAuthConfig::default());
            }
        }

        let guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        crate::app_auth::configure(config);
        Guard(guard)
    }

    pub(crate) fn world_with_account(account_id: &AccountId) -> World {
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let domain = Domain::new(domain_id.clone()).build(account_id);
        let account = Account::new(account_id.clone()).build(account_id);
        World::with([domain], [account], [])
    }

    fn bind_uaid_to_dataspace_manifest_for_test(
        world: &mut World,
        uaid: UniversalAccountId,
        dataspace: DataSpaceId,
    ) {
        let manifest = iroha_data_model::nexus::AssetPermissionManifest {
            version: iroha_data_model::nexus::ManifestVersion::default(),
            uaid,
            dataspace,
            issued_ms: 0,
            activation_epoch: 1,
            expiry_epoch: None,
            entries: Vec::new(),
        };
        let mut record =
            iroha_core::nexus::space_directory::SpaceDirectoryManifestRecord::new(manifest);
        record.lifecycle.mark_activated(1);
        let mut set = iroha_core::nexus::space_directory::SpaceDirectoryManifestSet::default();
        set.upsert(record);
        world
            .space_directory_manifests_mut_for_testing()
            .insert(uaid, set);
    }

    pub(crate) fn world_with_account_bound_to_dataspace(
        account_id: &AccountId,
        uaid: UniversalAccountId,
        dataspace: DataSpaceId,
    ) -> World {
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let domain = Domain::new(domain_id.clone()).build(account_id);
        let account = Account::new(account_id.clone())
            .with_uaid(Some(uaid))
            .build(account_id);
        let mut world = World::with([domain], [account], []);

        bind_uaid_to_dataspace_manifest_for_test(&mut world, uaid, dataspace);
        world
    }

    pub(crate) fn world_with_target_and_caller_bound_to_dataspace(
        target: &AccountId,
        caller: &AccountId,
        uaid: UniversalAccountId,
        dataspace: DataSpaceId,
    ) -> World {
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let domain = Domain::new(domain_id.clone()).build(target);
        let target_account = Account::new(target.clone())
            .with_uaid(Some(uaid))
            .build(target);
        let caller_account = Account::new(caller.clone()).build(caller);
        let mut world = World::with([domain], [target_account, caller_account], []);

        bind_uaid_to_dataspace_manifest_for_test(&mut world, uaid, dataspace);
        world
    }

    fn grant_account_permission_for_test(
        app: &SharedAppState,
        authority: &AccountId,
        permission: Permission,
    ) {
        let next_height = app
            .state
            .latest_block_header_fast()
            .map_or(1, |header| header.height().get().saturating_add(1));
        let header = BlockHeader::new(
            NonZeroU64::new(next_height).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = app.state.block(header);
        let mut tx = block.transaction();
        tx.world_mut_for_testing()
            .add_account_permission(authority, permission);
        tx.apply();
        block.commit().expect("commit permission seed");
    }

    fn seed_asset_definition_for_test(
        app: &SharedAppState,
        asset_definition_id: &iroha_data_model::asset::AssetDefinitionId,
    ) {
        let missing_domain = asset_definition_id
            .try_domain()
            .is_some_and(|domain_id| app.state.view().world().domain(domain_id).is_err());
        if missing_domain {
            bind_domain_name_for_test(
                app,
                &asset_definition_id
                    .try_domain()
                    .expect("missing domain implies a projected asset definition id")
                    .to_string(),
            );
        }

        let next_height = app
            .state
            .latest_block_header_fast()
            .map_or(1, |header| header.height().get().saturating_add(1));
        let header = BlockHeader::new(
            NonZeroU64::new(next_height).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = app.state.block(header);
        let mut tx = block.transaction();

        if let Some(domain_id) = asset_definition_id.try_domain()
            && app.state.view().world().domain(domain_id).is_err()
        {
            Register::domain(Domain::new(domain_id.clone()))
                .execute(&ALICE_ID, &mut tx)
                .expect("register asset domain");
        }

        let mut asset_definition =
            iroha_data_model::asset::AssetDefinition::numeric(asset_definition_id.clone())
                .with_name(
                    asset_definition_id
                        .try_name()
                        .map_or_else(String::new, ToString::to_string),
                );
        if asset_definition_requires_restricted_balance_policy_for_test(app, asset_definition_id) {
            asset_definition =
                asset_definition.with_balance_scope_policy(AssetBalancePolicy::DataspaceRestricted);
        }

        Register::asset_definition(asset_definition)
            .execute(&ALICE_ID, &mut tx)
            .expect("register asset definition");

        tx.apply();
        block.commit().expect("commit asset definition seed");
    }

    fn asset_definition_requires_restricted_balance_policy_for_test(
        app: &SharedAppState,
        asset_definition_id: &iroha_data_model::asset::AssetDefinitionId,
    ) -> bool {
        let Some(domain_id) = asset_definition_id.try_domain() else {
            return false;
        };
        let dataspace_alias = domain_id.dataspace().as_ref();
        let state_view = app.state.view();
        let nexus = state_view.nexus();
        let Some(dataspace_id) = (if dataspace_alias.eq_ignore_ascii_case("universal") {
            Some(DataSpaceId::UNIVERSAL)
        } else {
            nexus
                .dataspace_catalog
                .by_alias(dataspace_alias)
                .map(|entry| entry.id)
        }) else {
            return false;
        };

        dataspace_id != DataSpaceId::UNIVERSAL
            && !nexus.lane_catalog.lanes().iter().any(|lane| {
                lane.dataspace_id == dataspace_id
                    && lane.visibility == iroha_data_model::nexus::LaneVisibility::Public
            })
    }

    fn configure_nexus_fee_admission_for_test(
        app: &mut SharedAppState,
        fee_asset_id: &iroha_data_model::asset::AssetDefinitionId,
        fee_sink_account_id: &AccountId,
    ) {
        let mut nexus = iroha_config::parameters::actual::Nexus::default();
        nexus.enabled = true;
        nexus.fees.base_fee = Quantity::from(1_u32);
        nexus.fees.per_byte_fee = Quantity::zero();
        nexus.fees.per_instruction_fee = Quantity::zero();
        nexus.fees.per_gas_unit_fee = Quantity::zero();
        nexus.fees.fee_asset_id = fee_asset_id.to_string();
        nexus.fees.fee_sink_account_id = fee_sink_account_id.to_string();

        let app_state = Arc::get_mut(app).expect("unique app state");
        let state = Arc::get_mut(&mut app_state.state).expect("unique state");
        state.set_nexus(nexus.clone()).expect("apply nexus config");
        let state_view = app_state.state.view();
        app_state.queue.reconfigure_nexus(&nexus, &state_view, None);
    }

    pub(crate) fn configure_multiple_dataspace_routes_for_test(app: &mut SharedAppState) {
        let secondary_dataspace = DataSpaceId::new(1);
        let secondary_lane = LaneId::new(1);
        let lane_catalog = iroha_data_model::nexus::LaneCatalog::new(
            NonZeroU32::new(2).expect("nonzero lane count"),
            vec![
                iroha_data_model::nexus::LaneConfig::default(),
                iroha_data_model::nexus::LaneConfig {
                    id: secondary_lane,
                    dataspace_id: secondary_dataspace,
                    alias: "secondary".to_owned(),
                    ..iroha_data_model::nexus::LaneConfig::default()
                },
            ],
        )
        .expect("lane catalog");
        let dataspace_catalog = iroha_data_model::nexus::DataSpaceCatalog::new(vec![
            iroha_data_model::nexus::DataSpaceMetadata::default(),
            iroha_data_model::nexus::DataSpaceMetadata {
                id: secondary_dataspace,
                alias: "secondary".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("dataspace catalog");
        let nexus = iroha_config::parameters::actual::Nexus {
            enabled: true,
            lane_catalog,
            dataspace_catalog,
            ..iroha_config::parameters::actual::Nexus::default()
        };

        let app_state = Arc::get_mut(app).expect("unique app state");
        let state = Arc::get_mut(&mut app_state.state).expect("unique state");
        state.set_nexus(nexus.clone()).expect("apply nexus config");
        let state_view = app_state.state.view();
        app_state.queue.reconfigure_nexus(&nexus, &state_view, None);
    }

    pub(crate) fn configure_private_ingress_routes_for_test(
        app: &mut SharedAppState,
    ) -> (LaneId, DataSpaceId) {
        let nexus_lane = LaneId::new(0);
        let local_validator_keypair = checked_torii_test_keypair_from_seed_byte(
            0xb6,
            Algorithm::Ed25519,
            "derive private-ingress local validator fixture key",
        );
        let local_peer_keypair = checked_torii_test_keypair_from_seed_byte(
            0xb7,
            Algorithm::BlsNormal,
            "derive private-ingress local peer fixture key",
        );
        let local_validator = AccountId::new(local_validator_keypair.public_key().clone());
        let local_peer_id = PeerId::from(local_peer_keypair.public_key().clone());
        let governance_dataspace = DataSpaceId::new(1);
        let governance_lane = LaneId::new(1);
        let restricted_dataspace = DataSpaceId::new(10);
        let restricted_lane = LaneId::new(2);

        let app_mut = Arc::get_mut(app).expect("unique app state");
        let (online_tx, online_rx) = tokio::sync::watch::channel(std::collections::HashSet::new());
        online_tx
            .send(std::collections::HashSet::from([Peer::new(
                "127.0.0.1:12001".parse().expect("valid local address"),
                local_peer_keypair.public_key().clone(),
            )]))
            .expect("online peers update should succeed");
        app_mut.online_peers = OnlinePeersProvider::new(online_rx);
        app_mut.local_peer_id = Some(local_peer_id.clone());

        let lane_catalog = iroha_data_model::nexus::LaneCatalog::new(
            NonZeroU32::new(3).expect("nonzero lane count"),
            vec![
                iroha_data_model::nexus::LaneConfig::default(),
                iroha_data_model::nexus::LaneConfig {
                    id: governance_lane,
                    dataspace_id: governance_dataspace,
                    alias: "governance".to_owned(),
                    visibility: iroha_data_model::nexus::LaneVisibility::Public,
                    ..iroha_data_model::nexus::LaneConfig::default()
                },
                iroha_data_model::nexus::LaneConfig {
                    id: restricted_lane,
                    dataspace_id: restricted_dataspace,
                    alias: "restricted".to_owned(),
                    visibility: iroha_data_model::nexus::LaneVisibility::Restricted,
                    ..iroha_data_model::nexus::LaneConfig::default()
                },
            ],
        )
        .expect("lane catalog");
        let dataspace_catalog = iroha_data_model::nexus::DataSpaceCatalog::new(vec![
            iroha_data_model::nexus::DataSpaceMetadata::default(),
            iroha_data_model::nexus::DataSpaceMetadata {
                id: governance_dataspace,
                alias: "governance".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
            iroha_data_model::nexus::DataSpaceMetadata {
                id: restricted_dataspace,
                alias: "restricted".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("dataspace catalog");
        let nexus = iroha_config::parameters::actual::Nexus {
            enabled: true,
            lane_catalog,
            dataspace_catalog,
            ..iroha_config::parameters::actual::Nexus::default()
        };

        let state = Arc::get_mut(&mut app_mut.state).expect("unique state");
        state.set_nexus(nexus.clone()).expect("apply nexus config");
        ensure_runtime_peer_binding_for_test(state, &local_validator, &local_peer_keypair, "local");
        {
            let mut topology = state.commit_topology.block();
            topology.clear();
            topology.push(local_peer_id.clone());
            topology.commit();
        }
        install_lane_manifest_registry_for_test(
            state,
            &[
                (
                    nexus_lane,
                    vec![(local_validator.clone(), local_peer_id.clone())],
                ),
                (
                    governance_lane,
                    vec![(local_validator.clone(), local_peer_id.clone())],
                ),
                (restricted_lane, vec![(local_validator, local_peer_id)]),
            ],
        );
        let state_view = app_mut.state.view();
        app_mut.queue.reconfigure_nexus(&nexus, &state_view, None);

        (restricted_lane, restricted_dataspace)
    }

    pub(crate) fn configure_private_ingress_with_offline_foreign_route_for_test(
        app: &mut SharedAppState,
    ) -> (RoutingDecision, RoutingDecision) {
        let nexus_lane = LaneId::new(0);
        let local_validator_keypair = checked_torii_test_keypair_from_seed_byte(
            0xb8,
            Algorithm::Ed25519,
            "derive offline-foreign local validator fixture key",
        );
        let local_peer_keypair = checked_torii_test_keypair_from_seed_byte(
            0xb9,
            Algorithm::BlsNormal,
            "derive offline-foreign local peer fixture key",
        );
        let foreign_validator_keypair = checked_torii_test_keypair_from_seed_byte(
            0xba,
            Algorithm::Ed25519,
            "derive offline-foreign validator fixture key",
        );
        let foreign_peer_keypair = checked_torii_test_keypair_from_seed_byte(
            0xbb,
            Algorithm::BlsNormal,
            "derive offline-foreign peer fixture key",
        );
        let local_validator = AccountId::new(local_validator_keypair.public_key().clone());
        let local_peer_id = PeerId::from(local_peer_keypair.public_key().clone());
        let foreign_validator = AccountId::new(foreign_validator_keypair.public_key().clone());
        let foreign_peer_id = PeerId::from(foreign_peer_keypair.public_key().clone());
        let local_dataspace = DataSpaceId::new(10);
        let local_lane = LaneId::new(1);
        let foreign_dataspace = DataSpaceId::new(12);
        let foreign_lane = LaneId::new(2);

        let app_mut = Arc::get_mut(app).expect("unique app state");
        let (online_tx, online_rx) = tokio::sync::watch::channel(std::collections::HashSet::new());
        online_tx
            .send(std::collections::HashSet::from([Peer::new(
                "127.0.0.1:12001".parse().expect("valid local address"),
                local_peer_keypair.public_key().clone(),
            )]))
            .expect("online peers update should succeed");
        app_mut.online_peers = OnlinePeersProvider::new(online_rx);
        app_mut.local_peer_id = Some(local_peer_id.clone());

        let lane_catalog = iroha_data_model::nexus::LaneCatalog::new(
            NonZeroU32::new(3).expect("nonzero lane count"),
            vec![
                iroha_data_model::nexus::LaneConfig::default(),
                iroha_data_model::nexus::LaneConfig {
                    id: local_lane,
                    dataspace_id: local_dataspace,
                    alias: "local-restricted".to_owned(),
                    visibility: iroha_data_model::nexus::LaneVisibility::Restricted,
                    ..iroha_data_model::nexus::LaneConfig::default()
                },
                iroha_data_model::nexus::LaneConfig {
                    id: foreign_lane,
                    dataspace_id: foreign_dataspace,
                    alias: "foreign-restricted".to_owned(),
                    visibility: iroha_data_model::nexus::LaneVisibility::Restricted,
                    ..iroha_data_model::nexus::LaneConfig::default()
                },
            ],
        )
        .expect("lane catalog");
        let dataspace_catalog = iroha_data_model::nexus::DataSpaceCatalog::new(vec![
            iroha_data_model::nexus::DataSpaceMetadata::default(),
            iroha_data_model::nexus::DataSpaceMetadata {
                id: local_dataspace,
                alias: "local-restricted".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
            iroha_data_model::nexus::DataSpaceMetadata {
                id: foreign_dataspace,
                alias: "foreign-restricted".to_owned(),
                description: None,
                fault_tolerance: 1,
            },
        ])
        .expect("dataspace catalog");
        let nexus = iroha_config::parameters::actual::Nexus {
            enabled: true,
            lane_catalog,
            dataspace_catalog,
            ..iroha_config::parameters::actual::Nexus::default()
        };

        let state = Arc::get_mut(&mut app_mut.state).expect("unique state");
        state.set_nexus(nexus.clone()).expect("apply nexus config");
        ensure_runtime_peer_binding_for_test(state, &local_validator, &local_peer_keypair, "local");
        ensure_runtime_peer_binding_for_test(
            state,
            &foreign_validator,
            &foreign_peer_keypair,
            "foreign",
        );
        {
            let mut topology = state.commit_topology.block();
            topology.clear();
            topology.push(local_peer_id.clone());
            topology.push(foreign_peer_id.clone());
            topology.commit();
        }
        install_lane_manifest_registry_for_test(
            state,
            &[
                (
                    nexus_lane,
                    vec![(local_validator.clone(), local_peer_id.clone())],
                ),
                (
                    local_lane,
                    vec![(local_validator.clone(), local_peer_id.clone())],
                ),
                (foreign_lane, vec![(foreign_validator, foreign_peer_id)]),
            ],
        );
        let state_view = app_mut.state.view();
        app_mut.queue.reconfigure_nexus(&nexus, &state_view, None);

        (
            RoutingDecision::new(local_lane, local_dataspace),
            RoutingDecision::new(foreign_lane, foreign_dataspace),
        )
    }

    fn install_lane_manifest_registry_for_test(
        state: &IrohaState,
        lanes: &[(LaneId, Vec<(AccountId, PeerId)>)],
    ) {
        let lanes_with_torii_urls = lanes
            .iter()
            .map(|(lane_id, validator_bindings)| {
                (
                    *lane_id,
                    validator_bindings
                        .iter()
                        .map(|(validator, peer_id)| {
                            (validator.clone(), peer_id.clone(), None::<&str>)
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>();
        install_lane_manifest_registry_with_torii_urls_for_test(state, &lanes_with_torii_urls);
    }

    fn install_lane_manifest_registry_with_torii_urls_for_test(
        state: &IrohaState,
        lanes: &[(LaneId, Vec<(AccountId, PeerId, Option<&str>)>)],
    ) {
        let nexus = state.nexus_snapshot();
        let manifest_root = std::env::temp_dir().join(format!(
            "iroha-torii-manifests-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock should be valid")
                .as_nanos()
        ));
        std::fs::create_dir_all(&manifest_root).expect("create manifest directory");
        for (lane_id, validator_bindings) in lanes {
            let alias = nexus
                .lane_catalog
                .lanes()
                .iter()
                .find(|lane| lane.id == *lane_id)
                .map(|lane| lane.alias.clone())
                .unwrap_or_else(|| format!("lane-{}", lane_id.as_u32()));
            let validators_json = validator_bindings
                .iter()
                .map(|(validator, peer_id, torii_url)| {
                    let torii_url_json = torii_url
                        .map(|url| format!(r#","torii_url":"{url}""#))
                        .unwrap_or_default();
                    format!(
                        r#"{{"validator":"{validator}","peer_id":"{peer_id}"{torii_url_json}}}"#
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let manifest = format!(
                r#"{{"lane":"{alias}","governance":"parliament","version":1,"validators":[{validators_json}],"quorum":1}}"#
            );
            std::fs::write(
                manifest_root.join(format!("{alias}.manifest.json")),
                manifest,
            )
            .expect("write manifest");
        }

        let mut governance_modules = std::collections::BTreeMap::new();
        governance_modules.insert(
            "parliament".to_owned(),
            iroha_config::parameters::actual::GovernanceModule::default(),
        );
        let governance_catalog = iroha_config::parameters::actual::GovernanceCatalog {
            default_module: None,
            modules: governance_modules,
        };
        let registry_cfg = iroha_config::parameters::actual::LaneRegistry {
            manifest_directory: Some(manifest_root),
            ..iroha_config::parameters::actual::LaneRegistry::default()
        };
        let registry = std::sync::Arc::new(
            iroha_core::governance::manifest::LaneManifestRegistry::from_config(
                &nexus.lane_catalog,
                &governance_catalog,
                &registry_cfg,
            ),
        );
        state.install_lane_manifests(&registry);
    }

    fn ensure_runtime_peer_binding_for_test(
        state: &mut IrohaState,
        validator: &AccountId,
        peer_keypair: &KeyPair,
        consensus_label: &str,
    ) {
        let mut sumeragi_params = state.view().world().parameters().sumeragi().clone();
        sumeragi_params.key_activation_lead_blocks = 0;
        state.set_sumeragi_parameters(&sumeragi_params);

        let next_height = state
            .latest_block_header_fast()
            .map_or(1, |header| header.height().get().saturating_add(1));
        let header = BlockHeader::new(
            NonZeroU64::new(next_height).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = state.block(header);
        let mut tx = block.transaction();

        if state.view().world().account(validator).is_err() {
            Register::account(Account::new(validator.clone()))
                .execute(&ALICE_ID, &mut tx)
                .expect("register validator authority account");
        }

        let manage_consensus_keys = Permission::new(
            "CanManageConsensusKeys"
                .parse()
                .expect("CanManageConsensusKeys permission token"),
            Json::new(()),
        );
        Grant::account_permission(manage_consensus_keys, validator.clone())
            .execute(validator, &mut tx)
            .expect("grant manage consensus keys");

        let peer_id = PeerId::from(peer_keypair.public_key().clone());
        let pop = iroha_crypto::bls_normal_pop_prove(peer_keypair.private_key())
            .expect("PoP prove for peer keypair");
        let consensus_pop = pop.clone();
        RegisterPeerWithPop::new(peer_id.clone(), pop)
            .execute(validator, &mut tx)
            .expect("peer registration");

        let consensus_id = ConsensusKeyId::new(ConsensusKeyRole::Validator, consensus_label);
        let consensus_record = ConsensusKeyRecord {
            id: consensus_id.clone(),
            public_key: peer_keypair.public_key().clone(),
            pop: Some(consensus_pop),
            activation_height: next_height,
            expiry_height: None,
            hsm: None,
            replaces: None,
            status: ConsensusKeyStatus::Active,
        };
        RegisterConsensusKey {
            id: consensus_id,
            record: consensus_record,
        }
        .execute(validator, &mut tx)
        .expect("consensus key registration");

        tx.apply();
        block.commit().expect("commit runtime peer binding");
    }

    #[derive(Default)]
    struct CapturingIterableQueryExecutor {
        query: Mutex<Option<iroha_data_model::query::QueryWithParams>>,
    }

    impl CapturingIterableQueryExecutor {
        fn into_query(self) -> iroha_data_model::query::QueryWithParams {
            self.query
                .into_inner()
                .expect("capture mutex should not be poisoned")
                .expect("query builder should have captured a query")
        }
    }

    impl iroha_data_model::query::builder::QueryExecutor for CapturingIterableQueryExecutor {
        type Cursor = ();
        type Error = iroha_data_model::query::builder::TypedBatchDowncastError;

        fn execute_singular_query(
            &self,
            _query: iroha_data_model::query::SingularQueryBox,
        ) -> Result<iroha_data_model::query::SingularQueryOutputBox, Self::Error> {
            unreachable!("capturing executor should only be used for iterable queries")
        }

        fn start_query(
            &self,
            query: iroha_data_model::query::QueryWithParams,
        ) -> Result<
            (
                iroha_data_model::query::QueryOutputBatchBoxTuple,
                Option<u64>,
                Option<Self::Cursor>,
            ),
            Self::Error,
        > {
            *self.query.lock().expect("capture mutex should lock") = Some(query);
            Err(
                iroha_data_model::query::builder::TypedBatchDowncastError::ColumnCountMismatch {
                    expected: 1,
                    actual: 0,
                },
            )
        }

        fn continue_query(
            _cursor: Self::Cursor,
        ) -> Result<
            (
                iroha_data_model::query::QueryOutputBatchBoxTuple,
                Option<u64>,
                Option<Self::Cursor>,
            ),
            Self::Error,
        > {
            unreachable!("capturing executor should not continue queries")
        }
    }

    fn build_find_triggers_query_for_test() -> iroha_data_model::query::QueryWithParams {
        use iroha_data_model::query::builder::QueryBuilderExt;

        let executor = CapturingIterableQueryExecutor::default();
        let _ = iroha_data_model::query::builder::QueryBuilder::new(
            &executor,
            iroha_data_model::query::trigger::prelude::FindTriggers,
        )
        .execute();
        executor.into_query()
    }

    fn build_find_active_trigger_ids_query_for_test() -> iroha_data_model::query::QueryWithParams {
        use iroha_data_model::query::builder::QueryBuilderExt;

        let executor = CapturingIterableQueryExecutor::default();
        let _ = iroha_data_model::query::builder::QueryBuilder::new(
            &executor,
            iroha_data_model::query::trigger::prelude::FindActiveTriggerIds,
        )
        .execute();
        executor.into_query()
    }

    fn build_find_account_ids_query_for_test() -> iroha_data_model::query::QueryWithParams {
        use iroha_data_model::query::builder::QueryBuilderExt;

        let executor = CapturingIterableQueryExecutor::default();
        let _ = iroha_data_model::query::builder::QueryBuilder::new(
            &executor,
            iroha_data_model::query::account::prelude::FindAccountIds,
        )
        .execute();
        executor.into_query()
    }

    fn build_find_peers_query_for_test() -> iroha_data_model::query::QueryWithParams {
        use iroha_data_model::query::builder::QueryBuilderExt;

        let executor = CapturingIterableQueryExecutor::default();
        let _ = iroha_data_model::query::builder::QueryBuilder::new(
            &executor,
            iroha_data_model::query::peer::prelude::FindPeers,
        )
        .execute();
        executor.into_query()
    }

    fn build_find_permissions_by_account_query_for_test(
        account_id: AccountId,
    ) -> iroha_data_model::query::QueryWithParams {
        use iroha_data_model::query::builder::QueryBuilderExt;

        let executor = CapturingIterableQueryExecutor::default();
        let _ = iroha_data_model::query::builder::QueryBuilder::new(
            &executor,
            iroha_data_model::prelude::FindPermissionsByAccountId::new(account_id),
        )
        .execute();
        executor.into_query()
    }

    fn build_find_roles_by_account_query_for_test(
        account_id: AccountId,
    ) -> iroha_data_model::query::QueryWithParams {
        use iroha_data_model::query::builder::QueryBuilderExt;

        let executor = CapturingIterableQueryExecutor::default();
        let _ = iroha_data_model::query::builder::QueryBuilder::new(
            &executor,
            iroha_data_model::prelude::FindRolesByAccountId::new(account_id),
        )
        .execute();
        executor.into_query()
    }

    fn build_find_domains_by_account_query_for_test(
        account_id: AccountId,
    ) -> iroha_data_model::query::QueryWithParams {
        use iroha_data_model::query::builder::QueryBuilderExt;

        let executor = CapturingIterableQueryExecutor::default();
        let _ = iroha_data_model::query::builder::QueryBuilder::new(
            &executor,
            iroha_data_model::prelude::FindDomainsByAccountId::new(account_id),
        )
        .execute();
        executor.into_query()
    }

    fn build_find_assets_by_account_query_for_test(
        account_id: AccountId,
    ) -> iroha_data_model::query::QueryWithParams {
        use iroha_data_model::query::builder::QueryBuilderExt;

        let executor = CapturingIterableQueryExecutor::default();
        let _ = iroha_data_model::query::builder::QueryBuilder::new(
            &executor,
            iroha_data_model::prelude::FindAssetsByAccountId::new(account_id),
        )
        .execute();
        executor.into_query()
    }

    fn build_find_accounts_with_asset_query_for_test(
        asset_definition_id: iroha_data_model::asset::AssetDefinitionId,
    ) -> iroha_data_model::query::QueryWithParams {
        use iroha_data_model::query::builder::QueryBuilderExt;

        let executor = CapturingIterableQueryExecutor::default();
        let _ = iroha_data_model::query::builder::QueryBuilder::new(
            &executor,
            iroha_data_model::query::account::prelude::FindAccountsWithAsset::new(
                asset_definition_id,
            ),
        )
        .execute();
        executor.into_query()
    }

    fn build_find_nfts_by_account_query_for_test(
        account_id: AccountId,
    ) -> iroha_data_model::query::QueryWithParams {
        use iroha_data_model::query::builder::QueryBuilderExt;

        let executor = CapturingIterableQueryExecutor::default();
        let _ = iroha_data_model::query::builder::QueryBuilder::new(
            &executor,
            iroha_data_model::prelude::FindNftsByAccountId::new(account_id),
        )
        .execute();
        executor.into_query()
    }

    fn build_find_transactions_query_for_test() -> iroha_data_model::query::QueryWithParams {
        use iroha_data_model::query::builder::QueryBuilderExt;

        let executor = CapturingIterableQueryExecutor::default();
        let _ = iroha_data_model::query::builder::QueryBuilder::new(
            &executor,
            iroha_data_model::query::transaction::prelude::FindTransactions::new(),
        )
        .execute();
        executor.into_query()
    }

    fn assert_permissions_query_targets_account(
        query: &iroha_data_model::query::QueryWithParams,
        account_id: &AccountId,
    ) {
        use iroha_data_model::{
            permission::Permission,
            prelude::FindPermissionsByAccountId,
            query::{QueryItemKind, iter_query_inner},
        };

        if let Some(query_box) = query.query_box() {
            let erased = iter_query_inner::<Permission>(query_box)
                .expect("permissions query should preserve erased permission item kind");
            let decoded =
                super::decode_query_payload::<FindPermissionsByAccountId>(erased.payload())
                    .expect("permissions query payload should decode");
            assert_eq!(decoded.account_id(), account_id);
            return;
        }

        let (item_kind, _, _, payload) = query
            .fast_dsl_parts()
            .expect("permissions query should expose fast-dsl payload");
        assert_eq!(item_kind, QueryItemKind::Permission);
        let decoded = super::decode_query_payload::<FindPermissionsByAccountId>(payload)
            .expect("permissions query payload should decode");
        assert_eq!(decoded.account_id(), account_id);
    }

    fn assert_roles_query_targets_account(
        query: &iroha_data_model::query::QueryWithParams,
        account_id: &AccountId,
    ) {
        use iroha_data_model::{
            prelude::FindRolesByAccountId,
            query::{QueryItemKind, iter_query_inner},
            role::RoleId,
        };

        if let Some(query_box) = query.query_box() {
            let erased = iter_query_inner::<RoleId>(query_box)
                .expect("roles query should preserve erased role-id item kind");
            let decoded = super::decode_query_payload::<FindRolesByAccountId>(erased.payload())
                .expect("roles query payload should decode");
            assert_eq!(decoded.account_id(), account_id);
            return;
        }

        let (item_kind, _, _, payload) = query
            .fast_dsl_parts()
            .expect("roles query should expose fast-dsl payload");
        assert_eq!(item_kind, QueryItemKind::RoleId);
        let decoded = super::decode_query_payload::<FindRolesByAccountId>(payload)
            .expect("roles query payload should decode");
        assert_eq!(decoded.account_id(), account_id);
    }

    fn assert_domains_query_targets_account(
        query: &iroha_data_model::query::QueryWithParams,
        account_id: &AccountId,
    ) {
        use iroha_data_model::{
            prelude::FindDomainsByAccountId,
            query::{QueryItemKind, iter_query_inner},
        };

        if let Some(query_box) = query.query_box() {
            let erased = iter_query_inner::<iroha_data_model::domain::Domain>(query_box)
                .expect("domains query should preserve erased domain item kind");
            let decoded = super::decode_query_payload::<FindDomainsByAccountId>(erased.payload())
                .expect("domains query payload should decode");
            assert_eq!(decoded.account_id(), account_id);
            return;
        }

        let (item_kind, _, _, payload) = query
            .fast_dsl_parts()
            .expect("domains query should expose fast-dsl payload");
        assert_eq!(item_kind, QueryItemKind::Domain);
        let decoded = super::decode_query_payload::<FindDomainsByAccountId>(payload)
            .expect("domains query payload should decode");
        assert_eq!(decoded.account_id(), account_id);
    }

    fn assert_assets_query_targets_account(
        query: &iroha_data_model::query::QueryWithParams,
        account_id: &AccountId,
    ) {
        use iroha_data_model::{
            prelude::FindAssetsByAccountId,
            query::{QueryItemKind, iter_query_inner},
        };

        if let Some(query_box) = query.query_box() {
            let erased = iter_query_inner::<iroha_data_model::asset::value::Asset>(query_box)
                .expect("assets query should preserve erased asset item kind");
            let decoded = super::decode_query_payload::<FindAssetsByAccountId>(erased.payload())
                .expect("assets query payload should decode");
            assert_eq!(decoded.account_id(), account_id);
            return;
        }

        let (item_kind, _, _, payload) = query
            .fast_dsl_parts()
            .expect("assets query should expose fast-dsl payload");
        assert_eq!(item_kind, QueryItemKind::Asset);
        let decoded = super::decode_query_payload::<FindAssetsByAccountId>(payload)
            .expect("assets query payload should decode");
        assert_eq!(decoded.account_id(), account_id);
    }

    fn assert_accounts_with_asset_query_targets_domain(
        query: &iroha_data_model::query::QueryWithParams,
        asset_definition_id: &iroha_data_model::asset::AssetDefinitionId,
    ) {
        use iroha_data_model::{
            account::Account,
            prelude::FindAccountsWithAsset,
            query::{QueryItemKind, iter_query_inner},
        };

        if let Some(query_box) = query.query_box() {
            let erased = iter_query_inner::<Account>(query_box)
                .expect("accounts-with-asset query should preserve erased account item kind");
            let decoded = super::decode_query_payload::<FindAccountsWithAsset>(erased.payload())
                .expect("accounts-with-asset query payload should decode");
            assert_eq!(decoded.asset_definition_id(), asset_definition_id);
            return;
        }

        let (item_kind, _, _, payload) = query
            .fast_dsl_parts()
            .expect("accounts-with-asset query should expose fast-dsl payload");
        assert_eq!(item_kind, QueryItemKind::Account);
        let decoded = super::decode_query_payload::<FindAccountsWithAsset>(payload)
            .expect("accounts-with-asset query payload should decode");
        assert_eq!(decoded.asset_definition_id(), asset_definition_id);
    }

    fn assert_nfts_query_targets_account(
        query: &iroha_data_model::query::QueryWithParams,
        account_id: &AccountId,
    ) {
        use iroha_data_model::{
            prelude::FindNftsByAccountId,
            query::{QueryItemKind, iter_query_inner},
        };

        if let Some(query_box) = query.query_box() {
            let erased = iter_query_inner::<iroha_data_model::nft::Nft>(query_box)
                .expect("nfts query should preserve erased nft item kind");
            let decoded = super::decode_query_payload::<FindNftsByAccountId>(erased.payload())
                .expect("nfts query payload should decode");
            assert_eq!(decoded.account_id(), account_id);
            return;
        }

        let (item_kind, _, _, payload) = query
            .fast_dsl_parts()
            .expect("nfts query should expose fast-dsl payload");
        assert_eq!(item_kind, QueryItemKind::Nft);
        let decoded = super::decode_query_payload::<FindNftsByAccountId>(payload)
            .expect("nfts query payload should decode");
        assert_eq!(decoded.account_id(), account_id);
    }

    fn signed_find_triggers_query_for_test(
        authority: AccountId,
        key_pair: &KeyPair,
    ) -> iroha_data_model::query::SignedQuery {
        iroha_data_model::query::QueryRequest::Start(build_find_triggers_query_for_test())
            .with_authority(authority)
            .sign(key_pair)
    }

    fn signed_find_active_trigger_ids_query_for_test(
        authority: AccountId,
        key_pair: &KeyPair,
    ) -> iroha_data_model::query::SignedQuery {
        iroha_data_model::query::QueryRequest::Start(build_find_active_trigger_ids_query_for_test())
            .with_authority(authority)
            .sign(key_pair)
    }

    fn request_for_test(
        authority: &AccountId,
        request: iroha_data_model::query::QueryRequest,
    ) -> iroha_data_model::query::QueryRequestWithAuthority {
        request.with_authority(authority.clone())
    }

    fn roundtrip_request_for_test(
        authority: &AccountId,
        request: iroha_data_model::query::QueryRequest,
    ) -> iroha_data_model::query::QueryRequestWithAuthority {
        let request = request_for_test(authority, request);
        norito::decode_from_bytes(
            &norito::to_bytes(&request).expect("query request should encode deterministically"),
        )
        .expect("query request should decode deterministically")
    }

    #[cfg(feature = "app_api")]
    pub(crate) fn bind_account_alias_for_test(
        app: &SharedAppState,
        account_id: &AccountId,
        alias: &str,
    ) {
        let dataspace_catalog = app.state.nexus_snapshot().dataspace_catalog.clone();
        let label =
            iroha_data_model::account::rekey::AccountAlias::from_literal(alias, &dataspace_catalog)
                .expect("valid account alias");
        let next_height = app
            .state
            .latest_block_header_fast()
            .map_or(1, |header| header.height().get().saturating_add(1));
        let header = BlockHeader::new(
            NonZeroU64::new(next_height).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = app.state.block(header);
        let mut tx = block.transaction();
        let world = tx.world_mut_for_testing();
        let selector = iroha_core::sns::selector_for_account_alias(&label, &dataspace_catalog)
            .expect("account alias selector");
        let account_address =
            AccountAddress::from_account_id(account_id).expect("address from account id");
        let record = iroha_data_model::sns::NameRecordV1::new(
            selector.clone(),
            account_id.clone(),
            vec![iroha_data_model::sns::NameControllerV1::account(
                &account_address,
            )],
            0,
            0,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            iroha_data_model::metadata::Metadata::default(),
        );
        world
            .account_aliases_mut_for_testing()
            .insert(label.clone(), account_id.clone());
        let mut labels = world
            .account_aliases_by_account_mut_for_testing()
            .get(account_id)
            .cloned()
            .unwrap_or_default();
        labels.insert(label.clone());
        world
            .account_aliases_by_account_mut_for_testing()
            .insert(account_id.clone(), labels);
        world.account_rekey_records_mut_for_testing().insert(
            label.clone(),
            iroha_data_model::account::rekey::AccountRekeyRecord::new(label, account_id.clone()),
        );
        world.smart_contract_state_mut_for_testing().insert(
            iroha_core::sns::record_storage_key(&selector),
            norito::codec::Encode::encode(&record),
        );
        tx.apply();
        block.commit().expect("commit account alias for test");
    }

    #[cfg(feature = "app_api")]
    pub(crate) fn bind_dynamic_account_alias_for_test(
        app: &SharedAppState,
        account_id: &AccountId,
        alias_literal: &str,
        dataspace_id: DataSpaceId,
    ) {
        let canonical_name = alias_literal
            .parse::<iroha_data_model::alias_setup::AccountAliasName>()
            .expect("canonical dynamic account alias");
        let label = AccountAlias::new(
            canonical_name.label.clone(),
            canonical_name.domain.clone().map(AccountAliasDomain::new),
            dataspace_id,
        );
        let dataspace_selector =
            iroha_core::sns::selector_for_dataspace_alias(canonical_name.dataspace.as_ref())
                .expect("dynamic dataspace selector");
        let alias_selector = iroha_data_model::sns::NameSelectorV1::new(
            iroha_data_model::sns::ACCOUNT_ALIAS_SUFFIX_ID,
            alias_literal,
        )
        .expect("dynamic account alias selector");
        let account_address =
            AccountAddress::from_account_id(account_id).expect("address from account id");
        let controllers = vec![iroha_data_model::sns::NameControllerV1::account(
            &account_address,
        )];
        let mut dataspace_metadata = iroha_data_model::metadata::Metadata::default();
        dataspace_metadata.insert(
            iroha_core::sns::SNS_DATASPACE_ID_METADATA_KEY
                .parse()
                .expect("dataspace id metadata key"),
            iroha_primitives::json::Json::new(dataspace_id.as_u64()),
        );
        let dataspace_record = iroha_data_model::sns::NameRecordV1::new(
            dataspace_selector.clone(),
            account_id.clone(),
            controllers.clone(),
            0,
            0,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            dataspace_metadata,
        );
        let alias_record = iroha_data_model::sns::NameRecordV1::new(
            alias_selector.clone(),
            account_id.clone(),
            controllers,
            0,
            0,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            iroha_data_model::metadata::Metadata::default(),
        );

        let next_height = app
            .state
            .latest_block_header_fast()
            .map_or(1, |header| header.height().get().saturating_add(1));
        let header = BlockHeader::new(
            NonZeroU64::new(next_height).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = app.state.block(header);
        let mut tx = block.transaction();
        let world = tx.world_mut_for_testing();
        world
            .account_aliases_mut_for_testing()
            .insert(label.clone(), account_id.clone());
        let mut labels = world
            .account_aliases_by_account_mut_for_testing()
            .get(account_id)
            .cloned()
            .unwrap_or_default();
        labels.insert(label.clone());
        world
            .account_aliases_by_account_mut_for_testing()
            .insert(account_id.clone(), labels);
        world.account_rekey_records_mut_for_testing().insert(
            label.clone(),
            iroha_data_model::account::rekey::AccountRekeyRecord::new(label, account_id.clone()),
        );
        world.smart_contract_state_mut_for_testing().insert(
            iroha_core::sns::record_storage_key(&dataspace_selector),
            norito::codec::Encode::encode(&dataspace_record),
        );
        world.smart_contract_state_mut_for_testing().insert(
            iroha_core::sns::record_storage_key(&alias_selector),
            norito::codec::Encode::encode(&alias_record),
        );
        tx.apply();
        block
            .commit()
            .expect("commit dynamic account alias for test");
    }

    #[cfg(feature = "app_api")]
    pub(crate) fn bind_contract_alias_for_test(
        app: &SharedAppState,
        contract_address: &iroha_data_model::smart_contract::ContractAddress,
        alias: &str,
    ) {
        let contract_alias = iroha_data_model::smart_contract::ContractAlias::from_str(alias)
            .expect("valid contract alias");
        let header = BlockHeader::new(
            NonZeroU64::new(1).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = app.state.block(header);
        let mut tx = block.transaction();
        tx.world_mut_for_testing()
            .bind_active_contract_subject_for_testing(
                contract_address.clone(),
                iroha_crypto::Hash::new(b"Torii contract alias identity fixture"),
            );
        tx.world_mut_for_testing()
            .bind_contract_alias(contract_address, contract_alias, None, None, 0)
            .expect("bind contract alias");
        tx.apply();
        block.commit().expect("commit contract alias for test");
    }

    #[cfg(feature = "app_api")]
    pub(crate) fn bind_domain_name_for_test(app: &SharedAppState, literal: &str) {
        bind_domain_name_for_test_with_status(
            app,
            literal,
            iroha_data_model::sns::NameStatus::Active,
        );
    }

    #[cfg(feature = "app_api")]
    pub(crate) fn bind_domain_name_for_test_with_status(
        app: &SharedAppState,
        literal: &str,
        status: iroha_data_model::sns::NameStatus,
    ) {
        let domain_id = DomainId::parse_fully_qualified(literal).expect("valid domain literal");
        let next_height = app
            .state
            .latest_block_header_fast()
            .map_or(1, |header| header.height().get().saturating_add(1));
        let header = BlockHeader::new(
            NonZeroU64::new(next_height).expect("non-zero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let mut block = app.state.block(header);
        let mut tx = block.transaction();
        let selector = iroha_core::sns::selector_for_domain(&domain_id).expect("domain selector");
        let owner = ALICE_ID.clone();
        let owner_address = AccountAddress::from_account_id(&owner).expect("owner address");
        let mut record = iroha_data_model::sns::NameRecordV1::new(
            selector.clone(),
            owner,
            vec![iroha_data_model::sns::NameControllerV1::account(
                &owner_address,
            )],
            0,
            0,
            u64::MAX,
            u64::MAX,
            u64::MAX,
            iroha_data_model::metadata::Metadata::default(),
        );
        record.status = status;
        tx.world_mut_for_testing()
            .smart_contract_state_mut_for_testing()
            .insert(
                iroha_core::sns::record_storage_key(&selector),
                norito::codec::Encode::encode(&record),
            );
        tx.apply();
        block.commit().expect("commit domain alias for test");
    }

    #[cfg(feature = "app_api")]
    fn soradns_public_alias_router(app: SharedAppState) -> axum::Router {
        axum::Router::new()
            .route(
                "/soradns/{fqdn}",
                any(super::handler_soradns_public_alias_root),
            )
            .route(
                "/soradns/{fqdn}/",
                any(super::handler_soradns_public_alias_root),
            )
            .route(
                "/soradns/{fqdn}/{*path}",
                any(super::handler_soradns_public_alias_path),
            )
            .fallback(any(super::handler_soracloud_public_local_read))
            .with_state(app)
    }

    pub(crate) fn signed_app_headers(
        account: &AccountId,
        key_pair: &KeyPair,
        method: &axum::http::Method,
        uri: &axum::http::Uri,
        body: &[u8],
    ) -> HeaderMap {
        static TEST_NONCE_SEQ: LazyLock<std::sync::atomic::AtomicU64> =
            LazyLock::new(|| std::sync::atomic::AtomicU64::new(0));
        let timestamp_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock")
            .as_millis() as u64;
        let nonce_seq = TEST_NONCE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let nonce = format!("lib-test-{timestamp_ms}-{nonce_seq}");
        let message =
            crate::canonical_request_signature_message(method, uri, body, timestamp_ms, &nonce);
        let signature = checked_torii_test_signature(
            key_pair,
            &message,
            "sign Torii signed-app-header fixture",
        );
        let mut headers = HeaderMap::new();
        headers.insert(
            crate::HEADER_ACCOUNT,
            account.to_string().parse().expect("account header"),
        );
        headers.insert(
            crate::HEADER_SIGNATURE,
            crate::signature_header_value(&signature)
                .parse()
                .expect("signature header"),
        );
        headers.insert(
            crate::HEADER_TIMESTAMP_MS,
            timestamp_ms.to_string().parse().expect("timestamp header"),
        );
        headers.insert(crate::HEADER_NONCE, nonce.parse().expect("nonce header"));
        headers
    }

    fn checked_torii_test_signature(
        key_pair: &KeyPair,
        message: &[u8],
        context: &'static str,
    ) -> Signature {
        Signature::try_new(key_pair.private_key(), message).expect(context)
    }

    fn checked_torii_test_keypair(
        seed: Vec<u8>,
        algorithm: Algorithm,
        context: &'static str,
    ) -> KeyPair {
        KeyPair::try_from_seed(seed, algorithm).expect(context)
    }

    fn checked_torii_test_keypair_from_seed_byte(
        seed: u8,
        algorithm: Algorithm,
        context: &'static str,
    ) -> KeyPair {
        checked_torii_test_keypair(vec![seed; 32], algorithm, context)
    }

    pub(crate) fn checked_torii_test_ed25519_keypair(seed: u8, context: &'static str) -> KeyPair {
        checked_torii_test_keypair_from_seed_byte(seed, Algorithm::Ed25519, context)
    }

    pub(crate) fn checked_torii_test_account_id(seed: u8, context: &'static str) -> AccountId {
        AccountId::new(
            checked_torii_test_ed25519_keypair(seed, context)
                .public_key()
                .clone(),
        )
    }

    #[test]
    fn checked_torii_test_keypair_rejects_all_zero_seed_material() {
        assert!(
            KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
            "checked Torii fixtures must reject invalid Ed25519 seed material"
        );
        assert!(
            KeyPair::try_from_seed(vec![0; 32], Algorithm::Secp256k1).is_err(),
            "checked Torii fixtures must reject invalid secp256k1 seed material"
        );
    }

    fn checked_torii_test_transaction(
        builder: TransactionBuilder,
        keypair: &KeyPair,
        context: &'static str,
    ) -> SignedTransaction {
        builder.try_sign(keypair.private_key()).expect(context)
    }

    fn checked_torii_test_block_signature(
        signatory_index: u64,
        keypair: &KeyPair,
        header: &BlockHeader,
        context: &'static str,
    ) -> BlockSignature {
        BlockSignature::new(
            signatory_index,
            SignatureOf::try_from_hash(keypair.private_key(), header.hash()).expect(context),
        )
    }

    #[test]
    fn checked_torii_test_block_signature_verifies_and_rejects_wrong_key() {
        let keypair = checked_torii_test_keypair_from_seed_byte(
            0xb1,
            Algorithm::BlsNormal,
            "derive Torii block signature fixture key",
        );
        let header = BlockHeader::new(
            NonZeroU64::new(9).expect("nonzero height"),
            None,
            None,
            None,
            0,
            0,
        );
        let signature =
            checked_torii_test_block_signature(0, &keypair, &header, "sign Torii block fixture");

        signature
            .signature()
            .verify_hash(keypair.public_key(), header.hash())
            .expect("checked Torii block fixture signature verifies");

        let wrong_key = checked_torii_test_keypair_from_seed_byte(
            0xb2,
            Algorithm::BlsNormal,
            "derive wrong Torii block signature fixture key",
        );
        signature
            .signature()
            .verify_hash(wrong_key.public_key(), header.hash())
            .expect_err("checked Torii block fixture signature rejects wrong key");
    }

    fn mk_app_state_for_tests_with_world_and_options(
        world: World,
        iso: Option<iroha_config::parameters::actual::IsoBridge>,
        deploy_limit: Option<(u32, u32)>,
        norito_rpc: Option<iroha_config::parameters::actual::NoritoRpcTransport>,
        push: Option<iroha_config::parameters::actual::Push>,
    ) -> SharedAppState {
        let chain_id: ChainId = "chain".parse().unwrap();
        mk_app_state_for_tests_with_world_and_options_and_chain_id(
            world,
            iso,
            deploy_limit,
            norito_rpc,
            push,
            chain_id,
        )
    }

    fn mk_app_state_for_tests_with_world_and_options_and_chain_id(
        world: World,
        iso: Option<iroha_config::parameters::actual::IsoBridge>,
        deploy_limit: Option<(u32, u32)>,
        norito_rpc: Option<iroha_config::parameters::actual::NoritoRpcTransport>,
        push: Option<iroha_config::parameters::actual::Push>,
        chain_id: ChainId,
    ) -> SharedAppState {
        // Minimal core state
        let _ = &push;
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let state_inner = iroha_core::state::State::new_with_chain_for_testing(
            world,
            kura.clone(),
            query_handle.clone(),
            chain_id.clone(),
        );
        {
            let mut topo_block = state_inner.commit_topology.block();
            topo_block.clear();
            let peer_keypair = checked_torii_test_keypair_from_seed_byte(
                0xb3,
                Algorithm::Ed25519,
                "derive Torii topology fixture peer key",
            );
            let peer_id = iroha_data_model::peer::PeerId::from(peer_keypair.public_key().clone());
            topo_block.push(peer_id);
            topo_block.commit();
        }
        let state = Arc::new(state_inner);

        // Minimal queue/events
        let events: EventsSender = tokio::sync::broadcast::channel(1).0;
        let queue_cfg = iroha_config::parameters::actual::Queue::default();
        let queue = Arc::new(Queue::from_config(queue_cfg, events.clone()));
        let pipeline_status_cache = Arc::new(PipelineStatusCache::new());
        // Minimal Kiso and peers provider (mocked to avoid spawning the full actor in tests)
        let cfg = crate::test_utils::mk_minimal_root_cfg();
        let kiso = KisoHandle::mock(&cfg);
        let (_tx, rx) =
            tokio::sync::watch::channel::<std::collections::HashSet<Peer>>(Default::default());
        let peers = OnlinePeersProvider::new(rx);

        #[cfg(feature = "connect")]
        let connect_cfg = iroha_config::parameters::actual::Connect {
            enabled: false,
            ws_max_sessions: iroha_config::parameters::defaults::connect::WS_MAX_SESSIONS,
            ws_per_ip_max_sessions:
                iroha_config::parameters::defaults::connect::WS_PER_IP_MAX_SESSIONS,
            ws_rate_per_ip_per_min:
                iroha_config::parameters::defaults::connect::WS_RATE_PER_IP_PER_MIN,
            session_ttl: iroha_config::parameters::defaults::connect::SESSION_TTL,
            frame_max_bytes: iroha_config::parameters::defaults::connect::FRAME_MAX_BYTES,
            session_buffer_max_bytes:
                iroha_config::parameters::defaults::connect::SESSION_BUFFER_MAX_BYTES,
            ping_interval: iroha_config::parameters::defaults::connect::PING_INTERVAL,
            ping_miss_tolerance: iroha_config::parameters::defaults::connect::PING_MISS_TOLERANCE,
            ping_min_interval: iroha_config::parameters::defaults::connect::PING_MIN_INTERVAL,
            dedupe_ttl: iroha_config::parameters::defaults::connect::DEDUPE_TTL,
            dedupe_cap: iroha_config::parameters::defaults::connect::DEDUPE_CAP,
            relay_enabled: false,
            relay_strategy: iroha_config::parameters::defaults::connect::RELAY_STRATEGY,
            p2p_ttl_hops: iroha_config::parameters::defaults::connect::P2P_TTL_HOPS,
        };
        #[cfg(feature = "push")]
        let push_cfg = push.unwrap_or_default();
        #[cfg(feature = "push")]
        let push_bridge = if push_cfg.enabled {
            Some(push::PushBridge::new(push_cfg.clone()))
        } else {
            None
        };
        #[cfg(feature = "app_api")]
        let sorafs_cache: Option<Arc<RwLock<sorafs::ProviderAdvertCache>>> = None;
        #[cfg(feature = "app_api")]
        // Test fixtures opt into an isolated storage directory explicitly. Never
        // let a unit-test helper inherit the production data path.
        let sorafs_node = sorafs_node::NodeHandle::new(
            sorafs_node::config::StorageConfig::builder()
                .enabled(false)
                .data_dir(kura.store_root().join("sorafs"))
                .build(),
        );
        #[cfg(feature = "app_api")]
        let sorafs_limits = Arc::new(sorafs::SorafsQuotaEnforcer::unlimited());
        #[cfg(feature = "app_api")]
        let sorafs_alias_cache = sorafs::policy_from_config(
            &iroha_config::parameters::actual::SorafsAliasCachePolicy::default(),
        );
        #[cfg(feature = "app_api")]
        let sorafs_alias_enforcement = sorafs::enforcement_from_config(
            &iroha_config::parameters::actual::SorafsAliasCachePolicy::default(),
        );
        #[cfg(feature = "app_api")]
        let stream_token_issuer: Option<Arc<sorafs::StreamTokenIssuer>> = None;
        #[cfg(feature = "app_api")]
        let sorafs_publish_discovery =
            iroha_config::parameters::actual::SorafsPublishDiscovery::default();
        #[cfg(feature = "app_api")]
        let sorafs_gateway_config = iroha_config::parameters::actual::SorafsGateway::default();
        #[cfg(feature = "app_api")]
        let sorafs_site_bindings = None;
        let telemetry = routing::MaybeTelemetry::for_tests().map_gate(TelemetryProfile::Full);
        let telemetry_profile = telemetry.profile();
        let iso_bridge = iso
            .as_ref()
            .and_then(|cfg| {
                crate::iso20022_bridge::Iso20022BridgeRuntime::from_config(cfg)
                    .expect("iso bridge config for tests should be valid")
            })
            .map(Arc::new);
        let deploy_rate_limiter = match deploy_limit {
            Some((rate, burst)) => limits::RateLimiter::new(Some(rate), Some(burst)),
            None => limits::RateLimiter::new(None, None),
        };
        let norito_rpc_cfg = norito_rpc.unwrap_or_default();
        let da_replay_cache = Arc::new(iroha_core::da::ReplayCache::new(
            iroha_core::da::ReplayCacheConfig::new(),
        ));
        let da_replay_store = Arc::new(da::ReplayCursorStore::in_memory());
        let da_ingest = iroha_config::parameters::actual::DaIngest::default();
        let da_ingest_compute_inflight = Arc::new(tokio::sync::Semaphore::new(
            da_ingest.max_concurrent_compute_jobs.get(),
        ));
        let da_receipt_signer = checked_torii_test_keypair_from_seed_byte(
            0xb4,
            Algorithm::Secp256k1,
            "derive Torii DA receipt fixture signer",
        );
        let alias_service = iso.as_ref().and_then(|cfg| {
            alias_service_from_iso_config(cfg, AliasAttester::new(da_receipt_signer.clone()))
        });
        let da_receipt_log = Arc::new(da::DaReceiptLog::in_memory(
            Arc::clone(&da_replay_store),
            da_receipt_signer.public_key().clone(),
        ));

        #[cfg(all(feature = "app_api", feature = "telemetry"))]
        let peer_telemetry = telemetry::peers::PeerTelemetryService::new(
            Vec::new(),
            telemetry::peers::GeoLookupConfig::disabled(),
            None,
        );

        let content_config_snapshot = state.content_snapshot();
        let soranet_privacy_ingest =
            iroha_config::parameters::actual::SoranetPrivacyIngest::default();
        let soranet_privacy_tokens: HashSet<String> =
            soranet_privacy_ingest.tokens.iter().cloned().collect();
        let soranet_privacy_allow_nets = limits::parse_cidrs(&soranet_privacy_ingest.allow_cidrs);
        let soranet_privacy_rate_limiter = limits::RateLimiter::new(
            soranet_privacy_ingest
                .rate_per_sec
                .map(std::num::NonZeroU32::get),
            soranet_privacy_ingest.burst.map(std::num::NonZeroU32::get),
        );
        let api_tokens_set: Arc<HashSet<String>> = Arc::new(Default::default());
        let operator_auth = Arc::new(
            operator_auth::OperatorAuth::new(
                iroha_config::parameters::actual::ToriiOperatorAuth::default(),
                api_tokens_set.clone(),
                defaults::torii::data_dir(),
                telemetry.clone(),
            )
            .expect("operator auth defaults should be valid"),
        );
        let operator_signatures = Arc::new(operator_signatures::OperatorSignatures::new(
            iroha_config::parameters::actual::ToriiOperatorSignatures::default(),
            da_receipt_signer.public_key().clone(),
            defaults::torii::MAX_CONTENT_LEN.get(),
            telemetry.clone(),
        ));

        let zk_ivm_prove_jobs = Arc::new(DashMap::new());
        let zk_ivm_prove_job_budget = Arc::new(ZkIvmProveJobBudget::new(
            usize::try_from(defaults::torii::ZK_IVM_PROVE_JOB_MAX_RETAINED_BYTES.get())
                .unwrap_or(usize::MAX),
        ));
        let soracloud_public_inflight_total = defaults::torii::SORACLOUD_PUBLIC_MAX_INFLIGHT.get();
        let soracloud_public_inflight =
            Arc::new(tokio::sync::Semaphore::new(soracloud_public_inflight_total));
        let soracloud_mutation_inflight = Arc::new(tokio::sync::Semaphore::new(
            defaults::torii::SORACLOUD_MUTATION_MAX_INFLIGHT.get(),
        ));
        let zk_ivm_prove_max_inflight = defaults::torii::ZK_IVM_PROVE_MAX_INFLIGHT.max(1);
        let zk_ivm_prove_slots_total =
            zk_ivm_prove_max_inflight.saturating_add(defaults::torii::ZK_IVM_PROVE_MAX_QUEUE);
        let zk_ivm_prove_slots = Arc::new(tokio::sync::Semaphore::new(zk_ivm_prove_slots_total));
        let zk_ivm_prove_inflight =
            Arc::new(tokio::sync::Semaphore::new(zk_ivm_prove_max_inflight));
        let zk_ivm_prove_inflight_total = zk_ivm_prove_max_inflight;
        let proof_body_inflight = Arc::new(tokio::sync::Semaphore::new(
            defaults::torii::PROOF_BODY_MAX_INFLIGHT.get(),
        ));
        let mcp = iroha_config::parameters::actual::ToriiMcp::default();
        let mcp_rate_per_sec = mcp.rate_per_minute.map(|rate| {
            let per_minute = rate.get();
            let per_sec = per_minute.div_ceil(60);
            per_sec.max(1)
        });
        let mcp_burst = mcp.burst.map(std::num::NonZeroU32::get);
        let mcp_rate_limiter = limits::RateLimiter::new(mcp_rate_per_sec, mcp_burst);
        let mcp_tools = Arc::new(if mcp.enabled {
            mcp::build_tool_specs(&mcp)
        } else {
            Vec::new()
        });

        Arc::new(AppState {
            events,
            kura,
            chain_id: Arc::new(chain_id),
            #[cfg(feature = "app_api")]
            transaction_max_content_len: usize::try_from(defaults::torii::MAX_CONTENT_LEN.get())
                .unwrap_or(usize::MAX),
            transaction_ingress_compute_inflight: Arc::new(tokio::sync::Semaphore::new(
                defaults::torii::TRANSACTION_INGRESS_MAX_CONCURRENT_COMPUTE_JOBS.get(),
            )),
            transaction_batch_max_transactions:
                defaults::torii::TRANSACTION_INGRESS_MAX_BATCH_TRANSACTIONS.get(),
            transaction_batch_max_bytes: usize::try_from(defaults::torii::MAX_CONTENT_LEN.get())
                .unwrap_or(usize::MAX),
            state: state.clone(),
            kiso,
            query_service: query_handle,
            query_inflight: Arc::new(tokio::sync::Semaphore::new(
                defaults::torii::QUERY_MAX_INFLIGHT.get(),
            )),
            query_heavy_inflight: Arc::new(tokio::sync::Semaphore::new(
                defaults::torii::QUERY_HEAVY_MAX_INFLIGHT.get(),
            )),
            query_queue_timeout: Duration::from_millis(defaults::torii::QUERY_QUEUE_TIMEOUT_MS),
            rate_limiter: limits::RateLimiter::new(None, None),
            pipeline_status_rate_limiter: limits::RateLimiter::new(None, None),
            tx_rate_limiter: limits::RateLimiter::new(None, None),
            deploy_rate_limiter,
            proof_rate_limiter: limits::RateLimiter::new(None, None),
            proof_egress_limiter: limits::RateLimiter::new_u64(None, None),
            proof_body_inflight,
            soracloud_public_rate_limiter: limits::RateLimiter::new(None, None),
            soracloud_mutation_rate_limiter: limits::RateLimiter::new(None, None),
            soracloud_mutation_inflight,
            soracloud_public_max_response_bytes: usize::try_from(
                defaults::torii::SORACLOUD_PUBLIC_MAX_RESPONSE_BYTES.get(),
            )
            .unwrap_or(usize::MAX),
            soracloud_mutation_max_body_bytes: usize::try_from(
                defaults::torii::SORACLOUD_MUTATION_MAX_BODY_BYTES.get(),
            )
            .unwrap_or(usize::MAX),
            soracloud_upload_max_body_bytes: usize::try_from(
                defaults::torii::SORACLOUD_UPLOAD_MAX_BODY_BYTES.get(),
            )
            .unwrap_or(usize::MAX),
            content_request_limiter: limits::RateLimiter::new(None, None),
            content_egress_limiter: limits::RateLimiter::new_u64(None, None),
            proof_limits: routing::ProofApiLimits::default(),
            content_config: content_config_snapshot,
            soracloud_hf_config: Default::default(),
            ws_message_timeout: Duration::from_millis(defaults::torii::WS_MESSAGE_TIMEOUT_MS),
            require_api_token: false,
            api_tokens_set: api_tokens_set.clone(),
            webhooks_enabled: defaults::torii::WEBHOOKS_ENABLED,
            zk_attachments_enabled: defaults::torii::ZK_ATTACHMENTS_ENABLED,
            operator_auth,
            operator_signatures,
            soranet_privacy_ingest,
            soranet_privacy_tokens: Arc::new(soranet_privacy_tokens),
            soranet_privacy_allow_nets: Arc::new(soranet_privacy_allow_nets),
            soranet_privacy_rate_limiter,
            allow_nets: Arc::new(vec![]),
            trusted_proxy_nets: Arc::new(vec![]),
            norito_rpc_mtls_trusted_proxy_nets: Arc::new(limits::parse_cidrs(
                &norito_rpc_cfg.mtls_trusted_proxy_cidrs,
            )),
            preauth_gate: Arc::new(limits::PreAuthGate::disabled()),
            queue,
            pipeline_status_cache,
            mcp,
            mcp_rate_limiter,
            mcp_tools,
            mcp_dispatch_router: std::sync::RwLock::new(None),
            fee_policy: FeePolicy::Disabled,
            norito_rpc: norito_rpc_cfg,
            high_load_tx_threshold: usize::MAX,
            high_load_stream_tx_threshold: usize::MAX,
            high_load_subscription_tx_threshold: usize::MAX,
            online_peers: peers,
            iso_bridge,
            alias_service,
            #[cfg(feature = "app_api")]
            identifier_resolver: None,
            #[cfg(feature = "app_api")]
            tx_history_access_policy: Arc::new(TxHistoryAccessPolicy::default()),
            telemetry,
            telemetry_profile,
            zk_prover_keys_dir: defaults::torii::zk_prover_keys_dir(),
            zk_ivm_prove_jobs,
            zk_ivm_prove_job_budget,
            soracloud_public_inflight,
            soracloud_public_inflight_total,
            sns_name_cache: Arc::new(sns::SnsNameRecordCache::new()),
            zk_ivm_prove_inflight,
            zk_ivm_prove_slots,
            zk_ivm_prove_slots_total,
            zk_ivm_prove_inflight_total,
            zk_ivm_prove_job_ttl_ms: defaults::torii::ZK_IVM_PROVE_JOB_TTL_SECS * 1_000,
            zk_ivm_prove_job_max_entries: defaults::torii::ZK_IVM_PROVE_JOB_MAX_ENTRIES,
            ivm_tooling_timeout: Duration::from_millis(defaults::torii::ZK_IVM_TOOLING_TIMEOUT_MS),
            #[cfg(all(feature = "app_api", feature = "telemetry"))]
            peer_telemetry,
            da_replay_cache,
            da_replay_store,
            da_receipt_log,
            da_receipt_signer,
            torii_proxy_bridge_signer: checked_torii_test_keypair_from_seed_byte(
                0xb5,
                Algorithm::Ed25519,
                "derive Torii proxy bridge fixture signer",
            ),
            #[cfg(feature = "app_api")]
            public_dataspace_upstreams: Arc::new(BTreeMap::new()),
            #[cfg(feature = "app_api")]
            recipient_lookup: Arc::new(Default::default()),
            #[cfg(feature = "app_api")]
            recipient_lookup_rate_limiter: limits::RateLimiter::new_per_minute(
                Some(defaults::torii::recipient_lookup::REQUESTS_PER_MINUTE),
                Some(defaults::torii::recipient_lookup::REQUESTS_PER_MINUTE),
            ),
            da_ingest,
            da_ingest_compute_inflight,
            da_spooler: None,
            #[cfg(feature = "app_api")]
            sorafs_cache,
            #[cfg(feature = "app_api")]
            sorafs_routing_authority_cache: Arc::new(
                sorafs::delegated_routing::RoutingAuthorityCache::default(),
            ),
            #[cfg(feature = "app_api")]
            sorafs_node,
            #[cfg(feature = "app_api")]
            sorafs_proof_outcome_signer: None,
            #[cfg(feature = "app_api")]
            sorafs_repair_transaction_signer: None,
            #[cfg(feature = "app_api")]
            sorafs_reserve_transaction_signer: None,
            #[cfg(feature = "app_api")]
            sorafs_orderbook_transaction_signer: None,
            #[cfg(feature = "app_api")]
            sorafs_reputation_committed_reader: None,
            #[cfg(feature = "app_api")]
            sorafs_hedging_billing_runtime: None,
            #[cfg(feature = "app_api")]
            sorafs_potr_runtime_signers: None,
            #[cfg(feature = "app_api")]
            sorafs_moderation_orchestrator: None,
            #[cfg(feature = "app_api")]
            sorafs_evidence_viewer: None,
            #[cfg(feature = "app_api")]
            sorafs_moderation_orchestrator_worker: None,
            #[cfg(feature = "app_api")]
            sorafs_limits,
            #[cfg(feature = "app_api")]
            por_coordinator: Arc::new(sorafs::PorCoordinator::new()),
            #[cfg(feature = "app_api")]
            por_runtime: None,
            #[cfg(feature = "app_api")]
            por_auditor_signature_threshold: usize::from(
                defaults::sorafs::por::AUDITOR_SIGNATURE_THRESHOLD,
            ),
            #[cfg(feature = "app_api")]
            sorafs_alias_cache_policy: sorafs_alias_cache,
            #[cfg(feature = "app_api")]
            sorafs_alias_enforcement,
            #[cfg(feature = "app_api")]
            sorafs_admission: None,
            #[cfg(feature = "app_api")]
            sorafs_pop_credentials: None,
            #[cfg(feature = "app_api")]
            sorafs_publish_discovery,
            #[cfg(feature = "app_api")]
            sorafs_gateway_config,
            #[cfg(feature = "app_api")]
            sorafs_site_bindings,
            #[cfg(feature = "app_api")]
            sorafs_gateway_policy: None,
            #[cfg(feature = "app_api")]
            sorafs_gateway_tls_state: None,
            #[cfg(feature = "app_api")]
            sorafs_gateway_compliance_controller: Some(
                sorafs::gateway::allow_all_gateway_compliance_controller_for_tests(),
            ),
            #[cfg(feature = "app_api")]
            sorafs_gateway_compliance_feed_transport: None,
            #[cfg(all(test, feature = "app_api"))]
            sorafs_gateway_test_provider_id: Some([0x45; 32]),
            #[cfg(feature = "app_api")]
            sorafs_blinded_resolver: None,
            #[cfg(feature = "app_api")]
            stream_token_issuer,
            #[cfg(feature = "app_api")]
            stream_token_concurrency: sorafs::StreamTokenConcurrencyTracker::default(),
            #[cfg(feature = "app_api")]
            stream_token_quota: sorafs::StreamTokenQuotaTracker::default(),
            #[cfg(feature = "app_api")]
            sorafs_chunk_range_overrides: DashMap::new(),
            #[cfg(feature = "app_api")]
            account_faucet: None,
            #[cfg(feature = "app_api")]
            sorafs_appeal_finance_policy: Arc::new(
                sorafs::api::AppealFinanceRuntimePolicy::from_config(
                    &iroha_config::parameters::actual::SorafsAppealFinanceSettlement::default(),
                )
                .expect("baseline SoraFS appeal-finance policy"),
            ),
            #[cfg(feature = "app_api")]
            sorafs_appeal_settlement_submitter: None,
            #[cfg(feature = "app_api")]
            offline_commands: None,
            #[cfg(feature = "app_api")]
            account_onboarding: None,
            vpn_helper_ticket_secret: None,
            vpn_quotes: Arc::new(DashMap::new()),
            vpn_used_payments: Arc::new(DashMap::new()),
            vpn_sessions: Arc::new(DashMap::new()),
            vpn_receipts: Arc::new(DashMap::new()),
            vpn_state_lock: Arc::new(tokio::sync::Mutex::new(())),
            soracloud_runtime: None,
            #[cfg(feature = "app_api")]
            soracloud_proxy_pending: Arc::new(tokio::sync::Mutex::new(BTreeMap::new())),
            #[cfg(feature = "app_api")]
            soracloud_proxy_sequence: std::sync::atomic::AtomicU64::new(1),
            #[cfg(any(feature = "p2p_ws", feature = "connect"))]
            torii_proxy_pending: Arc::new(tokio::sync::Mutex::new(BTreeMap::new())),
            #[cfg(any(feature = "p2p_ws", feature = "connect"))]
            torii_proxy_completed: Arc::new(tokio::sync::Mutex::new(
                CompletedToriiProxyRequests::default(),
            )),
            #[cfg(any(feature = "p2p_ws", feature = "connect"))]
            torii_proxy_session_id: new_torii_proxy_session_id(),
            #[cfg(any(feature = "p2p_ws", feature = "connect"))]
            torii_proxy_sequence: std::sync::atomic::AtomicU64::new(1),
            sumeragi: None,
            #[cfg(any(feature = "app_api", feature = "p2p_ws", feature = "connect"))]
            p2p: None,
            #[cfg(any(feature = "app_api", feature = "p2p_ws", feature = "connect"))]
            local_peer_id: None,
            #[cfg(feature = "connect")]
            connect_bus: crate::connect::Bus::from_config(&connect_cfg),
            #[cfg(feature = "connect")]
            connect_enabled: connect_cfg.enabled,
            #[cfg(feature = "push")]
            push: push_bridge,
            #[cfg(feature = "push")]
            push_rate_limiter: limits::RateLimiter::new(
                push_cfg
                    .rate_per_minute
                    .map(|v| v.get().saturating_add(59) / 60),
                push_cfg.burst.map(std::num::NonZeroU32::get),
            ),
        })
    }

    #[cfg(feature = "telemetry")]
    pub async fn mk_norito_rpc_test_harness(
        cfg: NoritoRpcTransport,
    ) -> (SharedAppState, Arc<iroha_telemetry::metrics::Metrics>) {
        let app = mk_app_state_for_tests_with_options(None, None, Some(cfg), None);
        let metrics = iroha_telemetry::metrics::global_or_default();
        (app, metrics)
    }

    #[tokio::test]
    async fn runtime_handlers_ok_without_token_and_rate_limit() {
        let app = mk_app_state_for_tests();
        let headers = HeaderMap::new();

        // Active ABI version
        let resp = super::handler_runtime_abi_active(
            State(app.clone()),
            headers.clone(),
            crate::loopback_connect_info(),
            None,
        )
        .await
        .expect("ok");
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body");
        let active: crate::runtime::RuntimeAbiActiveResponse =
            norito::json::from_slice(&bytes).expect("decode json");
        assert_eq!(active.abi_version, 1);

        // ABI hash
        let resp = super::handler_runtime_abi_hash(
            State(app),
            headers,
            crate::loopback_connect_info(),
            None,
        )
        .await
        .expect("ok");
        assert_eq!(resp.status(), axum::http::StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body");
        let hash: crate::runtime::RuntimeAbiHashResponse =
            norito::json::from_slice(&bytes).expect("decode json");
        assert_eq!(hash.policy, "V1");
        assert_eq!(hash.abi_hash_hex.len(), 64);
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn explorer_transaction_detail_not_found_returns_json_response() {
        let app = mk_app_state_for_tests();
        let headers = HeaderMap::new();
        let missing_hash = "00".repeat(32);

        let response = super::handler_explorer_transaction_detail(
            State(app),
            headers,
            crate::loopback_connect_info(),
            axum::extract::Path(missing_hash),
        )
        .await
        .expect("explorer detail handler should map errors to HTTP responses");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/json")
        );

        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body");
        let _payload: norito::json::Value =
            norito::json::from_slice(&bytes).expect("json error payload");
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn explorer_instruction_detail_not_found_returns_json_response() {
        let app = mk_app_state_for_tests();
        let headers = HeaderMap::new();
        let missing_hash = "00".repeat(32);

        let response = super::handler_explorer_instruction_detail(
            State(app),
            headers,
            crate::loopback_connect_info(),
            axum::extract::Path((missing_hash, 0)),
        )
        .await
        .expect("explorer instruction detail handler should map errors to HTTP responses");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            response
                .headers()
                .get(axum::http::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("application/json")
        );

        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body");
        let _payload: norito::json::Value =
            norito::json::from_slice(&bytes).expect("json error payload");
    }

    #[cfg(feature = "telemetry")]
    #[tokio::test]
    async fn debug_witness_returns_json_body() {
        let app = mk_app_state_for_tests();
        let headers = HeaderMap::new();
        let accept = Some(crate::utils::extractors::ExtractAccept(
            HeaderValue::from_static("application/json"),
        ));

        let resp = super::handler_debug_witness(
            State(app),
            headers,
            crate::loopback_connect_info(),
            accept,
        )
        .await
        .expect("debug witness response");

        assert_eq!(resp.status(), StatusCode::OK);
        let content_type = resp
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .expect("content type header");
        assert_eq!(content_type, "application/json");

        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("response body");
        let _parsed: norito::json::Value = norito::json::from_slice(&bytes).expect("valid json");
    }

    #[tokio::test]
    async fn torii_tx_rate_uses_config_and_queue_default() {
        let mut cfg = crate::test_utils::mk_minimal_root_cfg();
        cfg.torii.tx_rate_per_authority_per_sec =
            Some(NonZeroU32::new(123).expect("nonzero tx rate"));
        cfg.torii.tx_burst_per_authority = Some(NonZeroU32::new(456).expect("nonzero tx burst"));
        cfg.torii.api_high_load_tx_threshold = None;

        let (kiso, _child) = KisoHandle::start(cfg.clone());
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = Arc::new(IrohaState::new_for_testing(
            World::default(),
            kura.clone(),
            query,
        ));
        let queue_cfg = iroha_config::parameters::actual::Queue {
            capacity: NonZeroUsize::new(100).expect("queue capacity non-zero"),
            capacity_per_user: NonZeroUsize::new(100).expect("queue per-user capacity non-zero"),
            transaction_time_to_live: Duration::from_secs(60),
            ..Default::default()
        };
        let queue_events: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
        let queue = Arc::new(Queue::from_config(queue_cfg, queue_events));
        let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
        let _ = peers_tx;
        let torii = Torii::new_with_handle(
            ChainId::from("tx-rate-test"),
            kiso,
            cfg.torii.clone(),
            queue,
            tokio::sync::broadcast::channel(1).0,
            LiveQueryStore::start_test(),
            kura,
            state,
            cfg.common.key_pair.clone(),
            OnlinePeersProvider::new(peers_rx),
            None,
            routing::MaybeTelemetry::disabled(),
        );

        assert_eq!(torii.tx_rate_per_authority_per_sec.unwrap().get(), 123);
        assert_eq!(torii.tx_burst_per_authority.unwrap().get(), 456);
        assert_eq!(torii.high_load_tx_threshold, 50);
    }

    #[cfg(feature = "app_api")]
    #[tokio::test]
    async fn torii_ram_lfe_uses_config_runtime() {
        let mut cfg = crate::test_utils::mk_minimal_root_cfg();
        let signer = checked_torii_test_ed25519_keypair(
            0x9f,
            "derive Torii RAM-LFE config signer fixture key",
        );
        cfg.torii.ram_lfe = Some(iroha_config::parameters::actual::ToriiRamLfe {
            programs: vec![iroha_config::parameters::actual::ToriiRamLfeProgram {
                program_id: "phone_retail".parse().expect("program id"),
                secret: vec![0x01, 0x02, 0x03, 0x04],
                hidden_program: iroha_crypto::default_bfv_programmed_hidden_program(),
                signer_private_key: iroha_crypto::ExposedPrivateKey(signer.private_key().clone()),
                receipt_ttl: Some(Duration::from_secs(30)),
            }],
        });

        let (kiso, _child) = KisoHandle::start(cfg.clone());
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = Arc::new(IrohaState::new_for_testing(
            World::default(),
            kura.clone(),
            query,
        ));
        let queue_cfg = iroha_config::parameters::actual::Queue {
            capacity: NonZeroUsize::new(100).expect("queue capacity non-zero"),
            capacity_per_user: NonZeroUsize::new(100).expect("queue per-user capacity non-zero"),
            transaction_time_to_live: Duration::from_secs(60),
            ..Default::default()
        };
        let queue_events: iroha_core::EventsSender = tokio::sync::broadcast::channel(1).0;
        let queue = Arc::new(Queue::from_config(queue_cfg, queue_events));
        let (peers_tx, peers_rx) = tokio::sync::watch::channel(<_>::default());
        let _ = peers_tx;

        let torii = Torii::new_with_handle(
            ChainId::from("identifier-resolver-config-test"),
            kiso,
            cfg.torii.clone(),
            queue,
            tokio::sync::broadcast::channel(1).0,
            LiveQueryStore::start_test(),
            kura,
            state,
            cfg.common.key_pair.clone(),
            OnlinePeersProvider::new(peers_rx),
            None,
            routing::MaybeTelemetry::disabled(),
        );

        assert!(
            torii.identifier_resolver.is_some(),
            "Torii should build an in-process identifier resolver from config"
        );
    }

    fn versioned_signed_for_test(
        tx: &SignedTransaction,
    ) -> JsonOrNoritoVersioned<SignedTransaction> {
        JsonOrNoritoVersioned(tx.clone())
    }

    fn versioned_entrypoint_for_test(
        entrypoint: TransactionEntrypoint,
    ) -> JsonOrNoritoVersioned<TransactionEntrypoint> {
        JsonOrNoritoVersioned(entrypoint)
    }

    fn versioned_query_for_test(query: SignedQuery) -> JsonOrNoritoVersioned<SignedQuery> {
        JsonOrNoritoVersioned(query)
    }

    struct CountingRouteRouter {
        route_calls: Arc<AtomicUsize>,
    }

    impl LaneRouter for CountingRouteRouter {
        fn route(&self, _tx: &dyn TransactionRoutingView) -> RoutingDecision {
            RoutingDecision::new(LaneId::SINGLE, DataSpaceId::UNIVERSAL)
        }

        fn try_route_without_state(
            &self,
            _tx: &dyn TransactionRoutingView,
        ) -> Result<Option<RoutingDecision>, RoutingResolveError> {
            Ok(None)
        }

        fn try_route_with_state(
            &self,
            tx: &dyn TransactionRoutingView,
            _state: &IrohaState,
        ) -> Result<RoutingDecision, RoutingResolveError> {
            self.route_calls.fetch_add(1, Ordering::Relaxed);
            Ok(self.route(tx))
        }

        fn try_route_plan_with_state(
            &self,
            tx: &dyn TransactionRoutingView,
            state: &IrohaState,
        ) -> Result<RoutingPlan, RoutingResolveError> {
            self.try_route_with_state(tx, state)
                .map(RoutingPlan::single)
        }
    }

    fn install_counting_route_queue(app: &mut SharedAppState) -> Arc<AtomicUsize> {
        let route_calls = Arc::new(AtomicUsize::new(0));
        let router: Arc<dyn LaneRouter> = Arc::new(CountingRouteRouter {
            route_calls: Arc::clone(&route_calls),
        });
        let app_mut = Arc::get_mut(app).expect("unique app state");
        app_mut.queue = Arc::new(Queue::from_config_with_router(
            iroha_config::parameters::actual::Queue::default(),
            app_mut.events.clone(),
            router,
        ));
        app_mut.high_load_tx_threshold = usize::MAX;
        route_calls
    }

    fn install_single_slot_transaction_queue(app: &mut SharedAppState) {
        let app_mut = Arc::get_mut(app).expect("unique app state");
        let mut queue_cfg = iroha_config::parameters::actual::Queue::default();
        queue_cfg.capacity = NonZeroUsize::new(1).expect("nonzero queue capacity");
        app_mut.queue = Arc::new(Queue::from_config(queue_cfg, app_mut.events.clone()));
        app_mut.high_load_tx_threshold = usize::MAX;
    }

    fn transaction_with_invalid_signature_for_test(mut tx: SignedTransaction) -> SignedTransaction {
        let mut signature = tx.signature().payload().payload().to_vec();
        let last = signature
            .last_mut()
            .expect("test signature payload is non-empty");
        *last ^= 0xff;
        tx.set_signature(TransactionSignature(SignatureOf::from_signature(
            Signature::from_bytes(&signature),
        )));
        tx
    }

    #[tokio::test]
    async fn handler_post_transaction_uses_tx_rate_limiter() {
        let mut app = mk_app_state_for_tests();
        {
            let app_mut = Arc::get_mut(&mut app).expect("unique app state");
            app_mut.high_load_tx_threshold = usize::MAX;
            app_mut.tx_rate_limiter = limits::RateLimiter::new(Some(1), Some(1));
            app_mut.fee_policy = FeePolicy::Disabled;
        }

        let keypair = checked_torii_test_keypair_from_seed_byte(
            0xc1,
            Algorithm::Ed25519,
            "derive post-transaction rate-limit fixture key",
        );
        let authority = AccountId::new(keypair.public_key().clone());
        let chain = (*app.chain_id).clone();
        let tx1 = TransactionBuilder::new(
            chain.clone(),
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "rate-limit-1".to_string())])
        .sign(keypair.private_key());
        let tx2 = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "rate-limit-2".to_string())])
        .sign(keypair.private_key());
        let headers = HeaderMap::new();
        let submitted_hash = tx1.hash().to_string();

        let ok = super::handler_post_transaction(
            State(app.clone()),
            headers.clone(),
            None,
            versioned_signed_for_test(&tx1),
        )
        .await
        .expect("accepted");
        let ok_response = ok.into_response();
        assert_eq!(ok_response.status(), StatusCode::ACCEPTED);
        let hash_header = ok_response
            .headers()
            .get("x-iroha-transaction-hash")
            .and_then(|value| value.to_str().ok())
            .expect("transaction hash header must be present");
        assert_eq!(hash_header, submitted_hash);
        let lane_header = ok_response
            .headers()
            .get("x-iroha-route-lane-id")
            .and_then(|value| value.to_str().ok())
            .expect("route lane header must be present");
        let dataspace_header = ok_response
            .headers()
            .get("x-iroha-route-dataspace-id")
            .and_then(|value| value.to_str().ok())
            .expect("route dataspace header must be present");
        let routed_by = ok_response
            .headers()
            .get("x-iroha-routed-by")
            .and_then(|value| value.to_str().ok())
            .expect("routed-by header must be present");
        assert!(!lane_header.trim().is_empty());
        assert!(!dataspace_header.trim().is_empty());
        assert_eq!(routed_by, "local");

        let err = match super::handler_post_transaction(
            State(app),
            headers,
            None,
            versioned_signed_for_test(&tx2),
        )
        .await
        {
            Ok(_) => panic!("expected rate limit"),
            Err(err) => err,
        };
        assert_eq!(err.into_response().status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn handler_post_transaction_reports_full_queue_before_rate_limit() {
        let mut app = mk_app_state_for_tests();
        {
            let app_mut = Arc::get_mut(&mut app).expect("unique app state");
            app_mut.tx_rate_limiter = limits::RateLimiter::new(Some(1), Some(1));
            app_mut.fee_policy = FeePolicy::Disabled;
        }
        install_single_slot_transaction_queue(&mut app);

        let keypair = checked_torii_test_keypair_from_seed_byte(
            0xce,
            Algorithm::Ed25519,
            "derive queue-before-rate-limit fixture key",
        );
        let authority = AccountId::new(keypair.public_key().clone());
        let chain = (*app.chain_id).clone();
        let tx1 = TransactionBuilder::new(
            chain.clone(),
            authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "queue-before-rate-1".to_string())])
        .sign(keypair.private_key());
        let tx2 = TransactionBuilder::new(
            chain,
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "queue-before-rate-2".to_string())])
        .sign(keypair.private_key());
        let mut headers = HeaderMap::new();
        headers.insert("x-api-token", HeaderValue::from_static("queue-before-rate"));

        let first = super::handler_post_transaction(
            State(app.clone()),
            headers.clone(),
            None,
            versioned_signed_for_test(&tx1),
        )
        .await
        .expect("first transaction should fill the queue")
        .into_response();
        assert_eq!(first.status(), StatusCode::ACCEPTED);

        let err = match super::handler_post_transaction(
            State(app.clone()),
            headers,
            None,
            versioned_signed_for_test(&tx2),
        )
        .await
        {
            Ok(_) => panic!("expected queue full before token rate limit"),
            Err(err) => err,
        };
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            response
                .headers()
                .get("x-iroha-reject-code")
                .and_then(|value| value.to_str().ok()),
            Some("PRTRY:QUEUE_FULL")
        );
    }

    #[tokio::test]
    async fn handler_post_transaction_uses_api_token_rate_limit_key() {
        let mut app = mk_app_state_for_tests();
        {
            let app_mut = Arc::get_mut(&mut app).expect("unique app state");
            app_mut.high_load_tx_threshold = usize::MAX;
            app_mut.tx_rate_limiter = limits::RateLimiter::new(Some(1), Some(1));
            app_mut.fee_policy = FeePolicy::Disabled;
        }

        let first_keypair = checked_torii_test_keypair_from_seed_byte(
            0xc2,
            Algorithm::Ed25519,
            "derive first post-transaction API-token fixture key",
        );
        let second_keypair = checked_torii_test_keypair_from_seed_byte(
            0xc3,
            Algorithm::Ed25519,
            "derive second post-transaction API-token fixture key",
        );
        let chain = (*app.chain_id).clone();
        let tx1 = TransactionBuilder::new(
            chain.clone(),
            AccountId::new(first_keypair.public_key().clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "token-rate-limit-1".to_string())])
        .sign(first_keypair.private_key());
        let tx2 = TransactionBuilder::new(
            chain,
            AccountId::new(second_keypair.public_key().clone()),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "token-rate-limit-2".to_string())])
        .sign(second_keypair.private_key());
        let mut headers = HeaderMap::new();
        headers.insert("x-api-token", HeaderValue::from_static("shared-token"));

        let first = super::handler_post_transaction(
            State(app.clone()),
            headers.clone(),
            None,
            versioned_signed_for_test(&tx1),
        )
        .await
        .expect("first token-keyed transaction accepted")
        .into_response();
        assert_eq!(first.status(), StatusCode::ACCEPTED);

        let err = match super::handler_post_transaction(
            State(app),
            headers,
            None,
            versioned_signed_for_test(&tx2),
        )
        .await
        {
            Ok(_) => panic!("expected shared token rate limit"),
            Err(err) => err,
        };
        assert_eq!(err.into_response().status(), StatusCode::TOO_MANY_REQUESTS);
    }

    #[tokio::test]
    async fn handler_post_transaction_reuses_resolved_route_for_enqueue() {
        let mut app = mk_app_state_for_tests();
        let route_calls = install_counting_route_queue(&mut app);

        let keypair = checked_torii_test_keypair_from_seed_byte(
            0xc4,
            Algorithm::Ed25519,
            "derive post-transaction route-cache fixture key",
        );
        let authority = AccountId::new(keypair.public_key().clone());
        let transaction = TransactionBuilder::new(
            (*app.chain_id).clone(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "route-cache-submit".to_string())])
        .sign(keypair.private_key());

        let response = super::handler_post_transaction(
            State(app.clone()),
            HeaderMap::new(),
            None,
            versioned_signed_for_test(&transaction),
        )
        .await
        .expect("accepted")
        .into_response();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(app.queue.active_len(), 1);
        assert_eq!(
            route_calls.load(Ordering::Relaxed),
            1,
            "handler should route once and pass the resolved decision into queue push"
        );
    }

    #[tokio::test]
    async fn handler_post_transaction_entrypoint_accepts_external_entrypoint() {
        let mut app = mk_app_state_for_tests();
        Arc::get_mut(&mut app)
            .expect("unique app state")
            .high_load_tx_threshold = usize::MAX;

        let keypair = checked_torii_test_keypair_from_seed_byte(
            0xc5,
            Algorithm::Ed25519,
            "derive entrypoint external fixture key",
        );
        let authority = AccountId::new(keypair.public_key().clone());
        let transaction = TransactionBuilder::new(
            (*app.chain_id).clone(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, "entrypoint-submit".to_string())])
        .sign(keypair.private_key());
        let entrypoint = TransactionEntrypoint::External(transaction);

        let response = super::handler_post_transaction_entrypoint(
            State(app.clone()),
            HeaderMap::new(),
            None,
            versioned_entrypoint_for_test(entrypoint),
        )
        .await
        .expect("accepted")
        .into_response();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let entrypoint_hash = response
            .headers()
            .get("x-iroha-entrypoint-hash")
            .and_then(|value| value.to_str().ok())
            .expect("entrypoint hash header must be present");
        let tx_hash = response
            .headers()
            .get("x-iroha-transaction-hash")
            .and_then(|value| value.to_str().ok())
            .expect("transaction hash header must be present");
        assert_eq!(tx_hash, entrypoint_hash);
        assert_eq!(app.queue.active_len(), 1);
    }

    #[tokio::test]
    async fn handler_post_transaction_entrypoint_reuses_resolved_route_for_enqueue() {
        let mut app = mk_app_state_for_tests();
        let route_calls = install_counting_route_queue(&mut app);

        let keypair = checked_torii_test_keypair_from_seed_byte(
            0xc6,
            Algorithm::Ed25519,
            "derive entrypoint route-cache fixture key",
        );
        let authority = AccountId::new(keypair.public_key().clone());
        let transaction = TransactionBuilder::new(
            (*app.chain_id).clone(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(
            Level::INFO,
            "entrypoint-route-cache-submit".to_string(),
        )])
        .sign(keypair.private_key());
        let entrypoint = TransactionEntrypoint::External(transaction);

        let response = super::handler_post_transaction_entrypoint(
            State(app.clone()),
            HeaderMap::new(),
            None,
            versioned_entrypoint_for_test(entrypoint),
        )
        .await
        .expect("accepted")
        .into_response();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        assert_eq!(app.queue.active_len(), 1);
        assert_eq!(
            route_calls.load(Ordering::Relaxed),
            1,
            "entrypoint handler should route once and pass the resolved decision into queue push"
        );
    }

