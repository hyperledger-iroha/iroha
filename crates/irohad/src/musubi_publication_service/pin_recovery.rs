//! Restart-safe exact signed pin reconciliation before provider coordination.
//!
//! This owner joins the immutable signed-intent outbox to the daemon's same-view State/Kura pin
//! reader. It never signs, queues, pins, or treats transaction inclusion as success. Publication
//! remains disabled until a coordinator also proves replication and provider completions.
use super::{
    DurableMusubiPinIntentOutboxV1, MusubiPinIntentOutboxErrorV1,
    MusubiPublicationFinalizedArchiveRegistrationQueryV1,
    MusubiPublicationFinalizedPinRegistrationQueryV1,
    MusubiPublicationFinalizedPinRegistrationReadErrorV1,
    MusubiPublicationFinalizedPinRegistrationReaderV1, MusubiRecoveredSignedPinIntentV1,
};
use iroha_data_model::sorafs::pin_registry::{ManifestDigest, PinManifestRecord};

/// Closed read-only recovery failure before any provider or Queue effect.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MusubiPublicationPinRecoveryErrorV1 {
    /// The outbox, operation, source, or finalized pin does not match.
    Invalid,
    /// The named finalized source or pin is ahead of local State/Kura.
    LocallyAhead,
    /// Local immutable custody cannot be read at this moment.
    Unavailable,
}
impl core::fmt::Display for MusubiPublicationPinRecoveryErrorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::Invalid => "Musubi pin recovery evidence is invalid",
            Self::LocallyAhead => "Musubi pin recovery evidence is ahead of local finality",
            Self::Unavailable => "Musubi pin recovery custody is unavailable",
        })
    }
}
impl std::error::Error for MusubiPublicationPinRecoveryErrorV1 {}
impl From<MusubiPinIntentOutboxErrorV1> for MusubiPublicationPinRecoveryErrorV1 {
    fn from(error: MusubiPinIntentOutboxErrorV1) -> Self {
        match error {
            MusubiPinIntentOutboxErrorV1::LocallyAhead => Self::LocallyAhead,
            MusubiPinIntentOutboxErrorV1::Unavailable | MusubiPinIntentOutboxErrorV1::Locked => {
                Self::Unavailable
            }
            MusubiPinIntentOutboxErrorV1::Invalid
            | MusubiPinIntentOutboxErrorV1::Conflict
            | MusubiPinIntentOutboxErrorV1::Capacity
            | MusubiPinIntentOutboxErrorV1::MissingFinalizedAnchor => Self::Invalid,
        }
    }
}

/// An exact retained signed intent with independently verified successful pin finality.
#[derive(Debug)]
pub struct MusubiPublicationRecoveredFinalizedPinV1 {
    /// The original immutable operation, source, manifest digest, and signed wire.
    pub intent: MusubiRecoveredSignedPinIntentV1,
    /// Height of the successful result-bearing pin transaction.
    pub finalized_height: u64,
    /// Current authoritative nonretired pin record from the same State/Kura view.
    pub record: PinManifestRecord,
}

/// Daemon-owned read-only recovery join for one paid-pin authority and network.
pub struct MusubiPublicationPinRecoveryV1 {
    outbox: DurableMusubiPinIntentOutboxV1,
    reader: MusubiPublicationFinalizedPinRegistrationReaderV1,
}
impl core::fmt::Debug for MusubiPublicationPinRecoveryV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("MusubiPublicationPinRecoveryV1")
            .finish_non_exhaustive()
    }
}
impl MusubiPublicationPinRecoveryV1 {
    /// Bind immutable signed-intent custody to the exact State/Kura reader authority.
    ///
    /// # Errors
    /// Rejects any network or public pin-account mismatch before recovery begins.
    pub fn new(
        outbox: DurableMusubiPinIntentOutboxV1,
        reader: MusubiPublicationFinalizedPinRegistrationReaderV1,
    ) -> Result<Self, MusubiPublicationPinRecoveryErrorV1> {
        if outbox.owner_binding() != reader.binding() {
            return Err(MusubiPublicationPinRecoveryErrorV1::Invalid);
        }
        Ok(Self { outbox, reader })
    }

    /// Recover the exact original signed wire for one operation without re-signing.
    ///
    /// A missing expected operation fails closed because local absence cannot establish that no
    /// earlier signed wire was dispatched. A retained operation with a different requested source
    /// is a permanent conflict, including when the manifest and transaction bytes happen to match.
    ///
    /// # Errors
    /// Refuses absent intent, substituted custody, stale source finality, or changed binding.
    pub fn recover_exact_operation(
        &self,
        operation_id: [u8; 32],
        source: &MusubiPublicationFinalizedArchiveRegistrationQueryV1,
    ) -> Result<MusubiRecoveredSignedPinIntentV1, MusubiPublicationPinRecoveryErrorV1> {
        self.outbox.verify_requested_source(source)?;
        let intent = self.outbox.recover_operation(operation_id)?;
        if intent.source != *source {
            return Err(MusubiPublicationPinRecoveryErrorV1::Invalid);
        }
        Ok(intent)
    }

    /// Report completion only after the exact signed network transaction succeeded and the
    /// current authoritative pin record still matches it.
    ///
    /// This is one pin-registration recovery gate, not a claim of complete publication: the
    /// replication order, three independent providers, two readbacks, and State-root publication
    /// remain separate requirements.
    ///
    /// # Errors
    /// Refuses absent intent, failed inclusion, wrong pin occurrence, stale/retired record, or
    /// finality artifacts not available on this node.
    pub fn recover_finalized_pin(
        &self,
        operation_id: [u8; 32],
        source: &MusubiPublicationFinalizedArchiveRegistrationQueryV1,
        manifest_digest: ManifestDigest,
        finalized_height: u64,
    ) -> Result<MusubiPublicationRecoveredFinalizedPinV1, MusubiPublicationPinRecoveryErrorV1> {
        self.recover_finalized_pin_with(
            operation_id,
            source,
            manifest_digest,
            finalized_height,
            |query| self.reader.read_current_pin(query),
        )
    }

    fn recover_finalized_pin_with(
        &self,
        operation_id: [u8; 32],
        source: &MusubiPublicationFinalizedArchiveRegistrationQueryV1,
        manifest_digest: ManifestDigest,
        finalized_height: u64,
        read_pin: impl FnOnce(
            &MusubiPublicationFinalizedPinRegistrationQueryV1,
        ) -> Result<
            PinManifestRecord,
            MusubiPublicationFinalizedPinRegistrationReadErrorV1,
        >,
    ) -> Result<MusubiPublicationRecoveredFinalizedPinV1, MusubiPublicationPinRecoveryErrorV1> {
        if finalized_height == 0 {
            return Err(MusubiPublicationPinRecoveryErrorV1::Invalid);
        }
        let intent = self.recover_exact_operation(operation_id, source)?;
        if intent.manifest_digest != manifest_digest {
            return Err(MusubiPublicationPinRecoveryErrorV1::Invalid);
        }
        let query = MusubiPublicationFinalizedPinRegistrationQueryV1 {
            version: 1,
            source: intent.source.clone(),
            transaction: intent.transaction.clone(),
            manifest_digest,
            finalized_height,
        };
        let record = read_pin(&query).map_err(|error| match error {
            MusubiPublicationFinalizedPinRegistrationReadErrorV1::LocallyAhead => {
                MusubiPublicationPinRecoveryErrorV1::LocallyAhead
            }
            MusubiPublicationFinalizedPinRegistrationReadErrorV1::Invalid => {
                MusubiPublicationPinRecoveryErrorV1::Invalid
            }
        })?;
        Ok(MusubiPublicationRecoveredFinalizedPinV1 {
            intent,
            finalized_height,
            record,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::musubi_publication_service::finality::tests::reader_fixture;
    use iroha_config::parameters::actual::MusubiPublicationPaidPinPolicy;
    use iroha_core::state::State;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        account::AccountId,
        asset::AssetDefinitionId,
        isi::{InstructionBox, sorafs::RegisterPinManifest},
        sorafs::pin_registry::{PinFeePayment, PinPolicy as RegistryPinPolicy, StorageClass},
        transaction::{
            DEFAULT_TRANSACTION_TIME_TO_LIVE, FeePaymentIntent, SignedTransaction,
            TransactionBuilder,
        },
    };
    use iroha_model_base::{domain::DomainId, metadata::Metadata};
    use iroha_primitives::numeric::Quantity;
    use sorafs_manifest::{
        DagCodecId, ManifestBuilder, PinPolicy, ProfileId, StorageClass as ManifestStorageClass,
    };
    use std::{fs, os::unix::fs::PermissionsExt as _, sync::Arc, time::Duration};

    struct Fixture {
        _root: tempfile::TempDir,
        outbox: DurableMusubiPinIntentOutboxV1,
        reader: MusubiPublicationFinalizedPinRegistrationReaderV1,
        source: MusubiPublicationFinalizedArchiveRegistrationQueryV1,
        digest: ManifestDigest,
        transaction: SignedTransaction,
        state: Arc<State>,
        archive_reader: super::super::MusubiPublicationFinalizedArchiveRegistrationReaderV1,
    }
    fn fixture() -> Fixture {
        let finalized = reader_fixture();
        let key =
            KeyPair::try_from_seed(vec![0x7a; 32], Algorithm::Ed25519).expect("pin authority key");
        let authority = AccountId::new(key.public_key().clone());
        let archive = &finalized.query.registration.commitment;
        let manifest = ManifestBuilder::new()
            .root_cid(archive.root_cid.as_bytes().to_vec())
            .dag_codec(DagCodecId(sorafs_manifest::MANIFEST_DAG_CODEC))
            .chunking_from_registry(ProfileId(1))
            .chunk_digest_sha3_256(*archive.chunk_plan_digest.as_bytes())
            .por_root(*archive.por_root.as_bytes())
            .content_length(archive.content_length)
            .car_digest(*archive.car_digest.as_bytes())
            .car_size(archive.car_size)
            .pin_policy(PinPolicy {
                min_replicas: 3,
                storage_class: ManifestStorageClass::Hot,
                retention_epoch: 43
                    + DEFAULT_TRANSACTION_TIME_TO_LIVE.as_secs()
                    + 30 * 24 * 60 * 60,
            })
            .build()
            .expect("canonical pin manifest");
        let digest = ManifestDigest::from_manifest(&manifest).expect("manifest digest");
        let mut builder = TransactionBuilder::new(
            finalized.query.network_id,
            authority.clone(),
            FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(vec![InstructionBox::from(RegisterPinManifest::new(
            manifest.encode().expect("canonical manifest bytes"),
            None,
            None,
        ))]);
        builder.set_creation_time(Duration::from_millis(42_001));
        let transaction = builder.sign(key.private_key());
        let policy = MusubiPublicationPaidPinPolicy {
            storage_class: StorageClass::Hot,
            retention_horizon_secs: 30 * 24 * 60 * 60,
            transaction_authority: authority.clone(),
        };
        let root = tempfile::tempdir().expect("private outbox root");
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o700)).expect("private mode");
        let canonical_root = root.path().canonicalize().expect("canonical root");
        let limits = super::super::MusubiPinIntentOutboxLimitsV1 {
            max_records: 1,
            max_total_bytes: 16 * 1024 * 1024,
        };
        DurableMusubiPinIntentOutboxV1::initialize(
            &canonical_root,
            finalized.query.network_id,
            [0xa1; 32],
            policy.clone(),
            limits,
        )
        .expect("initialize outbox");
        let mut outbox = DurableMusubiPinIntentOutboxV1::open_inner(
            &canonical_root,
            finalized.query.network_id,
            policy,
            limits,
            finalized.reader.clone(),
        )
        .expect("open outbox");
        outbox
            .stage_signed_intent([0x61; 32], &finalized.query, digest, &transaction)
            .expect("stage signed intent");
        let reader = MusubiPublicationFinalizedPinRegistrationReaderV1::from_test_handles(
            finalized.query.network_id,
            authority,
            Arc::clone(&finalized.state),
            finalized.reader.clone(),
        );
        Fixture {
            _root: root,
            outbox,
            reader,
            source: finalized.query,
            digest,
            transaction,
            state: finalized.state,
            archive_reader: finalized.reader,
        }
    }

    #[test]
    fn exact_operation_recovery_reuses_original_wire_and_rejects_changed_source() {
        let Fixture {
            outbox,
            reader,
            source,
            digest,
            transaction,
            _root,
            ..
        } = fixture();
        let recovery =
            MusubiPublicationPinRecoveryV1::new(outbox, reader).expect("same public pin binding");
        let retained = recovery
            .recover_exact_operation([0x61; 32], &source)
            .expect("retained operation");
        assert_eq!(retained.manifest_digest, digest);
        assert_eq!(
            retained
                .transaction
                .encode_wire_v1()
                .expect("retained wire"),
            transaction.encode_wire_v1().expect("original wire"),
        );
        assert!(matches!(
            recovery.recover_exact_operation([0x62; 32], &source),
            Err(MusubiPublicationPinRecoveryErrorV1::Invalid),
        ));
        let mut substituted = source.clone();
        substituted.expected_policy_revision += 1;
        assert!(matches!(
            recovery.recover_exact_operation([0x61; 32], &substituted),
            Err(MusubiPublicationPinRecoveryErrorV1::Invalid),
        ));
        assert!(matches!(
            recovery.recover_exact_operation([0x62; 32], &substituted),
            Err(MusubiPublicationPinRecoveryErrorV1::Invalid),
        ));
        assert!(matches!(
            recovery.recover_exact_operation([0; 32], &source),
            Err(MusubiPublicationPinRecoveryErrorV1::Invalid),
        ));
    }

    #[test]
    fn exact_signed_intent_and_finalized_reader_result_join_without_resigning() {
        let Fixture {
            outbox,
            reader,
            source,
            digest,
            transaction,
            _root,
            ..
        } = fixture();
        let recovery =
            MusubiPublicationPinRecoveryV1::new(outbox, reader).expect("same public pin binding");
        let archive = &source.registration.commitment;
        let authority = transaction.authority().clone();
        let height = source.registration.registered_at_height + 1;
        let mut record = PinManifestRecord::new(
            digest,
            archive.root_cid.clone(),
            archive.chunker.clone(),
            *archive.chunk_plan_digest.as_bytes(),
            *archive.por_root.as_bytes(),
            archive.content_length,
            RegistryPinPolicy {
                min_replicas: 3,
                storage_class: StorageClass::Hot,
                retention_epoch: 43
                    + DEFAULT_TRANSACTION_TIME_TO_LIVE.as_secs()
                    + 30 * 24 * 60 * 60,
            },
            authority.clone(),
            height,
            None,
            None,
            Metadata::default(),
        );
        record.record_pin_fee_payment(PinFeePayment {
            paid_by: authority.clone(),
            fee_asset_id: AssetDefinitionId::derive_from_components(
                DomainId::try_new("sora", "universal").expect("fee domain"),
                "xor".parse().expect("fee name"),
            ),
            treasury_account_id: authority,
            amount: Quantity::from(1_u32),
        });
        let result = recovery
            .recover_finalized_pin_with([0x61; 32], &source, digest, height, |query| {
                assert_eq!(query.version, 1);
                assert_eq!(query.source, source);
                assert_eq!(query.manifest_digest, digest);
                assert_eq!(query.finalized_height, height);
                assert_eq!(
                    query.transaction.encode_wire_v1().expect("query wire"),
                    transaction.encode_wire_v1().expect("original wire"),
                );
                Ok(record.clone())
            })
            .expect("exact reader result joins retained intent");
        assert_eq!(result.intent.operation_id, [0x61; 32]);
        assert_eq!(result.intent.manifest_digest, digest);
        assert_eq!(result.finalized_height, height);
        assert_eq!(result.record, record);
    }

    #[test]
    fn foreign_reader_and_unfinalized_pin_never_report_completion() {
        let first = fixture();
        let foreign = AccountId::new(
            KeyPair::try_from_seed(vec![0x7b; 32], Algorithm::Ed25519)
                .expect("foreign authority key")
                .public_key()
                .clone(),
        );
        let foreign_reader = MusubiPublicationFinalizedPinRegistrationReaderV1::from_test_handles(
            first.source.network_id,
            foreign,
            Arc::clone(&first.state),
            first.archive_reader.clone(),
        );
        assert!(matches!(
            MusubiPublicationPinRecoveryV1::new(first.outbox, foreign_reader),
            Err(MusubiPublicationPinRecoveryErrorV1::Invalid),
        ));
        let Fixture {
            outbox,
            reader,
            source,
            digest,
            _root,
            ..
        } = fixture();
        let recovery =
            MusubiPublicationPinRecoveryV1::new(outbox, reader).expect("same public pin binding");
        for (operation, manifest, height) in [
            ([0; 32], digest, 1),
            (
                [0x62; 32],
                digest,
                source.registration.registered_at_height + 1,
            ),
            ([0x61; 32], digest, 0),
            ([0x61; 32], ManifestDigest::new([0x91; 32]), 1),
            ([0x61; 32], digest, source.registration.registered_at_height),
        ] {
            assert!(matches!(
                recovery.recover_finalized_pin(operation, &source, manifest, height),
                Err(MusubiPublicationPinRecoveryErrorV1::Invalid),
            ));
        }
    }

    #[test]
    fn closed_error_mapping_preserves_local_deferral_only() {
        assert_eq!(
            MusubiPublicationPinRecoveryErrorV1::from(MusubiPinIntentOutboxErrorV1::LocallyAhead),
            MusubiPublicationPinRecoveryErrorV1::LocallyAhead,
        );
        assert_eq!(
            MusubiPublicationPinRecoveryErrorV1::from(MusubiPinIntentOutboxErrorV1::Unavailable),
            MusubiPublicationPinRecoveryErrorV1::Unavailable,
        );
        assert_eq!(
            MusubiPublicationPinRecoveryErrorV1::from(MusubiPinIntentOutboxErrorV1::Conflict),
            MusubiPublicationPinRecoveryErrorV1::Invalid,
        );
        assert_eq!(
            MusubiPublicationPinRecoveryErrorV1::from(
                MusubiPinIntentOutboxErrorV1::MissingFinalizedAnchor
            ),
            MusubiPublicationPinRecoveryErrorV1::Invalid,
        );
        assert!(
            MusubiPinIntentOutboxErrorV1::MissingFinalizedAnchor
                .to_string()
                .contains("finalized monotonic rollback anchor")
        );
        assert!(
            !MusubiPublicationPinRecoveryErrorV1::Invalid
                .to_string()
                .is_empty()
        );
    }
}
