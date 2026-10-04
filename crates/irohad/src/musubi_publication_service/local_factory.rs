//! Concrete daemon-owned assembly of the private Musubi publication protocol core.
//!
//! The factory reopens the original replay journal, seed custody, and durable clock before
//! constructing the service. Stock startup selects the complete configured installation and
//! supplies its runtime signer, original provider inventory, fresh native discovery, paid-pin
//! coordination and private TLS ingress boundaries.
// TODO: Qualify the installed listener and complete three-provider paid publication/recovery
// path on a current four-validator network and each supported native operating system.
use super::{
    MusubiPublicationAuthenticatedProviderReadbackV1,
    MusubiPublicationFinalizedArchiveRegistrationReaderV1,
    MusubiPublicationFinalizedSeedReadCapabilityV1, MusubiPublicationPrivateDeploymentV1,
    MusubiPublicationPrivateServiceContextV1, MusubiPublicationPrivateServiceFactoryErrorV1,
    MusubiPublicationPrivateServiceFactoryV1, shared_seed_staging::SharedSeedStagingBackendV1,
    storage_coordination::FinalizedRegistrationCheckedStorageBackendV1,
};
use iroha_config::parameters::actual::MusubiPublicationPaidPinPolicy;
use iroha_data_model::{NetworkId, account::AccountId, sorafs::capacity::ProviderId};
use iroha_musubi_service::{
    DurableMusubiPublicationServiceClockV1, DurableMusubiPublicationServiceJournalLimitsV1,
    MusubiPublicationPrivateServiceV1, MusubiPublicationServiceBackendErrorV1,
    MusubiPublicationServiceClockV1, MusubiPublicationServiceConfigurationV1,
    MusubiSeedIngressReceiptSigningProviderV1, MusubiStorageCoordinationBackendV1,
};
use iroha_storage_client::musubi_archive_fetch::AuthenticatedMusubiArchiveFetchClientV1;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

// Both adapters hold the sole native clock lease. A sample releases the mutex before the
// caller performs signatures, Queue admission, provider I/O or HTTP.
#[derive(Clone)]
struct SharedPublicationClock(Arc<Mutex<DurableMusubiPublicationServiceClockV1>>);
impl MusubiPublicationServiceClockV1 for SharedPublicationClock {
    fn current_time_ms(&mut self) -> Result<u64, MusubiPublicationServiceBackendErrorV1> {
        self.0
            .lock()
            .map_err(|_| MusubiPublicationServiceBackendErrorV1::Retryable)?
            .current_time_ms()
    }
}

/// Non-secret local paths, bounds, and public identity for one injected publication service.
///
/// A deployment must derive these values from `iroha_config` and explicitly initialize the
/// journal, seed-owner marker, and durable clock in native private custody at provisioning time.
/// Ordinary startup only reopens these initialized owners; a missing marker is not repaired.
pub struct MusubiPublicationPrivateLocalFactorySettingsV1 {
    /// Exact public service identity and timing limits.
    pub service: MusubiPublicationServiceConfigurationV1,
    /// Initialized durable replay-journal directory.
    pub journal_root: PathBuf,
    /// Initialized private exact-CAR seed staging directory.
    pub seed_root: PathBuf,
    /// Initialized durable clock-floor directory.
    pub clock_root: PathBuf,
    /// Bound on durable journal records and bytes.
    pub journal_limits: DurableMusubiPublicationServiceJournalLimitsV1,
    /// Maximum retained exact seed records.
    pub max_seed_records: u32,
    /// Maximum retained exact seed bytes.
    pub max_seed_bytes: u64,
    /// Non-secret paid-pin policy and exact public account expected to fund the transaction.
    pub paid_pin: MusubiPublicationPaidPinPolicy,
}
impl MusubiPublicationPrivateLocalFactorySettingsV1 {
    /// Project the configured non-secret custody root into three disjoint durable owners.
    ///
    /// The public network, broker and provider identities come from the live deployment; timing
    /// and retention policy come from the configuration file. Signer credentials, TLS keys and
    /// provider authentication stay in separate runtime custody. This projection alone does not
    /// enable private publication.
    ///
    /// # Errors
    /// Rejects invalid journal retention limits before opening any custody owner.
    pub fn from_config(
        config: &iroha_config::parameters::actual::MusubiPublication,
        network_id: NetworkId,
        ingress_broker: AccountId,
        seed_provider: ProviderId,
    ) -> Result<Self, MusubiPublicationPrivateServiceFactoryErrorV1> {
        if config.pin_retention_horizon_secs
            <= u64::from(
                iroha_data_model::sorafs::pin_registry::SORAFS_AUTO_REPLICATION_ORDER_INGEST_DEADLINE_SECS_V1,
            )
            || config.pin_retention_horizon_secs
                > iroha_config::parameters::defaults::musubi_publication::MAX_PIN_RETENTION_HORIZON_SECS
        {
            return Err(MusubiPublicationPrivateServiceFactoryErrorV1::Unqualified);
        }
        let journal_limits = DurableMusubiPublicationServiceJournalLimitsV1::new(
            config.journal_max_operations,
            config.journal_max_authorizations,
            config.journal_max_total_response_bytes,
            config.journal_max_snapshot_bytes,
        )
        .map_err(|_| MusubiPublicationPrivateServiceFactoryErrorV1::Unqualified)?;
        let paid_pin = config.paid_pin_policy(&ingress_broker);
        Ok(Self {
            service: MusubiPublicationServiceConfigurationV1 {
                network_id,
                ingress_broker,
                seed_provider,
                max_future_clock_skew_ms: config.max_future_clock_skew_ms,
                receipt_lifetime_ms: config.receipt_lifetime_ms,
            },
            journal_root: config.custody_root.join("journal"),
            seed_root: config.custody_root.join("seed"),
            clock_root: config.custody_root.join("clock"),
            journal_limits,
            max_seed_records: config.max_seed_records,
            max_seed_bytes: config.max_seed_bytes,
            paid_pin,
        })
    }
}

/// Deployment-owned provider mutation builder that receives verified finalized-state custody.
pub trait MusubiPublicationPrivateStorageBuilderV1: Send + 'static {
    /// Build a coordination backend from the daemon's live handles and finalized reader.
    ///
    /// # Errors
    /// Refuses unavailable or unqualified provider admission before TLS ingress starts.
    fn build(
        self: Box<Self>,
        context: &MusubiPublicationPrivateServiceContextV1,
        finalized_reader: MusubiPublicationFinalizedArchiveRegistrationReaderV1,
        finalized_seed: MusubiPublicationFinalizedSeedReadCapabilityV1,
        paid_pin: MusubiPublicationPaidPinPolicy,
        clock: Box<dyn MusubiPublicationServiceClockV1>,
    ) -> Result<
        Box<dyn MusubiStorageCoordinationBackendV1>,
        MusubiPublicationPrivateServiceFactoryErrorV1,
    >;
}

/// Deployment-owned private TLS ingress builder for the exact assembled protocol core.
pub trait MusubiPublicationPrivateIngressBuilderV1: Send + 'static {
    /// Bind a qualified private TLS listener to the service before supervisor publication.
    ///
    /// # Errors
    /// Refuses an unavailable or unqualified listener without starting a child.
    fn build(
        self: Box<Self>,
        service: MusubiPublicationPrivateServiceV1,
    ) -> Result<MusubiPublicationPrivateDeploymentV1, MusubiPublicationPrivateServiceFactoryErrorV1>;
}

/// Factory assembling the actual durable publication core from daemon-owned custody.
///
/// The receipt signer, authenticated provider client, and admitted advert cache enter through
/// runtime-only custody;
/// neither credential is serialized into public settings or a returned error. The storage and
/// ingress builders remain explicit deployment qualifications, so constructing this factory does
/// not by itself activate the stock service.
pub struct MusubiPublicationPrivateLocalFactoryV1 {
    settings: MusubiPublicationPrivateLocalFactorySettingsV1,
    receipt_signer: Box<dyn MusubiSeedIngressReceiptSigningProviderV1>,
    storage_builder: Box<dyn MusubiPublicationPrivateStorageBuilderV1>,
    readback_client: AuthenticatedMusubiArchiveFetchClientV1,
    provider_adverts: Arc<tokio::sync::RwLock<iroha_torii::sorafs::ProviderAdvertCache>>,
    ingress_builder: Box<dyn MusubiPublicationPrivateIngressBuilderV1>,
}
impl MusubiPublicationPrivateLocalFactoryV1 {
    /// Retain deployment-owned dependencies until the daemon's State and queue exist.
    #[must_use]
    pub fn new(
        settings: MusubiPublicationPrivateLocalFactorySettingsV1,
        receipt_signer: Box<dyn MusubiSeedIngressReceiptSigningProviderV1>,
        storage_builder: Box<dyn MusubiPublicationPrivateStorageBuilderV1>,
        readback_client: AuthenticatedMusubiArchiveFetchClientV1,
        provider_adverts: Arc<tokio::sync::RwLock<iroha_torii::sorafs::ProviderAdvertCache>>,
        ingress_builder: Box<dyn MusubiPublicationPrivateIngressBuilderV1>,
    ) -> Self {
        Self {
            settings,
            receipt_signer,
            storage_builder,
            readback_client,
            provider_adverts,
            ingress_builder,
        }
    }
}
impl MusubiPublicationPrivateServiceFactoryV1 for MusubiPublicationPrivateLocalFactoryV1 {
    fn build(
        self: Box<Self>,
        context: MusubiPublicationPrivateServiceContextV1,
    ) -> Result<MusubiPublicationPrivateDeploymentV1, MusubiPublicationPrivateServiceFactoryErrorV1>
    {
        let Self {
            settings,
            receipt_signer,
            storage_builder,
            readback_client,
            provider_adverts,
            ingress_builder,
        } = *self;
        if settings.service.network_id != context.network_id() {
            return Err(MusubiPublicationPrivateServiceFactoryErrorV1::Unqualified);
        }
        let readback = MusubiPublicationAuthenticatedProviderReadbackV1::new(
            &context,
            readback_client,
            provider_adverts,
        )
        .map_err(|_| MusubiPublicationPrivateServiceFactoryErrorV1::Unqualified)?;
        let custody = context
            .open_local_publication_custody(
                &settings.journal_root,
                &settings.seed_root,
                &settings.service,
                settings.journal_limits,
                settings.max_seed_records,
                settings.max_seed_bytes,
            )
            .map_err(|_| MusubiPublicationPrivateServiceFactoryErrorV1::Unavailable)?;
        let clock = DurableMusubiPublicationServiceClockV1::open_system(&settings.clock_root)
            .map_err(|_| MusubiPublicationPrivateServiceFactoryErrorV1::Unavailable)?;
        let clock = SharedPublicationClock(Arc::new(Mutex::new(clock)));
        let (journal, seed, finalized_reader) = custody.into_parts();
        let (shared_seed, finalized_seed) =
            SharedSeedStagingBackendV1::share(seed, finalized_reader.clone());
        // The deployment-selected coordinator may perform irreversible SoraFS effects. Require
        // this daemon-owned finality check even when its builder does not use the reader.
        let storage = storage_builder.build(
            &context,
            finalized_reader.clone(),
            finalized_seed,
            settings.paid_pin.clone(),
            Box::new(clock.clone()),
        )?;
        let storage = Box::new(FinalizedRegistrationCheckedStorageBackendV1::new(
            finalized_reader,
            storage,
        ));
        let service = MusubiPublicationPrivateServiceV1::new(
            settings.service,
            Box::new(clock),
            receipt_signer,
            Box::new(journal),
            Box::new(shared_seed),
            storage,
            Box::new(readback),
        )
        .map_err(|_| MusubiPublicationPrivateServiceFactoryErrorV1::Unqualified)?;
        ingress_builder.build(service)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::{account::AccountId, block::BlockHeader, sorafs::capacity::ProviderId};

    #[test]
    fn configured_custody_root_projects_only_non_secret_owner_paths() {
        let root = iroha_config::parameters::actual::MusubiPublication {
            custody_root: PathBuf::from("/private/musubi-v1"),
            journal_max_operations: 2,
            journal_max_authorizations: 4,
            journal_max_total_response_bytes: 16 * 1024 * 1024,
            journal_max_snapshot_bytes: 17 * 1024 * 1024,
            max_seed_records: 3,
            max_seed_bytes: 128 * 1024 * 1024,
            max_future_clock_skew_ms: 0,
            receipt_lifetime_ms: 60_000,
            ..Default::default()
        };
        let broker_key =
            KeyPair::try_from_seed(vec![0x37; 32], Algorithm::Ed25519).expect("broker test key");
        let service = MusubiPublicationServiceConfigurationV1 {
            network_id: iroha_data_model::NetworkId::from_genesis_hash(
                HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x15; 32])),
            ),
            ingress_broker: AccountId::new(broker_key.public_key().clone()),
            seed_provider: ProviderId::new([0x38; 32]),
            max_future_clock_skew_ms: 0,
            receipt_lifetime_ms: 60_000,
        };
        let settings = MusubiPublicationPrivateLocalFactorySettingsV1::from_config(
            &root,
            service.network_id,
            service.ingress_broker.clone(),
            service.seed_provider,
        )
        .expect("configured limits");
        let limits = DurableMusubiPublicationServiceJournalLimitsV1::new(
            root.journal_max_operations,
            root.journal_max_authorizations,
            root.journal_max_total_response_bytes,
            root.journal_max_snapshot_bytes,
        )
        .expect("valid configured limits");
        assert_eq!(
            settings.service.max_future_clock_skew_ms,
            root.max_future_clock_skew_ms
        );
        assert_eq!(
            settings.service.receipt_lifetime_ms,
            root.receipt_lifetime_ms
        );
        assert_eq!(settings.service.ingress_broker, service.ingress_broker);
        assert_eq!(settings.service.seed_provider, service.seed_provider);
        assert_eq!(settings.service, service);
        assert_eq!(settings.journal_root, root.custody_root.join("journal"));
        assert_eq!(settings.seed_root, root.custody_root.join("seed"));
        assert_eq!(settings.clock_root, root.custody_root.join("clock"));
        assert_eq!(settings.journal_limits, limits);
        assert_eq!(settings.max_seed_records, root.max_seed_records);
        assert_eq!(settings.max_seed_bytes, root.max_seed_bytes);
        assert_eq!(
            settings.paid_pin.storage_class,
            iroha_data_model::sorafs::pin_registry::StorageClass::Hot,
        );
        assert_eq!(
            settings.paid_pin.retention_horizon_secs,
            iroha_config::parameters::defaults::musubi_publication::PIN_RETENTION_HORIZON_SECS,
        );
        assert_eq!(
            settings.paid_pin.transaction_authority,
            service.ingress_broker
        );

        let other_key =
            KeyPair::try_from_seed(vec![0x39; 32], Algorithm::Ed25519).expect("pin authority");
        let other = AccountId::new(other_key.public_key().clone());
        let mut configured_pin = root.clone();
        configured_pin.pin_storage_class =
            iroha_data_model::sorafs::pin_registry::StorageClass::Warm;
        configured_pin.pin_retention_horizon_secs = 90 * 24 * 60 * 60;
        configured_pin.pin_transaction_authority =
            iroha_config::parameters::actual::MusubiPinTransactionAuthority::Account(other.clone());
        let selected = MusubiPublicationPrivateLocalFactorySettingsV1::from_config(
            &configured_pin,
            service.network_id,
            service.ingress_broker.clone(),
            service.seed_provider,
        )
        .expect("configured pin identity");
        assert_eq!(selected.paid_pin.transaction_authority, other);
        assert_eq!(selected.paid_pin.retention_horizon_secs, 90 * 24 * 60 * 60);
        assert_eq!(
            selected.paid_pin.storage_class,
            iroha_data_model::sorafs::pin_registry::StorageClass::Warm,
        );
        configured_pin.pin_retention_horizon_secs =
            u64::from(
                iroha_data_model::sorafs::pin_registry::SORAFS_AUTO_REPLICATION_ORDER_INGEST_DEADLINE_SECS_V1,
            );
        assert!(matches!(
            MusubiPublicationPrivateLocalFactorySettingsV1::from_config(
                &configured_pin,
                service.network_id,
                service.ingress_broker.clone(),
                service.seed_provider,
            ),
            Err(MusubiPublicationPrivateServiceFactoryErrorV1::Unqualified),
        ));

        let invalid = iroha_config::parameters::actual::MusubiPublication {
            journal_max_operations: 0,
            ..root
        };
        assert!(matches!(
            MusubiPublicationPrivateLocalFactorySettingsV1::from_config(
                &invalid,
                service.network_id,
                service.ingress_broker,
                service.seed_provider,
            ),
            Err(MusubiPublicationPrivateServiceFactoryErrorV1::Unqualified),
        ));
    }
}

#[cfg(test)]
mod shared_clock_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    struct Source(Arc<AtomicU64>);
    impl MusubiPublicationServiceClockV1 for Source {
        fn current_time_ms(&mut self) -> Result<u64, MusubiPublicationServiceBackendErrorV1> {
            Ok(self.0.load(Ordering::SeqCst))
        }
    }
    #[test]
    fn both_adapters_share_one_durable_floor_and_hold_one_lock_until_last_drop() {
        let temporary = tempfile::tempdir().unwrap();
        let path = temporary.path().join("clock");
        let owner = iroha_fs::PrivateDirectory::open_or_create(&path).unwrap();
        let value = Arc::new(AtomicU64::new(100));
        let clock = DurableMusubiPublicationServiceClockV1::initialize(
            owner.path(),
            Box::new(Source(Arc::clone(&value))),
        )
        .unwrap();
        let mut service = SharedPublicationClock(Arc::new(Mutex::new(clock)));
        let mut backend = service.clone();
        value.store(200, Ordering::SeqCst);
        assert_eq!(service.current_time_ms().unwrap(), 200);
        value.store(300, Ordering::SeqCst);
        assert_eq!(backend.current_time_ms().unwrap(), 300);
        assert_eq!(service.0.lock().unwrap().durable_floor_ms(), 300);
        drop(service);
        assert!(
            DurableMusubiPublicationServiceClockV1::open(
                owner.path(),
                Box::new(Source(Arc::clone(&value)))
            )
            .is_err()
        );
        drop(backend);
        let reopened =
            DurableMusubiPublicationServiceClockV1::open(owner.path(), Box::new(Source(value)))
                .unwrap();
        assert_eq!(reopened.durable_floor_ms(), 300);
    }
}
