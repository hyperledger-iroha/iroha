#[cfg(test)]
mod tests {
    use super::{
        BlockSignaturePolicy, BodyValidationError, BodyValidationRejectionIdentity,
        QuarantinedValidationOutcome, RecoveredTerminalValidateOutcomeCatalogError,
        RevalidatedRejectedBody, STORE_MAGIC, STORE_VERSION, V2BodyStore, V2BodyStoreError,
        VALIDATED_MAGIC, VALIDATION_OUTCOME_MARKER_VERSION, ValidatedBodyReceipt,
        ValidationOutcomeMarker, ValidationOutcomeMarkerKind, write_validation_outcome_marker,
    };
    use crate::sumeragi::{
        v2::RecoveredValidationAuthority, v2_apply::VerifiedRecoveredFinalitySubject,
        v2_chunks::encode_payload,
    };
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, SignatureOf};
    use iroha_data_model::{
        NetworkId,
        block::{
            BlockHeader, BlockSignature, CertifiedMergeLedgerReference, SignedBlock,
            consensus_v2 as wire, decode_framed_signed_block,
        },
        merge::MergeQuorumCertificate,
        peer::PeerId,
    };
    use std::{cell::Cell, fs, num::NonZeroU64, path::Path};
    use tempfile::TempDir;
    fn test_network_id() -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([0x94; Hash::LENGTH]),
        ))
    }
    #[derive(Debug)]
    enum FixtureValidationError {
        MissingMergeSidecar(CertifiedMergeLedgerReference),
        Invalid(&'static str),
    }
    impl std::fmt::Display for FixtureValidationError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::MissingMergeSidecar(reference) => {
                    write!(formatter, "missing merge sidecar {}", reference.entry_hash)
                }
                Self::Invalid(reason) => formatter.write_str(reason),
            }
        }
    }
    impl BodyValidationError for FixtureValidationError {
        fn missing_certified_merge_sidecar(&self) -> Option<&CertifiedMergeLedgerReference> {
            match self {
                Self::MissingMergeSidecar(reference) => Some(reference),
                Self::Invalid(_) => None,
            }
        }
    }
    fn context_and_keys() -> (wire::HeightContext, Vec<KeyPair>) {
        let mut keys = (1_u8..=4)
            .map(|seed| {
                KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                    .expect("deterministic key")
            })
            .collect::<Vec<_>>();
        keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
        let roster = keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let context = wire::HeightContext {
            network_id: test_network_id(),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("fixture quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"test nexus amx context"),
            execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: wire::MAX_DA_CHUNK_SIZE_BYTES,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: u64::from(wire::MAX_DA_CHUNK_SIZE_BYTES),
                max_chunk_count: 2,
            },
            leader_seed: [0x42; 32],
        };
        (context, keys)
    }
    fn missing_merge_reference(
        receipt: &super::DurableBodyReceipt,
    ) -> CertifiedMergeLedgerReference {
        let parent_hash =
            HashOf::from_untyped_unchecked(Hash::new(b"body-store validation parent"));
        CertifiedMergeLedgerReference {
            version: 1,
            entry_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"body-store missing merge sidecar",
            )),
            encoded_len: 512,
            epoch_id: 7,
            execution_batch_hash: None,
            entrypoint_count: None,
            entrypoint_merkle_root: None,
            result_merkle_root: None,
            base_state_height: None,
            base_state_hash: None,
            merge_qc: MergeQuorumCertificate::new(
                receipt.round().view,
                7,
                receipt.round().height,
                parent_hash,
                test_network_id(),
                1,
                HashOf::new(&Vec::<PeerId>::new()),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Hash::new(b"body-store validation certificate"),
            ),
        }
    }
    fn body_and_manifest(
        context: &wire::HeightContext,
        keys: &[KeyPair],
        signing_key_index: Option<usize>,
    ) -> (Vec<u8>, wire::PayloadManifest) {
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        };
        let leader = context.leader(round.view);
        let leader_index = usize::try_from(leader).expect("leader index");
        body_and_manifest_with_signature(
            context,
            &keys[signing_key_index.unwrap_or(leader_index)],
            u64::from(leader),
        )
    }
    fn body_and_manifest_with_signature(
        context: &wire::HeightContext,
        signing_key: &KeyPair,
        signature_index: u64,
    ) -> (Vec<u8>, wire::PayloadManifest) {
        body_and_manifest_with_signature_and_views(context, signing_key, signature_index, 0, 0)
    }
    fn body_and_manifest_with_signature_and_views(
        context: &wire::HeightContext,
        signing_key: &KeyPair,
        signature_index: u64,
        proposal_view: u64,
        header_view: u64,
    ) -> (Vec<u8>, wire::PayloadManifest) {
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: proposal_view,
        };
        let header = BlockHeader::new(
            NonZeroU64::new(round.height).expect("non-zero height"),
            None,
            None,
            None,
            1_000,
            header_view,
        );
        let signature = SignatureOf::try_from_hash(signing_key.private_key(), header.hash())
            .expect("sign block header");
        let block = SignedBlock::presigned(
            BlockSignature::new(signature_index, signature),
            header,
            Vec::new(),
        );
        let canonical_wire = block.encode_wire().expect("canonical block wire");
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: block.hash(),
            payload_hash: Hash::new(&canonical_wire),
        };
        let manifest = encode_payload(context, round, subject, &canonical_wire)
            .expect("encode canonical fixture payload")
            .manifest()
            .clone();
        (canonical_wire, manifest)
    }
    #[test]
    fn body_store_instance_identity_distinguishes_a_same_path_reopen() {
        let directory = TempDir::new().expect("temporary identity body store");
        let (context, _) = context_and_keys();
        let store = V2BodyStore::open(directory.path(), context.clone())
            .expect("open first body-store instance");
        let first = store.instance_identity();
        assert!(first.same_instance(&store.instance_identity()));
        let reopened = V2BodyStore::open(directory.path(), context)
            .expect("reopen the same body-store path independently");
        assert!(
            !first.same_instance(&reopened.instance_identity()),
            "path and context equality cannot substitute for move-only instance ownership"
        );
    }
    #[test]
    fn emergency_fast_body_store_skips_inventory_and_rejects_writes() {
        let root = TempDir::new().expect("temporary emergency body store");
        let (context, keys) = context_and_keys();
        let expected_directory = root
            .path()
            .join(hex::encode(context.id().0.as_ref()));
        let mut store = V2BodyStore::open_emergency_fast_read_only(
            root.path(),
            context.clone(),
            BlockSignaturePolicy::RotatingLeader,
        )
        .expect("open inert emergency body store");
        assert!(
            !expected_directory.exists(),
            "emergency open must not create or inventory the context directory"
        );

        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let final_path = store.path_for(manifest.round, manifest.subject);
        std::fs::create_dir_all(final_path.parent().unwrap()).expect("create ignored directory");
        let sentinel = b"untouched Strict-recovery body";
        std::fs::write(&final_path, sentinel).expect("write ignored final body");
        assert!(matches!(
            store.store(manifest, body),
            Err(V2BodyStoreError::EmergencyFastReadOnly)
        ));
        assert_eq!(
            std::fs::read(final_path).expect("reread ignored final body"),
            sentinel
        );
    }
    fn store_with_promoted_terminal_outcomes(
        directory: &Path,
        context: &wire::HeightContext,
        keys: &[KeyPair],
    ) -> V2BodyStore {
        let (validated_body, validated_manifest) = body_and_manifest(context, keys, None);
        let mut store =
            V2BodyStore::open(directory, context.clone()).expect("open terminal outcome store");
        let validated_receipt = store
            .store(validated_manifest, validated_body)
            .expect("persist terminal success body");
        let commitment =
            ValidatedBodyReceipt::for_test(validated_receipt.clone()).execution_commitment();
        let _validated = store
            .persist_validated_receipt(&validated_receipt, commitment)
            .expect("promote terminal success");
        let rejected_view = 1;
        let rejected_leader = context.leader(rejected_view);
        let rejected_leader_index =
            usize::try_from(rejected_leader).expect("rejected leader index");
        let (rejected_body, rejected_manifest) = body_and_manifest_with_signature_and_views(
            context,
            &keys[rejected_leader_index],
            u64::from(rejected_leader),
            rejected_view,
            rejected_view,
        );
        let rejected_receipt = store
            .store(rejected_manifest, rejected_body)
            .expect("persist terminal rejection body");
        let _rejected = store
            .persist_rejected_outcome(
                &rejected_receipt,
                BodyValidationRejectionIdentity::Rejected.canonical_code(),
                "volatile terminal rejection diagnostic".to_owned(),
            )
            .expect("promote terminal rejection");
        store
    }
    fn durable_files_snapshot(root: &Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
        fn visit(root: &Path, directory: &Path, files: &mut Vec<(std::path::PathBuf, Vec<u8>)>) {
            let mut entries = fs::read_dir(directory)
                .expect("read body-store snapshot directory")
                .map(|entry| entry.expect("read body-store snapshot entry").path())
                .collect::<Vec<_>>();
            entries.sort();
            for path in entries {
                if path.is_dir() {
                    visit(root, &path, files);
                } else {
                    files.push((
                        path.strip_prefix(root)
                            .expect("snapshot entry belongs to root")
                            .to_path_buf(),
                        fs::read(&path).expect("read body-store snapshot file"),
                    ));
                }
            }
        }
        let mut files = Vec::new();
        visit(root, root, &mut files);
        files
    }
    #[test]
    fn durable_body_roundtrips_and_reopens_idempotently() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        assert!(store.matches_context(&context));
        let mut foreign_context = context.clone();
        foreign_context.height = foreign_context.height.saturating_add(1);
        assert!(!store.matches_context(&foreign_context));
        let receipt = store
            .store(manifest.clone(), body.clone())
            .expect("store exact body");
        let execution_commitment =
            ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
        let validated = store
            .validate(&receipt, |block| {
                (block.hash() == receipt.subject().block_hash)
                    .then_some(execution_commitment)
                    .ok_or("wrong block")
            })
            .expect("validate exact durable body");
        assert_eq!(validated.durable(), &receipt);
        assert_eq!(validated.execution_commitment(), execution_commitment);
        assert_eq!(
            store
                .load(&receipt)
                .expect("load exact body")
                .encode_wire()
                .unwrap(),
            body
        );
        assert_eq!(
            store
                .store(manifest.clone(), body)
                .expect("idempotent store returns same receipt"),
            receipt
        );
        drop(store);
        let mut reopened = V2BodyStore::open(directory.path(), context).expect("replay store");
        assert_eq!(
            reopened.receipt(manifest.round, manifest.subject),
            Some(receipt.clone())
        );
        assert_eq!(
            reopened
                .recovered(manifest.round, manifest.subject)
                .expect("reload recovered manifest"),
            Some((manifest, receipt.clone()))
        );
        assert_eq!(
            reopened
                .load(&receipt)
                .expect("receipt remains valid after replay")
                .hash(),
            receipt.subject().block_hash
        );
        assert!(reopened.validated_recovery_catalog().is_empty());
        assert!(matches!(
            reopened.ensure_recovered_markers_revalidated(),
            Err(V2BodyStoreError::UnrevalidatedValidationMarkers)
        ));
        let callback_ran = Cell::new(false);
        let _validated = reopened
            .validate(&receipt, |_| {
                callback_ran.set(true);
                Ok::<wire::ExecutionCommitment, &str>(execution_commitment)
            })
            .expect("durable validation marker resumes after semantic replay");
        assert!(callback_ran.get());
        reopened
            .ensure_recovered_markers_revalidated()
            .expect("recovered marker crossed semantic replay");
        assert_eq!(
            reopened
                .validated_recovery_catalog()
                .get(&(receipt.round(), receipt.subject()))
                .map(ValidatedBodyReceipt::execution_commitment),
            Some(execution_commitment),
        );
    }
    #[test]
    fn recovered_marker_cannot_restore_vote_authority_without_semantic_replay() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let receipt = store.store(manifest, body).expect("store exact body");
        let expected = ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
        let _validated = store
            .persist_validated_receipt(&receipt, expected)
            .expect("persist legitimate validation marker");
        let forged = wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"forged parent root"),
            Hash::new(b"forged post root"),
            Hash::new(b"forged ordinary writes"),
            1,
            Hash::new(b"forged executed block"),
        );
        assert_ne!(expected, forged);
        let marker = ValidationOutcomeMarker {
            version: VALIDATION_OUTCOME_MARKER_VERSION,
            context_id: receipt.context_id,
            round: receipt.round,
            subject: receipt.subject,
            manifest_hash: receipt.manifest_hash,
            body_frame_hash: receipt.frame_hash,
            outcome: ValidationOutcomeMarkerKind::Validated(forged),
        };
        write_validation_outcome_marker(
            &store.validated_path_for(receipt.round(), receipt.subject()),
            &marker,
        )
        .expect("substitute a checksum-valid local marker");
        drop(store);
        let mut reopened = V2BodyStore::open(directory.path(), context)
            .expect("structurally read substituted marker");
        assert!(reopened.validated_recovery_catalog().is_empty());
        assert!(matches!(
            reopened.revalidate_recovered_markers(|_| {
                Ok::<wire::ExecutionCommitment, String>(expected)
            }),
            Err(V2BodyStoreError::RecoveredValidationCommitmentMismatch)
        ));
        assert!(reopened.validated_recovery_catalog().is_empty());
        assert!(matches!(
            reopened.ensure_recovered_markers_revalidated(),
            Err(V2BodyStoreError::UnrevalidatedValidationMarkers)
        ));
    }
    #[test]
    fn recovered_marker_missing_sidecar_retires_authority_without_losing_body() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let receipt = store
            .store(manifest.clone(), body)
            .expect("store exact body");
        let execution_commitment =
            ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
        let _validated = store
            .persist_validated_receipt(&receipt, execution_commitment)
            .expect("persist validation marker");
        drop(store);
        let mut reopened = V2BodyStore::open(directory.path(), context).expect("reopen store");
        assert!(matches!(
            reopened.revalidate_recovered_markers(|_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::Invalid(
                    "terminal recovered validation failure",
                ))
            }),
            Err(V2BodyStoreError::RecoveredValidationOutcomeMismatch)
        ));
        assert!(matches!(
            reopened.ensure_recovered_markers_revalidated(),
            Err(V2BodyStoreError::UnrevalidatedValidationMarkers)
        ));
        let reference = missing_merge_reference(&receipt);
        reopened
            .revalidate_recovered_markers(|_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::MissingMergeSidecar(
                    reference.clone(),
                ))
            })
            .expect("missing sidecar retires marker authority without failing startup");
        reopened
            .ensure_recovered_markers_revalidated()
            .expect("no untrusted marker authority survives startup");
        assert!(reopened.validated_recovery_catalog().is_empty());
        assert_eq!(reopened.retired_revalidation.len(), 1);
        assert_eq!(
            reopened
                .recovered(manifest.round, manifest.subject)
                .expect("inspect retained exact body"),
            Some((manifest, receipt.clone()))
        );
        let deferred = reopened
            .execute_durable_validation(receipt.clone(), receipt.manifest_hash(), |_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::MissingMergeSidecar(
                    reference.clone(),
                ))
            })
            .expect("ordinary validation defers on the exact missing sidecar");
        assert_eq!(deferred.missing_merge_sidecar(), Some(&reference));
        assert!(reopened.validated_recovery_catalog().is_empty());
        let validated = reopened
            .execute_durable_validation(receipt.clone(), receipt.manifest_hash(), |_| {
                Ok::<_, FixtureValidationError>(execution_commitment)
            })
            .expect("ordinary bounded retry validates after sidecar recovery");
        assert_eq!(
            validated
                .validated_receipt()
                .map(ValidatedBodyReceipt::execution_commitment),
            Some(execution_commitment)
        );
        assert!(reopened.retired_revalidation.is_empty());
    }
    #[test]
    fn wal_frontier_bounds_many_view_restart_validation_work() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let mut receipts = Vec::new();
        for view in 0_u64..32 {
            let leader = context.leader(view);
            let leader_index = usize::try_from(leader).expect("leader index");
            let (body, manifest) = body_and_manifest_with_signature_and_views(
                &context,
                &keys[leader_index],
                u64::from(leader),
                view,
                view,
            );
            let receipt = store.store(manifest, body).expect("store view candidate");
            let commitment = ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
            let _validated = store
                .persist_validated_receipt(&receipt, commitment)
                .expect("persist view validation marker");
            receipts.push((receipt, commitment));
        }
        drop(store);
        let mut reopened =
            V2BodyStore::open(directory.path(), context.clone()).expect("reopen view catalog");
        let selected = [receipts[7].0.clone(), receipts[31].0.clone()];
        let authority = RecoveredValidationAuthority::for_test(
            &context,
            selected
                .iter()
                .map(|receipt| (receipt.round(), receipt.subject())),
        );
        assert_eq!(authority.len(), 2);
        reopened
            .retain_recovered_markers_for_authority(authority)
            .expect("WAL frontier belongs to the exact body context");
        let callback_count = Cell::new(0_usize);
        reopened
            .revalidate_recovered_markers(|block| {
                callback_count.set(callback_count.get().saturating_add(1));
                receipts
                    .iter()
                    .find_map(|(receipt, commitment)| {
                        (receipt.subject().block_hash == block.hash()).then_some(*commitment)
                    })
                    .ok_or_else(|| "replayed an unauthorized body".to_owned())
            })
            .expect("revalidate only the authenticated WAL frontier");
        assert_eq!(callback_count.get(), 2);
        assert_eq!(reopened.validated_recovery_catalog().len(), 2);
        assert_eq!(
            reopened
                .recovery_catalog()
                .expect("retained body catalog")
                .len(),
            32,
            "superseded markers lose authority without deleting DA body evidence"
        );
    }
    #[test]
    fn wal_frontier_capability_cannot_cross_height_contexts() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let receipt = store.store(manifest, body).expect("store exact body");
        let commitment = ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
        let _validated = store
            .persist_validated_receipt(&receipt, commitment)
            .expect("persist validation marker");
        drop(store);
        let mut reopened =
            V2BodyStore::open(directory.path(), context.clone()).expect("reopen store");
        let pending_before = reopened.pending_revalidation.clone();
        let validated_before = reopened.validated.clone();
        let mut foreign_context = context;
        foreign_context.leader_seed[0] ^= 0x40;
        let foreign_round = wire::ConsensusRound {
            context_id: foreign_context.id(),
            height: foreign_context.height,
            view: receipt.round().view,
        };
        let authority = RecoveredValidationAuthority::for_test(
            &foreign_context,
            [(foreign_round, receipt.subject())],
        );
        assert!(matches!(
            reopened.retain_recovered_markers_for_authority(authority),
            Err(V2BodyStoreError::RecoveredValidationAuthorityContextMismatch)
        ));
        assert_eq!(reopened.pending_revalidation, pending_before);
        assert_eq!(reopened.validated, validated_before);
    }
    #[test]
    fn verified_decision_retires_losing_restart_marker_authority() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let mut receipts = Vec::new();
        for view in [0_u64, 1] {
            let leader = context.leader(view);
            let leader_index = usize::try_from(leader).expect("leader index");
            let (body, manifest) = body_and_manifest_with_signature_and_views(
                &context,
                &keys[leader_index],
                u64::from(leader),
                view,
                view,
            );
            let receipt = store.store(manifest, body).expect("store candidate body");
            let commitment = ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
            let _validated = store
                .persist_validated_receipt(&receipt, commitment)
                .expect("persist candidate validation marker");
            receipts.push((receipt, commitment));
        }
        assert_ne!(receipts[0].0.subject(), receipts[1].0.subject());
        drop(store);
        let mut reopened =
            V2BodyStore::open(directory.path(), context.clone()).expect("reopen store");
        reopened
            .retain_recovered_markers_for_subject(VerifiedRecoveredFinalitySubject::for_test(
                &context,
                receipts[0].0.subject(),
            ))
            .expect("verified decision belongs to the recovered context");
        reopened
            .revalidate_recovered_markers(|_| {
                Ok::<wire::ExecutionCommitment, String>(receipts[0].1)
            })
            .expect("revalidate only the verified decision");
        reopened
            .ensure_recovered_markers_revalidated()
            .expect("losing marker authority was retired");
        let catalog = reopened.validated_recovery_catalog();
        assert!(catalog.contains_key(&(receipts[0].0.round(), receipts[0].0.subject())));
        assert!(!catalog.contains_key(&(receipts[1].0.round(), receipts[1].0.subject())));
    }
    #[test]
    fn verified_decision_capability_cannot_cross_height_contexts() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let receipt = store.store(manifest, body).expect("store candidate body");
        let commitment = ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
        let _validated = store
            .persist_validated_receipt(&receipt, commitment)
            .expect("persist candidate validation marker");
        drop(store);
        let mut reopened =
            V2BodyStore::open(directory.path(), context.clone()).expect("reopen store");
        let pending_before = reopened.pending_revalidation.clone();
        let validated_before = reopened.validated.clone();
        let mut foreign_context = context.clone();
        foreign_context.leader_seed[0] ^= 0x80;
        assert_ne!(foreign_context.id(), context.id());
        let error = reopened
            .retain_recovered_markers_for_subject(VerifiedRecoveredFinalitySubject::for_test(
                &foreign_context,
                receipt.subject(),
            ))
            .expect_err("foreign finality capability must fail closed");
        assert!(matches!(
            error,
            V2BodyStoreError::RecoveredFinalityContextMismatch
        ));
        assert_eq!(reopened.pending_revalidation, pending_before);
        assert_eq!(reopened.validated, validated_before);
    }
    #[test]
    fn verified_decision_retires_already_promoted_losing_marker_authority() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let mut receipts = Vec::new();
        for view in [0_u64, 1] {
            let leader = context.leader(view);
            let leader_index = usize::try_from(leader).expect("leader index");
            let (body, manifest) = body_and_manifest_with_signature_and_views(
                &context,
                &keys[leader_index],
                u64::from(leader),
                view,
                view,
            );
            let receipt = store.store(manifest, body).expect("store candidate body");
            let commitment = ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
            let _validated = store
                .persist_validated_receipt(&receipt, commitment)
                .expect("persist candidate validation marker");
            receipts.push((receipt, commitment));
        }
        assert_ne!(receipts[0].0.subject(), receipts[1].0.subject());
        drop(store);
        let mut reopened =
            V2BodyStore::open(directory.path(), context.clone()).expect("reopen store");
        let _validated = reopened
            .validate(&receipts[1].0, |_| {
                Ok::<wire::ExecutionCommitment, &str>(receipts[1].1)
            })
            .expect("promote the losing recovered marker before finality filtering");
        assert!(
            reopened
                .validated_recovery_catalog()
                .contains_key(&(receipts[1].0.round(), receipts[1].0.subject()))
        );
        reopened
            .retain_recovered_markers_for_subject(VerifiedRecoveredFinalitySubject::for_test(
                &context,
                receipts[0].0.subject(),
            ))
            .expect("verified decision belongs to the recovered context");
        assert!(reopened.validated_recovery_catalog().is_empty());
        reopened
            .revalidate_recovered_markers(|_| {
                Ok::<wire::ExecutionCommitment, String>(receipts[0].1)
            })
            .expect("revalidate only the verified decision");
        reopened
            .ensure_recovered_markers_revalidated()
            .expect("all losing marker authority was retired");
        let catalog = reopened.validated_recovery_catalog();
        assert!(catalog.contains_key(&(receipts[0].0.round(), receipts[0].0.subject())));
        assert!(!catalog.contains_key(&(receipts[1].0.round(), receipts[1].0.subject())));
    }
    #[test]
    fn non_v1_body_and_validation_frames_are_rejected() {
        const UNSUPPORTED_VERSION: u16 = 2;
        let body_directory = TempDir::new().expect("temporary body directory");
        let (body_context, body_keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&body_context, &body_keys, None);
        let mut body_store =
            V2BodyStore::open(body_directory.path(), body_context.clone()).expect("open store");
        let body_receipt = body_store.store(manifest, body).expect("store body");
        let body_path = body_store.path_for(body_receipt.round(), body_receipt.subject());
        drop(body_store);
        let mut body_frame = fs::read(&body_path).expect("read body frame");
        assert_eq!(
            u16::from_le_bytes(
                body_frame[STORE_MAGIC.len()..STORE_MAGIC.len() + size_of::<u16>()]
                    .try_into()
                    .expect("body frame version has fixed width"),
            ),
            STORE_VERSION,
        );
        body_frame[STORE_MAGIC.len()..STORE_MAGIC.len() + size_of::<u16>()]
            .copy_from_slice(&UNSUPPORTED_VERSION.to_le_bytes());
        fs::write(&body_path, body_frame).expect("write unsupported body frame");
        assert!(matches!(
            V2BodyStore::open(body_directory.path(), body_context),
            Err(V2BodyStoreError::UnsupportedVersion(UNSUPPORTED_VERSION))
        ));
        let marker_directory = TempDir::new().expect("temporary marker directory");
        let (marker_context, marker_keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&marker_context, &marker_keys, None);
        let mut marker_store = V2BodyStore::open(marker_directory.path(), marker_context.clone())
            .expect("open marker store");
        let marker_receipt = marker_store
            .store(manifest, body)
            .expect("store marker body");
        let commitment =
            ValidatedBodyReceipt::for_test(marker_receipt.clone()).execution_commitment();
        let _validated_receipt = marker_store
            .validate(&marker_receipt, |_| Ok::<_, &'static str>(commitment))
            .expect("persist validation marker");
        let marker_path =
            marker_store.validated_path_for(marker_receipt.round(), marker_receipt.subject());
        drop(marker_store);
        let mut marker_frame = fs::read(&marker_path).expect("read validation frame");
        assert_eq!(
            u16::from_le_bytes(
                marker_frame[VALIDATED_MAGIC.len()..VALIDATED_MAGIC.len() + size_of::<u16>()]
                    .try_into()
                    .expect("validation frame version has fixed width"),
            ),
            VALIDATION_OUTCOME_MARKER_VERSION,
        );
        marker_frame[VALIDATED_MAGIC.len()..VALIDATED_MAGIC.len() + size_of::<u16>()]
            .copy_from_slice(&UNSUPPORTED_VERSION.to_le_bytes());
        fs::write(&marker_path, marker_frame).expect("write unsupported validation frame");
        assert!(matches!(
            V2BodyStore::open(marker_directory.path(), marker_context),
            Err(V2BodyStoreError::UnsupportedVersion(UNSUPPORTED_VERSION))
        ));
    }
    #[test]
    fn rotating_leader_locked_body_reproposal_is_stored_and_revalidated_per_round() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, origin_manifest) = body_and_manifest(&context, &keys, None);
        let mut store =
            V2BodyStore::open(directory.path(), context.clone()).expect("open body store");
        let origin_receipt = store
            .store(origin_manifest.clone(), body.clone())
            .expect("store the origin-view body");
        let execution_commitment =
            ValidatedBodyReceipt::for_test(origin_receipt.clone()).execution_commitment();
        let first_callback_ran = Cell::new(false);
        let origin_validation = store
            .execute_durable_validation(
                origin_receipt.clone(),
                origin_receipt.manifest_hash(),
                |_| {
                    first_callback_ran.set(true);
                    Ok::<_, FixtureValidationError>(execution_commitment)
                },
            )
            .expect("validate the exact body once");
        assert!(first_callback_ran.get());
        assert_eq!(
            origin_validation
                .validated_receipt()
                .map(ValidatedBodyReceipt::execution_commitment),
            Some(execution_commitment)
        );
        drop(store);
        let mut store = V2BodyStore::open(directory.path(), context.clone())
            .expect("recover the exact origin-view validation marker");
        store
            .revalidate_recovered_markers(|_| {
                Ok::<wire::ExecutionCommitment, String>(execution_commitment)
            })
            .expect("semantically replay the recovered origin marker");
        let later_round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 7,
        };
        let later_manifest = encode_payload(&context, later_round, origin_manifest.subject, &body)
            .expect("encode the exact body for its later view")
            .manifest()
            .clone();
        let later_receipt = store
            .store(later_manifest, body)
            .expect("the original leader signature authenticates an unchanged reproposal body");
        let callback_ran = Cell::new(false);
        let later_validation = store
            .execute_durable_validation(
                later_receipt.clone(),
                later_receipt.manifest_hash(),
                |_| {
                    callback_ran.set(true);
                    Ok::<_, FixtureValidationError>(execution_commitment)
                },
            )
            .expect("revalidate the unchanged body under its new proposal round");
        assert!(
            callback_ran.get(),
            "validation markers never promote across rounds"
        );
        assert_eq!(
            later_validation
                .validated_receipt()
                .map(ValidatedBodyReceipt::execution_commitment),
            Some(execution_commitment)
        );
        assert!(
            store
                .validated_path_for(later_round, origin_manifest.subject)
                .exists()
        );
        assert_eq!(
            store
                .validated_recovery_catalog()
                .get(&(origin_manifest.round, origin_manifest.subject))
                .map(ValidatedBodyReceipt::durable),
            Some(&origin_receipt)
        );
    }
    #[test]
    fn locked_body_reproposal_cannot_change_rejection_into_success() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, origin_manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let origin_receipt = store
            .store(origin_manifest.clone(), body.clone())
            .expect("store origin body");
        let _rejected = store
            .execute_durable_validation(
                origin_receipt.clone(),
                origin_receipt.manifest_hash(),
                |_| {
                    Err::<wire::ExecutionCommitment, _>(FixtureValidationError::Invalid(
                        "origin body is invalid",
                    ))
                },
            )
            .expect("persist origin rejection");
        let later_round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 7,
        };
        let later_manifest = encode_payload(&context, later_round, origin_manifest.subject, &body)
            .expect("encode unchanged body for later proposal round")
            .manifest()
            .clone();
        let later_receipt = store
            .store(later_manifest, body)
            .expect("store unchanged later-round body");
        let later_manifest_hash = later_receipt.manifest_hash();
        let success = ValidatedBodyReceipt::for_test(later_receipt.clone()).execution_commitment();
        assert!(matches!(
            store.execute_durable_validation(
                later_receipt.clone(),
                later_receipt.manifest_hash(),
                |_| Ok::<_, FixtureValidationError>(success),
            ),
            Err(V2BodyStoreError::ConflictingValidationOutcome)
        ));
        assert!(
            !store
                .validated_path_for(later_round, origin_manifest.subject)
                .exists(),
            "a conflicting outcome must not become durable"
        );
        let later_rejection = store
            .execute_durable_validation(later_receipt, later_manifest_hash, |_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::Invalid(
                    "later round reproduces rejection",
                ))
            })
            .expect("same closed rejection is consistent across proposal rounds");
        assert_eq!(
            later_rejection.rejection_identity(),
            Some(&BodyValidationRejectionIdentity::Rejected)
        );
    }
    #[test]
    fn genesis_cross_view_validation_is_reexecuted_and_conflicts_fail_closed() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, _keys) = context_and_keys();
        let genesis = KeyPair::try_from_seed(vec![0xC4; 32], Algorithm::Ed25519)
            .expect("deterministic genesis key");
        let (body, origin_manifest) =
            body_and_manifest_with_signature_and_views(&context, &genesis, 0, 0, 0);
        let mut store = V2BodyStore::open_with_policy(
            directory.path(),
            context.clone(),
            BlockSignaturePolicy::GenesisAuthority(genesis.public_key().clone()),
        )
        .expect("open genesis body store");
        let origin_receipt = store
            .store(origin_manifest.clone(), body.clone())
            .expect("store the origin-view body");
        let origin_commitment =
            ValidatedBodyReceipt::for_test(origin_receipt.clone()).execution_commitment();
        let _ = store
            .persist_validated_receipt(&origin_receipt, origin_commitment)
            .expect("persist the origin-view validation witness");
        let later_round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 7,
        };
        let later_manifest = encode_payload(&context, later_round, origin_manifest.subject, &body)
            .expect("encode the exact body for its later view")
            .manifest()
            .clone();
        let later_receipt = store
            .store(later_manifest, body)
            .expect("durably bind the exact body to the later round");
        let conflicting_commitment =
            ValidatedBodyReceipt::for_test(later_receipt.clone()).execution_commitment();
        assert_ne!(origin_commitment, conflicting_commitment);
        let callback_ran = Cell::new(false);
        let error = store
            .execute_durable_validation(
                later_receipt.clone(),
                later_receipt.manifest_hash(),
                |_| {
                    callback_ran.set(true);
                    Ok::<_, FixtureValidationError>(conflicting_commitment)
                },
            )
            .expect_err("a prior-view marker must not bypass exact-round validation");
        assert!(
            callback_ran.get(),
            "the later proposal round must be revalidated"
        );
        assert!(matches!(
            error,
            V2BodyStoreError::ConflictingValidationCommitment
        ));
        let marker_path = store.validated_path_for(later_round, origin_manifest.subject);
        assert!(!marker_path.exists());
        let conflicting_marker = ValidationOutcomeMarker {
            version: VALIDATION_OUTCOME_MARKER_VERSION,
            context_id: later_receipt.context_id,
            round: later_receipt.round,
            subject: later_receipt.subject,
            manifest_hash: later_receipt.manifest_hash,
            body_frame_hash: later_receipt.frame_hash,
            outcome: ValidationOutcomeMarkerKind::Validated(conflicting_commitment),
        };
        write_validation_outcome_marker(&marker_path, &conflicting_marker)
            .expect("write a syntactically valid conflicting marker");
        drop(store);
        let error = match V2BodyStore::open_with_policy(
            directory.path(),
            context,
            BlockSignaturePolicy::GenesisAuthority(genesis.public_key().clone()),
        ) {
            Ok(_) => panic!("recovery must reject conflicting exact-body commitments"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            V2BodyStoreError::ConflictingValidationCommitment
        ));
    }
    #[test]
    fn result_bearing_proposal_is_rejected_before_durable_admission() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut result_bearing = decode_framed_signed_block(&body).expect("decode fixture body");
        result_bearing
            .set_transaction_results(Vec::new(), &[], Vec::new())
            .expect("attach empty deterministic execution result");
        assert!(!result_bearing.is_resultless_proposal());
        let result_bearing_wire = result_bearing
            .encode_wire()
            .expect("encode result-bearing body");
        let subject = wire::BlockSubject {
            parent_block_hash: result_bearing.header().prev_block_hash(),
            block_hash: result_bearing.hash(),
            payload_hash: Hash::new(&result_bearing_wire),
        };
        let result_bearing_manifest =
            encode_payload(&context, manifest.round, subject, &result_bearing_wire)
                .expect("encode result-bearing fixture payload")
                .manifest()
                .clone();
        let mut store = V2BodyStore::open(directory.path(), context).expect("open store");
        assert!(matches!(
            store.store(result_bearing_manifest, result_bearing_wire),
            Err(V2BodyStoreError::ResultBearingProposal)
        ));
    }
    #[test]
    fn typed_validation_deferral_and_durable_rejection_never_mint_success_receipts() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context).expect("open store");
        let receipt = store.store(manifest, body).expect("store exact body");
        let reference = missing_merge_reference(&receipt);
        let execution_commitment =
            ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
        let deferred = store
            .execute_durable_validation(receipt.clone(), receipt.manifest_hash(), |_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::MissingMergeSidecar(
                    reference.clone(),
                ))
            })
            .expect("classify exact missing sidecar as deferred");
        assert_eq!(deferred.missing_merge_sidecar(), Some(&reference));
        assert!(store.validated_recovery_catalog().is_empty());
        assert!(
            !store
                .validated_path_for(receipt.round(), receipt.subject())
                .exists()
        );
        let rejected = store
            .execute_durable_validation(receipt.clone(), receipt.manifest_hash(), |_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::Invalid(
                    "invalid candidate",
                ))
            })
            .expect("return terminal deterministic rejection");
        assert_eq!(rejected.rejection_reason(), Some("invalid candidate"));
        assert!(store.validated_recovery_catalog().is_empty());
        assert!(
            store
                .validated_path_for(receipt.round(), receipt.subject())
                .exists()
        );
        let marker_before_repeat =
            fs::read(store.validated_path_for(receipt.round(), receipt.subject()))
                .expect("rejection marker is durable before returning the outcome");
        let callback_ran = Cell::new(false);
        let repeated = store
            .execute_durable_validation(receipt.clone(), receipt.manifest_hash(), |_| {
                callback_ran.set(true);
                Ok::<_, FixtureValidationError>(execution_commitment)
            })
            .expect("exact repeat reuses the durable rejection");
        assert!(!callback_ran.get());
        assert_eq!(repeated.rejection_reason(), Some("invalid candidate"));
        assert_eq!(
            fs::read(store.validated_path_for(receipt.round(), receipt.subject()))
                .expect("read repeated rejection marker"),
            marker_before_repeat,
        );
        assert!(matches!(
            store.persist_validated_receipt(&receipt, execution_commitment),
            Err(V2BodyStoreError::ConflictingValidationOutcome)
        ));
        assert!(store.validated_recovery_catalog().is_empty());
    }
    #[test]
    fn durable_validation_persists_success_and_repeats_idempotently() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context).expect("open store");
        let receipt = store
            .store(manifest, body)
            .expect("persist exact candidate body");
        let expected_manifest_hash = receipt.manifest_hash();
        let execution_commitment =
            ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
        let validated = store
            .execute_durable_validation(receipt.clone(), expected_manifest_hash, |_| {
                Ok::<_, FixtureValidationError>(execution_commitment)
            })
            .expect("durable validation succeeds");
        assert_eq!(validated.durable_body(), &receipt);
        assert_eq!(
            validated
                .validated_receipt()
                .map(ValidatedBodyReceipt::execution_commitment),
            Some(execution_commitment)
        );
        let marker_path = store.validated_path_for(receipt.round(), receipt.subject());
        let marker_before_repeat = fs::read(&marker_path)
            .expect("success marker is durable before the outcome is returned");
        let files_before_repeat = durable_files_snapshot(directory.path());
        let validator_called = Cell::new(false);
        let repeated = store
            .execute_durable_validation(receipt.clone(), expected_manifest_hash, |_| {
                validator_called.set(true);
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::Invalid(
                    "idempotent validation must not rerun the callback",
                ))
            })
            .expect("repeat reuses the exact durable success");
        assert!(!validator_called.get());
        assert_eq!(repeated.durable_body(), &receipt);
        assert_eq!(repeated.validated_receipt(), validated.validated_receipt());
        assert_eq!(
            fs::read(marker_path).expect("read repeated success marker"),
            marker_before_repeat
        );
        assert_eq!(
            durable_files_snapshot(directory.path()),
            files_before_repeat
        );
        assert_eq!(
            repeated
                .into_validated_receipt()
                .expect("success-only extraction accepts the durable validation")
                .execution_commitment(),
            execution_commitment
        );
    }
    #[test]
    fn durable_validation_binds_rejection_and_typed_deferral_to_the_exact_body() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context).expect("open store");
        let receipt = store
            .store(manifest, body)
            .expect("persist exact candidate body");
        let expected_manifest_hash = receipt.manifest_hash();
        let marker_path = store.validated_path_for(receipt.round(), receipt.subject());
        let files_before = durable_files_snapshot(directory.path());
        let reference = missing_merge_reference(&receipt);
        let deferred = store
            .execute_durable_validation(receipt.clone(), expected_manifest_hash, |_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::MissingMergeSidecar(
                    reference.clone(),
                ))
            })
            .expect("return a reference-bound deferral outcome");
        assert_eq!(deferred.durable_body(), &receipt);
        assert_eq!(deferred.missing_merge_sidecar(), Some(&reference));
        assert!(deferred.validated_receipt().is_none());
        assert!(deferred.rejection_reason().is_none());
        assert!(deferred.rejection_identity().is_none());
        assert!(!marker_path.exists());
        assert!(store.validated_recovery_catalog().is_empty());
        assert_eq!(durable_files_snapshot(directory.path()), files_before);
        let rejected = store
            .execute_durable_validation(receipt.clone(), expected_manifest_hash, |_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::Invalid(
                    "candidate is invalid",
                ))
            })
            .expect("return a closed rejection outcome");
        assert_eq!(rejected.durable_body(), &receipt);
        assert_eq!(rejected.rejection_reason(), Some("candidate is invalid"));
        assert_eq!(
            rejected.rejection_identity(),
            Some(&BodyValidationRejectionIdentity::Rejected)
        );
        assert!(rejected.validated_receipt().is_none());
        assert!(rejected.missing_merge_sidecar().is_none());
        assert!(marker_path.exists());
        let marker = super::read_validation_outcome_marker(&marker_path)
            .expect("decode durable rejection marker");
        assert_eq!(marker.version, VALIDATION_OUTCOME_MARKER_VERSION);
        assert_eq!(
            marker.outcome,
            ValidationOutcomeMarkerKind::Rejected(
                BodyValidationRejectionIdentity::Rejected.canonical_code()
            )
        );
        assert_ne!(durable_files_snapshot(directory.path()), files_before);
        let rejected = rejected
            .into_validated_receipt()
            .expect_err("rejection must remain intact on the success-only path");
        assert_eq!(rejected.durable_body(), &receipt);
        assert_eq!(rejected.rejection_reason(), Some("candidate is invalid"));
        assert_eq!(
            rejected.rejection_identity(),
            Some(&BodyValidationRejectionIdentity::Rejected)
        );
        assert!(store.validated_recovery_catalog().is_empty());
    }
    #[test]
    fn durable_rejection_reopens_quarantined_and_promotes_only_after_exact_replay() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store =
            V2BodyStore::open(directory.path(), context.clone()).expect("open exact store");
        let receipt = store
            .store(manifest, body)
            .expect("persist exact candidate body");
        let marker_path = store.validated_path_for(receipt.round(), receipt.subject());
        let rejected = store
            .execute_durable_validation(receipt.clone(), receipt.manifest_hash(), |_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::Invalid(
                    "first volatile rejection diagnostic",
                ))
            })
            .expect("persist deterministic rejection");
        assert_eq!(
            rejected.rejection_identity(),
            Some(&BodyValidationRejectionIdentity::Rejected)
        );
        let marker_bytes = fs::read(&marker_path).expect("read durable rejection marker");
        assert!(
            !marker_bytes
                .windows(b"first volatile rejection diagnostic".len())
                .any(|window| window == b"first volatile rejection diagnostic"),
            "raw diagnostics must not enter durable authority"
        );
        drop(store);
        let mut reopened = V2BodyStore::open(directory.path(), context).expect("reopen store");
        assert_eq!(reopened.pending_revalidation.len(), 1);
        assert!(reopened.validated_recovery_catalog().is_empty());
        assert!(reopened.rejected.is_empty());
        assert!(matches!(
            reopened.ensure_recovered_markers_revalidated(),
            Err(V2BodyStoreError::UnrevalidatedValidationMarkers)
        ));
        let callback_count = Cell::new(0_usize);
        reopened
            .revalidate_recovered_markers(|_| {
                callback_count.set(callback_count.get().saturating_add(1));
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::Invalid(
                    "reproduced volatile rejection diagnostic",
                ))
            })
            .expect("exact rejection code reproduces the durable outcome");
        assert_eq!(callback_count.get(), 1);
        reopened
            .ensure_recovered_markers_revalidated()
            .expect("rejection marker crossed semantic replay");
        assert!(reopened.validated_recovery_catalog().is_empty());
        assert_eq!(reopened.rejected.len(), 1);
        let repeat_callback_ran = Cell::new(false);
        let repeated = reopened
            .execute_durable_validation(
                receipt.clone(),
                receipt.manifest_hash(),
                |_| -> Result<wire::ExecutionCommitment, FixtureValidationError> {
                    repeat_callback_ran.set(true);
                    unreachable!("an exact durable rejection repeat must not rerun validation")
                },
            )
            .expect("reuse semantically revalidated rejection");
        assert!(!repeat_callback_ran.get());
        assert_eq!(
            repeated.rejection_reason(),
            Some("reproduced volatile rejection diagnostic")
        );
        assert_eq!(
            fs::read(marker_path).expect("read unchanged rejection marker"),
            marker_bytes
        );
    }
    #[test]
    fn recovered_rejection_rejects_outcome_change_and_retires_on_missing_sidecar() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store =
            V2BodyStore::open(directory.path(), context.clone()).expect("open exact store");
        let receipt = store
            .store(manifest.clone(), body)
            .expect("persist exact candidate body");
        let _rejected = store
            .execute_durable_validation(receipt.clone(), receipt.manifest_hash(), |_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::Invalid(
                    "durable deterministic rejection",
                ))
            })
            .expect("persist rejection marker");
        drop(store);
        let mut reopened = V2BodyStore::open(directory.path(), context).expect("reopen store");
        let success = ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
        assert!(matches!(
            reopened.revalidate_recovered_markers(|_| { Ok::<_, FixtureValidationError>(success) }),
            Err(V2BodyStoreError::RecoveredValidationOutcomeMismatch)
        ));
        assert_eq!(reopened.pending_revalidation.len(), 1);
        assert!(reopened.rejected.is_empty());
        assert!(reopened.validated_recovery_catalog().is_empty());
        let reference = missing_merge_reference(&receipt);
        reopened
            .revalidate_recovered_markers(|_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::MissingMergeSidecar(
                    reference.clone(),
                ))
            })
            .expect("missing sidecar retires quarantined rejection authority");
        reopened
            .ensure_recovered_markers_revalidated()
            .expect("no quarantined marker authority survives deferral");
        assert!(reopened.rejected.is_empty());
        assert!(reopened.validated_recovery_catalog().is_empty());
        assert_eq!(reopened.retired_revalidation.len(), 1);
        assert!(matches!(
            reopened.execute_durable_validation(
                receipt.clone(),
                receipt.manifest_hash(),
                |_| Ok::<_, FixtureValidationError>(success),
            ),
            Err(V2BodyStoreError::RecoveredValidationOutcomeMismatch)
        ));
        assert_eq!(reopened.retired_revalidation.len(), 1);
        assert_eq!(
            reopened
                .recovered(manifest.round, manifest.subject)
                .expect("body remains available after marker retirement"),
            Some((manifest, receipt))
        );
    }
    #[test]
    fn durable_validation_preflight_errors_preserve_store_state_byte_for_byte() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store =
            V2BodyStore::open(directory.path(), context.clone()).expect("open exact store");
        let receipt = store
            .store(manifest, body)
            .expect("persist exact candidate body");
        let entries_before = store.entries.clone();
        let manifests_before = store.manifests.clone();
        let pending_before = store.pending_revalidation.clone();
        let retired_before = store.retired_revalidation.clone();
        let validated_before = store.validated.clone();
        let rejected_before = store.rejected.clone();
        let files_before = durable_files_snapshot(directory.path());
        let callback_called = Cell::new(false);
        let wrong_expected = HashOf::<wire::PayloadManifest>::from_untyped_unchecked(Hash::new(
            b"independently wrong expected manifest",
        ));
        let wrong_manifest = store.execute_durable_validation(
            receipt.clone(),
            wrong_expected,
            |_| -> Result<wire::ExecutionCommitment, FixtureValidationError> {
                callback_called.set(true);
                unreachable!("manifest mismatch must precede the validator")
            },
        );
        assert!(matches!(
            wrong_manifest,
            Err(V2BodyStoreError::ReceiptMismatch)
        ));
        assert!(!callback_called.get());
        assert_eq!(store.entries, entries_before);
        assert_eq!(store.manifests, manifests_before);
        assert_eq!(store.pending_revalidation, pending_before);
        assert_eq!(store.retired_revalidation, retired_before);
        assert_eq!(store.validated, validated_before);
        assert_eq!(store.rejected, rejected_before);
        assert_eq!(durable_files_snapshot(directory.path()), files_before);
        let foreign_directory = TempDir::new().expect("foreign temporary directory");
        let mut foreign_context = context;
        foreign_context.network_id =
            crate::sumeragi::synthetic_network_id("foreign-sumeragi-v2-body-store");
        let (foreign_body, foreign_manifest) = body_and_manifest(&foreign_context, &keys, None);
        let mut foreign_store = V2BodyStore::open(foreign_directory.path(), foreign_context)
            .expect("open foreign store");
        let foreign_receipt = foreign_store
            .store(foreign_manifest, foreign_body)
            .expect("persist foreign body");
        let foreign_result = store.execute_durable_validation(
            foreign_receipt.clone(),
            foreign_receipt.manifest_hash(),
            |_| -> Result<wire::ExecutionCommitment, FixtureValidationError> {
                callback_called.set(true);
                unreachable!("foreign receipt must precede the validator")
            },
        );
        assert!(matches!(
            foreign_result,
            Err(V2BodyStoreError::ReceiptMismatch)
        ));
        assert!(!callback_called.get());
        assert_eq!(store.entries, entries_before);
        assert_eq!(store.manifests, manifests_before);
        assert_eq!(store.pending_revalidation, pending_before);
        assert_eq!(store.retired_revalidation, retired_before);
        assert_eq!(store.validated, validated_before);
        assert_eq!(store.rejected, rejected_before);
        assert_eq!(durable_files_snapshot(directory.path()), files_before);
    }
    #[test]
    fn terminal_validate_outcome_catalog_drop_restores_both_maps_and_retired_seals() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let mut store = store_with_promoted_terminal_outcomes(directory.path(), &context, &keys);
        let (retired_key, retired) = store
            .validated
            .iter()
            .next()
            .map(|(key, validated)| {
                (
                    *key,
                    QuarantinedValidationOutcome {
                        durable: validated.durable().clone(),
                        outcome: ValidationOutcomeMarkerKind::Validated(
                            validated.execution_commitment(),
                        ),
                    },
                )
            })
            .expect("promoted success exists");
        store.retired_revalidation.insert(retired_key, retired);
        let validated_before = store.validated.clone();
        let rejected_before = store.rejected.clone();
        let retired_before = store.retired_revalidation.clone();
        {
            let _cut = store
                .detach_terminal_validate_outcome_catalog()
                .expect("detach aggregate terminal outcome catalog");
        }
        assert_eq!(store.validated, validated_before);
        assert_eq!(store.rejected, rejected_before);
        assert_eq!(store.retired_revalidation, retired_before);
    }
    #[test]
    fn terminal_validate_outcome_catalog_commit_restores_all_unselected_entries() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let mut store = store_with_promoted_terminal_outcomes(directory.path(), &context, &keys);
        let validated_before = store.validated.clone();
        let rejected_before = store.rejected.clone();
        store
            .detach_terminal_validate_outcome_catalog()
            .expect("detach aggregate terminal outcome catalog")
            .commit_selected();
        assert_eq!(store.validated, validated_before);
        assert_eq!(store.rejected, rejected_before);
    }
    #[test]
    fn terminal_validate_outcome_catalog_rejects_pending_markers_without_mutation() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let mut store = store_with_promoted_terminal_outcomes(directory.path(), &context, &keys);
        let (pending_key, pending) = store
            .validated
            .iter()
            .next()
            .map(|(key, validated)| {
                (
                    *key,
                    QuarantinedValidationOutcome {
                        durable: validated.durable().clone(),
                        outcome: ValidationOutcomeMarkerKind::Validated(
                            validated.execution_commitment(),
                        ),
                    },
                )
            })
            .expect("promoted success exists");
        store.pending_revalidation.insert(pending_key, pending);
        let pending_before = store.pending_revalidation.clone();
        let validated_before = store.validated.clone();
        let rejected_before = store.rejected.clone();
        let error = match store.detach_terminal_validate_outcome_catalog() {
            Ok(cut) => {
                drop(cut);
                panic!("pending semantic replay must prevent catalog detachment");
            }
            Err(error) => error,
        };
        assert_eq!(
            error,
            RecoveredTerminalValidateOutcomeCatalogError::UnrevalidatedMarkers
        );
        assert_eq!(store.pending_revalidation, pending_before);
        assert_eq!(store.validated, validated_before);
        assert_eq!(store.rejected, rejected_before);
    }
    #[test]
    fn terminal_validate_outcome_catalog_rejects_ambiguous_key_without_mutation() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let mut store = store_with_promoted_terminal_outcomes(directory.path(), &context, &keys);
        let (ambiguous_key, ambiguous_rejection) = store
            .validated
            .iter()
            .next()
            .map(|(key, validated)| {
                (
                    *key,
                    RevalidatedRejectedBody {
                        durable: validated.durable().clone(),
                        identity_code: BodyValidationRejectionIdentity::Rejected.canonical_code(),
                        reason: "volatile ambiguous diagnostic".to_owned(),
                    },
                )
            })
            .expect("promoted success exists");
        store.rejected.insert(ambiguous_key, ambiguous_rejection);
        let validated_before = store.validated.clone();
        let rejected_before = store.rejected.clone();
        let error = match store.detach_terminal_validate_outcome_catalog() {
            Ok(cut) => {
                drop(cut);
                panic!("ambiguous outcome key must prevent catalog detachment");
            }
            Err(error) => error,
        };
        assert_eq!(
            error,
            RecoveredTerminalValidateOutcomeCatalogError::AmbiguousOutcome
        );
        assert_eq!(store.validated, validated_before);
        assert_eq!(store.rejected, rejected_before);
    }
    crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
        terminal_validate_outcome_catalog_cut_is_opaque_and_move_only
    );
    crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
        durable_validation_surface_has_no_scheduler_identity_or_ordinal
    );
    #[test]
    fn rejection_marker_version_code_and_frame_binding_fail_closed() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store =
            V2BodyStore::open(directory.path(), context.clone()).expect("open exact store");
        let receipt = store.store(manifest, body).expect("store exact body");
        let _rejected = store
            .execute_durable_validation(receipt.clone(), receipt.manifest_hash(), |_| {
                Err::<wire::ExecutionCommitment, _>(FixtureValidationError::Invalid(
                    "durable rejection",
                ))
            })
            .expect("persist rejection marker");
        let marker_path = store.validated_path_for(receipt.round(), receipt.subject());
        let canonical_marker = super::read_validation_outcome_marker(&marker_path)
            .expect("decode canonical rejection marker");
        assert_eq!(
            canonical_marker.outcome,
            ValidationOutcomeMarkerKind::Rejected(
                BodyValidationRejectionIdentity::Rejected.canonical_code()
            )
        );
        drop(store);
        let mut wrong_frame = canonical_marker.clone();
        wrong_frame.body_frame_hash = Hash::new(b"foreign durable body frame");
        write_validation_outcome_marker(&marker_path, &wrong_frame)
            .expect("write checksum-valid foreign-frame marker");
        assert!(matches!(
            V2BodyStore::open(directory.path(), context.clone()),
            Err(V2BodyStoreError::ValidationMarkerMismatch)
        ));
        let mut unknown_code = canonical_marker.clone();
        unknown_code.outcome = ValidationOutcomeMarkerKind::Rejected(u8::MAX);
        write_validation_outcome_marker(&marker_path, &unknown_code)
            .expect("write checksum-valid unknown rejection code");
        assert!(matches!(
            V2BodyStore::open(directory.path(), context.clone()),
            Err(V2BodyStoreError::UnknownValidationRejectionIdentity(
                u8::MAX
            ))
        ));
        let mut unsupported_version = canonical_marker;
        unsupported_version.version = VALIDATION_OUTCOME_MARKER_VERSION.saturating_add(1);
        write_validation_outcome_marker(&marker_path, &unsupported_version)
            .expect("write checksum-valid unsupported marker version");
        assert!(matches!(
            V2BodyStore::open(directory.path(), context),
            Err(V2BodyStoreError::UnsupportedValidationOutcomeMarkerVersion(version))
                if version == VALIDATION_OUTCOME_MARKER_VERSION.saturating_add(1)
        ));
    }
    #[test]
    fn corrupted_or_orphaned_validation_marker_fails_closed() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let receipt = store.store(manifest, body).expect("store exact body");
        let execution_commitment =
            ValidatedBodyReceipt::for_test(receipt.clone()).execution_commitment();
        let _validated = store
            .validate(&receipt, |_| Ok::<_, &'static str>(execution_commitment))
            .expect("persist validation marker");
        let marker_path = store.validated_path_for(receipt.round(), receipt.subject());
        let mut marker_bytes = fs::read(&marker_path).expect("read marker");
        *marker_bytes.last_mut().expect("nonempty marker") ^= 0x80;
        fs::write(&marker_path, marker_bytes).expect("corrupt marker");
        drop(store);
        assert!(matches!(
            V2BodyStore::open(directory.path(), context.clone()),
            Err(V2BodyStoreError::ChecksumMismatch)
        ));
        fs::remove_file(&marker_path).expect("remove corrupt marker");
        let reopened = V2BodyStore::open(directory.path(), context.clone()).expect("reopen body");
        let marker = ValidationOutcomeMarker {
            version: VALIDATION_OUTCOME_MARKER_VERSION,
            context_id: receipt.context_id(),
            round: wire::ConsensusRound {
                view: receipt.round().view.saturating_add(1),
                ..receipt.round()
            },
            subject: receipt.subject(),
            manifest_hash: receipt.manifest_hash(),
            body_frame_hash: receipt.frame_hash,
            outcome: ValidationOutcomeMarkerKind::Validated(execution_commitment),
        };
        let orphan_path = reopened.validated_path_for(marker.round, marker.subject);
        write_validation_outcome_marker(&orphan_path, &marker).expect("write orphan marker");
        drop(reopened);
        assert!(matches!(
            V2BodyStore::open(directory.path(), context),
            Err(V2BodyStoreError::OrphanedValidationMarker)
        ));
    }
    #[test]
    fn final_file_corruption_fails_closed_but_incomplete_temp_is_ignored() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let (body, manifest) = body_and_manifest(&context, &keys, None);
        let mut store = V2BodyStore::open(directory.path(), context.clone()).expect("open store");
        let _receipt = store.store(manifest, body).expect("store body");
        let context_directory = directory.path().join(hex::encode(context.id().0.as_ref()));
        fs::write(context_directory.join("interrupted.norito.tmp"), b"partial")
            .expect("write incomplete temp file");
        V2BodyStore::open(directory.path(), context.clone())
            .expect("incomplete temp is unacknowledged");
        let final_path = fs::read_dir(&context_directory)
            .expect("list context directory")
            .map(|entry| entry.expect("directory entry").path())
            .find(|path| path.extension().and_then(|value| value.to_str()) == Some("norito"))
            .expect("durable final body");
        let mut bytes = fs::read(&final_path).expect("read final body");
        let last = bytes.last_mut().expect("non-empty frame");
        *last ^= 0x80;
        fs::write(&final_path, bytes).expect("corrupt final body");
        assert!(matches!(
            V2BodyStore::open(directory.path(), context),
            Err(V2BodyStoreError::ChecksumMismatch)
        ));
    }
    #[test]
    fn wrong_leader_signature_is_rejected_before_durability() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let leader = usize::try_from(context.leader(0)).expect("leader index");
        let wrong = (leader + 1) % keys.len();
        let (body, manifest) = body_and_manifest(&context, &keys, Some(wrong));
        let mut store = V2BodyStore::open(directory.path(), context).expect("open store");
        assert!(matches!(
            store.store(manifest, body),
            Err(V2BodyStoreError::InvalidExpectedSignature)
        ));
    }
    #[test]
    fn height_one_can_require_the_distinct_genesis_authority() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, _keys) = context_and_keys();
        let genesis = KeyPair::try_from_seed(vec![0xA5; 32], Algorithm::Ed25519)
            .expect("deterministic genesis key");
        let impostor = KeyPair::try_from_seed(vec![0x5A; 32], Algorithm::Ed25519)
            .expect("deterministic impostor key");
        let (body, manifest) = body_and_manifest_with_signature(&context, &genesis, 0);
        let mut store = V2BodyStore::open_with_policy(
            directory.path(),
            context.clone(),
            BlockSignaturePolicy::GenesisAuthority(genesis.public_key().clone()),
        )
        .expect("open genesis body store");
        let _receipt = store
            .store(manifest, body)
            .expect("configured genesis signature is accepted");
        let other_directory = TempDir::new().expect("other temporary directory");
        let (body, manifest) = body_and_manifest_with_signature(&context, &impostor, 0);
        let mut store = V2BodyStore::open_with_policy(
            other_directory.path(),
            context,
            BlockSignaturePolicy::GenesisAuthority(genesis.public_key().clone()),
        )
        .expect("open genesis body store");
        assert!(matches!(
            store.store(manifest, body),
            Err(V2BodyStoreError::InvalidExpectedSignature)
        ));
    }
    #[test]
    fn fixed_genesis_body_can_be_reproposed_after_a_certified_view_change() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, _keys) = context_and_keys();
        let genesis = KeyPair::try_from_seed(vec![0xC3; 32], Algorithm::Ed25519)
            .expect("deterministic genesis key");
        let (body, manifest) =
            body_and_manifest_with_signature_and_views(&context, &genesis, 0, 3, 0);
        let mut store = V2BodyStore::open_with_policy(
            directory.path(),
            context,
            BlockSignaturePolicy::GenesisAuthority(genesis.public_key().clone()),
        )
        .expect("open genesis body store");
        let _receipt = store
            .store(manifest, body)
            .expect("fixed signed genesis body is valid in a later proposal view");
    }
    #[test]
    fn rotating_leader_reproposal_authenticates_the_immutable_header_leader() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let origin_view = 1;
        let later_view = 4;
        let origin_leader = usize::try_from(context.leader(origin_view)).expect("leader index");
        let (body, manifest) = body_and_manifest_with_signature_and_views(
            &context,
            &keys[origin_leader],
            u64::try_from(origin_leader).expect("leader index fits u64"),
            later_view,
            origin_view,
        );
        let mut store = V2BodyStore::open(directory.path(), context).expect("open body store");
        let _ = store
            .store(manifest, body)
            .expect("a later reproposal retains the original header leader signature");
    }
    #[test]
    fn body_from_a_future_view_is_rejected() {
        let directory = TempDir::new().expect("temporary directory");
        let (context, keys) = context_and_keys();
        let proposal_view = 2;
        let future_origin_view = 3;
        let future_leader =
            usize::try_from(context.leader(future_origin_view)).expect("leader index");
        let (body, manifest) = body_and_manifest_with_signature_and_views(
            &context,
            &keys[future_leader],
            u64::try_from(future_leader).expect("leader index fits u64"),
            proposal_view,
            future_origin_view,
        );
        let mut store = V2BodyStore::open(directory.path(), context).expect("open body store");
        assert!(matches!(
            store.store(manifest, body),
            Err(V2BodyStoreError::BlockSubjectMismatch)
        ));
    }
}
