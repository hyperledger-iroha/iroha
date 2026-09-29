//! Checked local stage and recovery of one signed Musubi pin intent.
//!
//! This coordinator retains the process-exclusive outbox and reads the daemon's current
//! finalized State/Kura high-water before and after each operation. It has no Queue or provider
//! capability. Its results are local observations and a native advance instruction, never
//! authorization to submit a pin transaction. Stock outbox open remains closed until an
//! independently current network checkpoint and deployment-sealed rollback protection exist.
use super::{
    DurableMusubiPinIntentOutboxV1, MusubiPinIntentOutboxErrorV1,
    MusubiPublicationFinalizedArchiveRegistrationQueryV1,
    MusubiPublicationPinOutboxHighWaterReadErrorV1, MusubiPublicationPinOutboxHighWaterReaderV1,
};
use iroha_data_model::{
    account::AccountId, isi::musubi::AdvanceMusubiPinOutboxV1, musubi::MusubiPinOutboxHighWaterV1,
    sorafs::pin_registry::ManifestDigest, transaction::SignedTransaction,
};

/// Local-only outcome of a durably staged signed pin intent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MusubiPinOutboxCheckedStageV1 {
    /// Submit this native compare-and-set advance and wait for its finalized result in a future
    /// qualified coordinator. This is not permission to submit the retained pin transaction.
    NeedsFinality(AdvanceMusubiPinOutboxV1),
    /// The exact complete local inventory matches a finalized State/Kura high-water. A current
    /// independent network checkpoint is still required before any Queue effect.
    LocallyFinalized {
        /// Finalized native inventory revision.
        revision: u64,
        /// Exact complete immutable inventory digest.
        inventory_digest: [u8; 32],
    },
}

/// Redacted, non-authorizing observation of one exact retained signed intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MusubiPinOutboxCheckedRecoveryV1 {
    /// Original immutable publication operation.
    pub operation_id: [u8; 32],
    /// Canonical pin manifest digest.
    pub manifest_digest: ManifestDigest,
    /// Hash of the recovered exact signed transaction.
    pub transaction_hash: [u8; 32],
    /// Finalized local inventory revision that covers this intent.
    pub revision: u64,
    /// Exact complete inventory digest at that revision.
    pub inventory_digest: [u8; 32],
}

/// Daemon-owned, process-exclusive local stage/recovery join.
///
/// The raw outbox can only be opened through its currently closed daemon constructor; this
/// type therefore does not activate stock signing or Queue/provider mutation.
pub struct MusubiPublicationPinOutboxCheckedCoordinatorV1 {
    outbox: DurableMusubiPinIntentOutboxV1,
    high_water_reader: MusubiPublicationPinOutboxHighWaterReaderV1,
}
impl core::fmt::Debug for MusubiPublicationPinOutboxCheckedCoordinatorV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("MusubiPublicationPinOutboxCheckedCoordinatorV1")
            .finish_non_exhaustive()
    }
}
impl MusubiPublicationPinOutboxCheckedCoordinatorV1 {
    /// Bind the original outbox to a finalized reader for the same daemon network.
    ///
    /// # Errors
    /// Rejects a reader for a different network before retaining either capability.
    pub fn new(
        outbox: DurableMusubiPinIntentOutboxV1,
        high_water_reader: MusubiPublicationPinOutboxHighWaterReaderV1,
    ) -> Result<Self, MusubiPinIntentOutboxErrorV1> {
        if outbox.owner_binding().0 != high_water_reader.network_id() {
            return Err(MusubiPinIntentOutboxErrorV1::Invalid);
        }
        Ok(Self {
            outbox,
            high_water_reader,
        })
    }

    /// Durably stage one exact signed wire after authenticating the predecessor inventory.
    ///
    /// An interrupted stage may be retried only when removing this exact retained operation
    /// reconstructs the current finalized predecessor. The result carries no signed wire and
    /// cannot authorize Queue submission. A qualified future owner must finalize the advance,
    /// verify an independently current network checkpoint, and recheck complete custody.
    ///
    /// # Errors
    /// Refuses absent or changed finality, a second pending intent, different signed wire,
    /// source substitution, revision overflow, or unsafe local custody.
    pub fn stage_signed_intent_checked(
        &mut self,
        operation_id: [u8; 32],
        source: &MusubiPublicationFinalizedArchiveRegistrationQueryV1,
        manifest_digest: ManifestDigest,
        transaction: &SignedTransaction,
    ) -> Result<MusubiPinOutboxCheckedStageV1, MusubiPinIntentOutboxErrorV1> {
        stage_signed_intent_checked_with(
            &mut self.outbox,
            operation_id,
            source,
            manifest_digest,
            transaction,
            |authority| self.high_water_reader.read_current(authority),
        )
    }

    /// Audit one original signed intent under a locally finalized complete inventory.
    ///
    /// Only metadata is returned. The exact signed wire remains in the process-exclusive
    /// outbox; this observation does not authorize Queue retry or claim pin finality.
    ///
    /// # Errors
    /// Refuses missing or changing high-water, changed inventory, absent operation, or stale
    /// source registration.
    pub fn recover_locally_finalized_operation(
        &self,
        operation_id: [u8; 32],
        source: &MusubiPublicationFinalizedArchiveRegistrationQueryV1,
    ) -> Result<MusubiPinOutboxCheckedRecoveryV1, MusubiPinIntentOutboxErrorV1> {
        recover_locally_finalized_operation_with(&self.outbox, operation_id, source, |authority| {
            self.high_water_reader.read_current(authority)
        })
    }
}

fn stage_signed_intent_checked_with(
    outbox: &mut DurableMusubiPinIntentOutboxV1,
    operation_id: [u8; 32],
    source: &MusubiPublicationFinalizedArchiveRegistrationQueryV1,
    manifest_digest: ManifestDigest,
    transaction: &SignedTransaction,
    mut read_current: impl FnMut(
        &AccountId,
    ) -> Result<
        Option<MusubiPinOutboxHighWaterV1>,
        MusubiPublicationPinOutboxHighWaterReadErrorV1,
    >,
) -> Result<MusubiPinOutboxCheckedStageV1, MusubiPinIntentOutboxErrorV1> {
    use MusubiPinIntentOutboxErrorV1::{Conflict, Invalid};
    let predecessor = read_bound_high_water(outbox, &mut read_current)?;
    predecessor.revision.checked_add(1).ok_or(Invalid)?;
    let before = outbox.inventory_digest()?;
    if before == predecessor.inventory_digest {
        outbox.stage_signed_intent(operation_id, source, manifest_digest, transaction)?;
    } else {
        // Exactly one already-durable, still-unfinalized intent may be resumed. Never add a
        // second intent while the complete local inventory is ahead of finalized State/Kura.
        let retained = outbox
            .recover_operation(operation_id)
            .map_err(|error| if error == Invalid { Conflict } else { error })?;
        if retained.source != *source
            || retained.manifest_digest != manifest_digest
            || retained.transaction.encode_wire_v1().map_err(|_| Invalid)?
                != transaction.encode_wire_v1().map_err(|_| Invalid)?
            || outbox.inventory_digest_excluding_operation(operation_id)?
                != predecessor.inventory_digest
        {
            return Err(Conflict);
        }
    }
    let after = outbox.inventory_digest()?;
    let observed = read_bound_high_water(outbox, &mut read_current)?;
    if observed == predecessor {
        if after == predecessor.inventory_digest {
            return Ok(MusubiPinOutboxCheckedStageV1::LocallyFinalized {
                revision: observed.revision,
                inventory_digest: after,
            });
        }
        let advance = AdvanceMusubiPinOutboxV1 {
            network_id: predecessor.network_id,
            pin_authority: predecessor.pin_authority,
            session_id: predecessor.session_id,
            expected_revision: predecessor.revision,
            expected_inventory_digest: predecessor.inventory_digest,
            inventory_digest: after,
        };
        advance.validate().map_err(|_| Invalid)?;
        return Ok(MusubiPinOutboxCheckedStageV1::NeedsFinality(advance));
    }
    if observed.revision == predecessor.revision.checked_add(1).ok_or(Invalid)?
        && observed.inventory_digest == after
    {
        return Ok(MusubiPinOutboxCheckedStageV1::LocallyFinalized {
            revision: observed.revision,
            inventory_digest: after,
        });
    }
    Err(Invalid)
}

fn recover_locally_finalized_operation_with(
    outbox: &DurableMusubiPinIntentOutboxV1,
    operation_id: [u8; 32],
    source: &MusubiPublicationFinalizedArchiveRegistrationQueryV1,
    mut read_current: impl FnMut(
        &AccountId,
    ) -> Result<
        Option<MusubiPinOutboxHighWaterV1>,
        MusubiPublicationPinOutboxHighWaterReadErrorV1,
    >,
) -> Result<MusubiPinOutboxCheckedRecoveryV1, MusubiPinIntentOutboxErrorV1> {
    use MusubiPinIntentOutboxErrorV1::Invalid;
    let first = read_bound_high_water(outbox, &mut read_current)?;
    outbox.verify_finalized_high_water(&first)?;
    let retained = outbox.recover_operation(operation_id)?;
    if retained.source != *source {
        return Err(Invalid);
    }
    let second = read_bound_high_water(outbox, &mut read_current)?;
    if second != first {
        return Err(Invalid);
    }
    outbox.verify_finalized_high_water(&second)?;
    Ok(MusubiPinOutboxCheckedRecoveryV1 {
        operation_id,
        manifest_digest: retained.manifest_digest,
        transaction_hash: *retained.transaction.hash().as_ref(),
        revision: second.revision,
        inventory_digest: second.inventory_digest,
    })
}

fn read_bound_high_water(
    outbox: &DurableMusubiPinIntentOutboxV1,
    read_current: &mut impl FnMut(
        &AccountId,
    ) -> Result<
        Option<MusubiPinOutboxHighWaterV1>,
        MusubiPublicationPinOutboxHighWaterReadErrorV1,
    >,
) -> Result<MusubiPinOutboxHighWaterV1, MusubiPinIntentOutboxErrorV1> {
    let (network_id, pin_authority) = outbox.owner_binding();
    let record = read_current(pin_authority)
        .map_err(|error| match error {
            MusubiPublicationPinOutboxHighWaterReadErrorV1::LocallyAhead => {
                MusubiPinIntentOutboxErrorV1::LocallyAhead
            }
            MusubiPublicationPinOutboxHighWaterReadErrorV1::Invalid => {
                MusubiPinIntentOutboxErrorV1::Invalid
            }
        })?
        .ok_or(MusubiPinIntentOutboxErrorV1::MissingFinalizedAnchor)?;
    record
        .validate()
        .map_err(|_| MusubiPinIntentOutboxErrorV1::Invalid)?;
    if &record.network_id != network_id
        || &record.pin_authority != pin_authority
        || record.session_id != outbox.session_id()
    {
        return Err(MusubiPinIntentOutboxErrorV1::Invalid);
    }
    Ok(record)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::musubi_publication_service::finality::tests::reader_fixture;
    use iroha_config::parameters::actual::MusubiPublicationPaidPinPolicy;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        account::AccountId,
        isi::{InstructionBox, sorafs::RegisterPinManifest},
        sorafs::pin_registry::StorageClass,
        transaction::{DEFAULT_TRANSACTION_TIME_TO_LIVE, FeePaymentIntent, TransactionBuilder},
    };
    use sorafs_manifest::{
        DagCodecId, ManifestBuilder, PinPolicy, ProfileId, StorageClass as ManifestStorageClass,
    };
    use std::{fs, os::unix::fs::PermissionsExt as _, time::Duration};

    fn fixture() -> (
        tempfile::TempDir,
        DurableMusubiPinIntentOutboxV1,
        MusubiPublicationFinalizedArchiveRegistrationQueryV1,
        ManifestDigest,
        SignedTransaction,
        MusubiPublicationPinOutboxHighWaterReaderV1,
    ) {
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
            transaction_authority: authority,
        };
        let temp = tempfile::tempdir().expect("private outbox root");
        fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700))
            .expect("private root mode");
        let root = temp.path().canonicalize().expect("canonical root");
        let limits = super::super::MusubiPinIntentOutboxLimitsV1 {
            max_records: 2,
            max_total_bytes: 16 * 1024 * 1024,
        };
        DurableMusubiPinIntentOutboxV1::initialize(
            &root,
            finalized.query.network_id,
            [0xa1; 32],
            policy.clone(),
            limits,
        )
        .expect("initialize outbox");
        let outbox = DurableMusubiPinIntentOutboxV1::open_inner(
            &root,
            finalized.query.network_id,
            policy,
            limits,
            finalized.reader,
        )
        .expect("open outbox");
        let high_water_reader = MusubiPublicationPinOutboxHighWaterReaderV1::new(
            finalized.query.network_id,
            finalized.state,
        )
        .expect("same network State");
        (
            temp,
            outbox,
            finalized.query,
            digest,
            transaction,
            high_water_reader,
        )
    }

    fn synthetic_high_water(
        outbox: &DurableMusubiPinIntentOutboxV1,
        revision: u64,
    ) -> MusubiPinOutboxHighWaterV1 {
        let (network_id, pin_authority) = outbox.owner_binding();
        MusubiPinOutboxHighWaterV1 {
            version: 1,
            network_id: *network_id,
            pin_authority: pin_authority.clone(),
            session_id: outbox.session_id(),
            revision,
            inventory_digest: outbox.inventory_digest().expect("complete inventory"),
            recorded_at_height: revision + 2,
            transaction_hash: [revision as u8; 32],
        }
    }

    #[test]
    fn checked_stage_resumes_exact_pending_predecessor_and_reports_local_finality() {
        let (_temp, mut outbox, source, digest, transaction, _reader) = fixture();
        let predecessor = synthetic_high_water(&outbox, 1);
        let first = stage_signed_intent_checked_with(
            &mut outbox,
            [0x61; 32],
            &source,
            digest,
            &transaction,
            |_| Ok(Some(predecessor.clone())),
        )
        .expect("durable stage with exact predecessor");
        let next_digest = outbox.inventory_digest().expect("staged inventory");
        let MusubiPinOutboxCheckedStageV1::NeedsFinality(advance) = first.clone() else {
            panic!("native advance required before pin dispatch");
        };
        assert_eq!(advance.expected_revision, 1);
        assert_eq!(
            advance.expected_inventory_digest,
            predecessor.inventory_digest
        );
        assert_eq!(advance.inventory_digest, next_digest);
        assert_eq!(advance.session_id, predecessor.session_id);
        assert_eq!(
            stage_signed_intent_checked_with(
                &mut outbox,
                [0x61; 32],
                &source,
                digest,
                &transaction,
                |_| Ok(Some(predecessor.clone())),
            ),
            Ok(first),
            "an interrupted durable stage resumes without a second file",
        );
        assert_eq!(
            stage_signed_intent_checked_with(
                &mut outbox,
                [0x62; 32],
                &source,
                digest,
                &transaction,
                |_| Ok(Some(predecessor.clone())),
            ),
            Err(MusubiPinIntentOutboxErrorV1::Conflict),
            "a second pending operation cannot overtake the predecessor",
        );
        let mut changed_source = source.clone();
        changed_source.expected_policy_revision += 1;
        assert_eq!(
            stage_signed_intent_checked_with(
                &mut outbox,
                [0x61; 32],
                &changed_source,
                digest,
                &transaction,
                |_| Ok(Some(predecessor.clone())),
            ),
            Err(MusubiPinIntentOutboxErrorV1::Conflict),
        );
        let finalized = synthetic_high_water(&outbox, 2);
        assert_eq!(
            stage_signed_intent_checked_with(
                &mut outbox,
                [0x61; 32],
                &source,
                digest,
                &transaction,
                |_| Ok(Some(finalized.clone())),
            ),
            Ok(MusubiPinOutboxCheckedStageV1::LocallyFinalized {
                revision: 2,
                inventory_digest: next_digest,
            }),
        );
        let recovered =
            recover_locally_finalized_operation_with(&outbox, [0x61; 32], &source, |_| {
                Ok(Some(finalized.clone()))
            })
            .expect("exact retained intent covered by local high-water");
        assert_eq!(recovered.operation_id, [0x61; 32]);
        assert_eq!(recovered.manifest_digest, digest);
        assert_eq!(recovered.transaction_hash, *transaction.hash().as_ref());
        assert_eq!(recovered.revision, 2);
    }

    #[test]
    fn checked_stage_and_recovery_close_on_missing_or_changed_finality() {
        use MusubiPublicationPinOutboxHighWaterReadErrorV1::{Invalid, LocallyAhead};
        let (_temp, mut outbox, source, digest, transaction, reader) = fixture();
        let predecessor = synthetic_high_water(&outbox, 1);
        let mut coordinator = MusubiPublicationPinOutboxCheckedCoordinatorV1::new(outbox, reader)
            .expect("same-network coordinator");
        assert_eq!(
            coordinator.stage_signed_intent_checked([0x61; 32], &source, digest, &transaction),
            Err(MusubiPinIntentOutboxErrorV1::MissingFinalizedAnchor),
            "the production reader sees no finalized advance in this fixture",
        );
        assert_eq!(
            coordinator.recover_locally_finalized_operation([0x61; 32], &source),
            Err(MusubiPinIntentOutboxErrorV1::MissingFinalizedAnchor),
        );
        outbox = coordinator.outbox;
        assert_eq!(
            stage_signed_intent_checked_with(
                &mut outbox,
                [0x61; 32],
                &source,
                digest,
                &transaction,
                |_| Ok(None),
            ),
            Err(MusubiPinIntentOutboxErrorV1::MissingFinalizedAnchor),
        );
        assert_eq!(
            stage_signed_intent_checked_with(
                &mut outbox,
                [0x61; 32],
                &source,
                digest,
                &transaction,
                |_| Err(LocallyAhead),
            ),
            Err(MusubiPinIntentOutboxErrorV1::LocallyAhead),
        );
        let mut switched_session = predecessor.clone();
        switched_session.session_id = [0xa2; 32];
        assert_eq!(
            stage_signed_intent_checked_with(
                &mut outbox,
                [0x61; 32],
                &source,
                digest,
                &transaction,
                |_| Ok(Some(switched_session.clone())),
            ),
            Err(MusubiPinIntentOutboxErrorV1::Invalid),
        );
        assert_eq!(outbox.retained_digests().expect("still empty"), []);
        let mut reads = 0;
        assert_eq!(
            stage_signed_intent_checked_with(
                &mut outbox,
                [0x61; 32],
                &source,
                digest,
                &transaction,
                |_| {
                    reads += 1;
                    if reads == 1 {
                        Ok(Some(predecessor.clone()))
                    } else {
                        Err(Invalid)
                    }
                },
            ),
            Err(MusubiPinIntentOutboxErrorV1::Invalid),
            "the durable signed wire remains for explicit recovery after a read race",
        );
        assert_eq!(
            outbox.retained_digests().expect("retained after stage"),
            [digest]
        );
        assert_eq!(
            recover_locally_finalized_operation_with(&outbox, [0x61; 32], &source, |_| {
                Ok(Some(predecessor.clone()))
            }),
            Err(MusubiPinIntentOutboxErrorV1::Invalid),
            "a pending inventory cannot be recovered as finalized",
        );
        let finalized = synthetic_high_water(&outbox, 2);
        let mut reads = 0;
        assert_eq!(
            recover_locally_finalized_operation_with(&outbox, [0x61; 32], &source, |_| {
                reads += 1;
                if reads == 1 {
                    Ok(Some(finalized.clone()))
                } else {
                    Ok(Some(predecessor.clone()))
                }
            }),
            Err(MusubiPinIntentOutboxErrorV1::Invalid),
            "the State/Kura high-water must remain stable during recovery",
        );
    }
}
