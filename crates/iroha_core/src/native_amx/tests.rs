    use super::*;
    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_data_model::{
        block::{
            consensus::NativeAmxAttestationBodyV2,
            consensus_v2::{ConsensusRound, HeightContext, HeightContextId},
        },
        nexus::{DataSpaceId, LaneId},
        peer::PeerId,
        transaction::TransactionEntrypoint,
    };
    use std::num::NonZeroUsize;
    fn checked_bls_keypair(seed: u8) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
            .expect("generate checked native AMX BLS fixture keypair")
    }
    fn checked_bls_signature_payload(keypair: &KeyPair, message: &[u8]) -> Vec<u8> {
        let signature = Signature::try_new(keypair.private_key(), message)
            .expect("checked native AMX vote fixture signature");
        signature
            .verify(keypair.public_key(), message)
            .expect("checked native AMX vote fixture signature verifies");
        signature.payload().to_vec()
    }
    fn body(phase: NativeAmxPhase) -> NativeAmxAttestationBodyV2 {
        let mut body = NativeAmxAttestationBodyV2 {
            round: ConsensusRound {
                context_id: HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
                    Hash::new(b"native-amx-v2-test-context"),
                )),
                height: 42,
                view: 3,
            },
            epoch: 7,
            network_id: network_id(b"native-amx-v2-test-genesis"),
            source_id: [0xCD; iroha_crypto::Hash::LENGTH],
            tx_entrypoint_hash:
                iroha_crypto::HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
                    Hash::prehashed([0xCD; iroha_crypto::Hash::LENGTH]),
                ),
            plan_digest: Hash::new(b"native-amx-plan"),
            phase,
            coordinator_lane_id: LaneId::new(1),
            coordinator_dataspace_id: DataSpaceId::new(7),
            coordinator_lane_incarnation: Hash::new(b"native-amx-v2-coordinator-incarnation"),
            participant_lane_id: LaneId::new(2),
            participant_dataspace_id: DataSpaceId::new(8),
            participant_lane_incarnation: Hash::new(b"native-amx-v2-participant-incarnation"),
            participant_previous_block_height: 0,
            participant_previous_block_descriptor_hash: None,
            participant_lane_block_height: 1,
            participant_lane_block_view: 0,
            participant_proposal_hash: Hash::new(b"native-amx-v2-participant-proposal"),
            participant_settlement_commitment: Hash::prehashed([0; Hash::LENGTH]),
            participant_validator_set_hash: HashOf::new(&Vec::<PeerId>::new()),
            participant_validator_count: 1,
            participant_min_quorum: 1,
            authority_context_height: 42,
            planned_coordinator_block_height: 42,
            coordinator_lane_block_view: 3,
            coordinator_proposal_hash: Hash::new(b"native-amx-v2-coordinator-proposal"),
        };
        body.participant_settlement_commitment = body
            .computed_grouped_participant_settlement_commitment(&[body.source_id])
            .expect("single-source test fixture settlement is valid");
        body
    }
    fn signing_guard_signer(seed: u8) -> (KeyPair, PeerId) {
        let keypair = checked_bls_keypair(seed);
        let signer = PeerId::new(keypair.public_key().clone());
        (keypair, signer)
    }
    fn signing_guard_capacity(value: usize) -> NonZeroUsize {
        NonZeroUsize::new(value).expect("test signing capacity is non-zero")
    }
    fn signing_guard_limits(max_records: usize) -> NativeAmxSigningGuardLimits {
        NativeAmxSigningGuardLimits::new(
            signing_guard_capacity(max_records),
            iroha_config::parameters::defaults::sumeragi::V2_NATIVE_AMX_SIGNING_GUARD_RECORD_BYTES,
            iroha_config::parameters::defaults::sumeragi::V2_NATIVE_AMX_SIGNING_GUARD_ANCHOR_BYTES,
        )
        .expect("test signing guard limits are valid")
    }
    #[test]
    fn signing_journal_identity_ignores_ambient_norito_layout() {
        let (_, signer) = signing_guard_signer(0x6D);
        let body = body(NativeAmxPhase::Commit);
        let binding = NativeAmxHeightBindingV2 {
            active_height: body.authority_context_height,
            context_id: body.round.context_id,
            epoch: body.epoch,
            network_id: body.network_id,
            signer: signer.clone(),
            max_records: 8,
        };
        let genesis_head = binding
            .genesis_head()
            .expect("derive canonical genesis head");
        let record = NativeAmxSigningRecordV2::from_body(1, genesis_head, &body, &signer)
            .expect("derive canonical signing record");
        let body_digest = record
            .computed_body_digest()
            .expect("derive canonical body digest");
        let record_hash = record
            .computed_record_hash()
            .expect("derive canonical record hash");
        let canonical_record =
            norito::encode_canonical(&record).expect("encode canonical signing record");
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        assert_ne!(
            norito::to_bytes(&record).expect("encode alternate-layout signing record"),
            canonical_record,
            "fixture must exercise a distinct ambient Norito layout"
        );
        assert_eq!(
            binding
                .genesis_head()
                .expect("derive genesis head under alternate layout"),
            genesis_head
        );
        assert_eq!(
            record
                .computed_body_digest()
                .expect("derive body digest under alternate layout"),
            body_digest
        );
        assert_eq!(
            record
                .computed_record_hash()
                .expect("derive record hash under alternate layout"),
            record_hash
        );
    }
    #[cfg(unix)]
    fn open_signing_guard(
        root: &Path,
        body: &NativeAmxAttestationBodyV2,
        signer: PeerId,
        max_records: usize,
    ) -> Result<NativeAmxSigningGuard, NativeAmxSigningGuardError> {
        NativeAmxSigningGuard::open(
            root,
            body.authority_context_height,
            body.round.context_id,
            body.epoch,
            body.network_id,
            signer,
            signing_guard_limits(max_records),
        )
    }
    #[cfg(unix)]
    fn open_guard(
        root: &Path,
        body: &NativeAmxAttestationBodyV2,
        signer: PeerId,
        max_records: usize,
    ) -> NativeAmxSigningGuard {
        open_signing_guard(root, body, signer, max_records).expect("open signing guard fixture")
    }
    #[cfg(unix)]
    fn assert_unsafe_open(
        root: &Path,
        body: &NativeAmxAttestationBodyV2,
        signer: PeerId,
        max_records: usize,
    ) {
        assert!(matches!(
            open_signing_guard(root, body, signer, max_records),
            Err(NativeAmxSigningGuardError::UnsafeJournal(_))
        ));
    }
    #[cfg(unix)]
    fn signing_record_paths(guard: &NativeAmxSigningGuard) -> Vec<PathBuf> {
        let mut paths = fs::read_dir(&guard.directory)
            .expect("read signer journal")
            .map(|entry| entry.expect("journal entry"))
            .filter(|entry| native_amx_valid_record_filename(&entry.file_name().to_string_lossy()))
            .map(|entry| entry.path())
            .collect::<Vec<_>>();
        paths.sort();
        paths
    }
    #[cfg(unix)]
    fn signing_record_path(guard: &NativeAmxSigningGuard) -> PathBuf {
        signing_record_paths(guard)
            .into_iter()
            .next()
            .expect("signing record")
    }
    #[cfg(unix)]
    fn write_secure_new(path: &Path, bytes: &[u8]) {
        let mut options = OpenOptions::new();
        options
            .create_new(true)
            .write(true)
            .mode(NATIVE_AMX_SIGNING_FILE_MODE);
        let mut file = options.open(path).expect("create secure test record");
        file.write_all(bytes).expect("write secure test record");
        file.sync_all().expect("sync secure test record");
        fs::set_permissions(
            path,
            fs::Permissions::from_mode(NATIVE_AMX_SIGNING_FILE_MODE),
        )
        .expect("set secure test record mode");
    }
    #[cfg(unix)]
    fn write_v4_signing_journal(
        root: &Path,
        signer: &PeerId,
        bodies: &[NativeAmxAttestationBodyV2],
        max_records: usize,
    ) -> PathBuf {
        let first = bodies.first().expect("V4 fixture has at least one record");
        let owner_uid = native_amx_effective_user_id(root).expect("effective uid");
        let signer_digest =
            native_amx_signer_directory_digest(root, signer).expect("signer digest");
        let legacy_root = root.join(NATIVE_AMX_V4_SIGNING_GUARD_DIRECTORY);
        native_amx_ensure_secure_directory(&legacy_root, owner_uid).expect("create secure V4 root");
        let directory = legacy_root.join(signer_digest.to_string());
        native_amx_ensure_secure_directory(&directory, owner_uid)
            .expect("create secure V4 signer journal");
        let binding = NativeAmxHeightBindingV2 {
            active_height: first.authority_context_height,
            context_id: first.round.context_id,
            epoch: first.epoch,
            network_id: first.network_id,
            signer: signer.clone(),
            max_records: u32::try_from(max_records).expect("fixture capacity fits u32"),
        };
        let mut anchor =
            NativeAmxSigningAnchorV4::empty_for_test(binding).expect("create V4 anchor");
        for (index, body) in bodies.iter().enumerate() {
            let sequence = u32::try_from(index + 1).expect("fixture sequence fits u32");
            let record = NativeAmxSigningRecordV4::from_body_for_test(
                sequence,
                anchor.head_hash,
                body,
                signer,
            )
            .expect("create V4 signing record");
            let bytes = norito::encode_canonical(&record).expect("encode V4 signing record");
            write_secure_new(
                &NativeAmxSigningGuard::v4_record_path(&directory, &record),
                &bytes,
            );
            anchor.record_count = sequence;
            anchor.head_hash = record.record_hash;
            anchor.highest_view = Some(
                anchor
                    .highest_view
                    .map_or(body.round.view, |view| view.max(body.round.view)),
            );
        }
        let anchor_bytes = norito::encode_canonical(&anchor).expect("encode V4 anchor");
        write_secure_new(
            &directory.join(NATIVE_AMX_SIGNING_GUARD_ANCHOR_FILE),
            &anchor_bytes,
        );
        native_amx_sync_directory_path(&directory).expect("sync V4 signer journal");
        native_amx_sync_directory_path(&legacy_root).expect("sync V4 root");
        directory
    }
    fn another_context(label: &[u8]) -> HeightContextId {
        HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(Hash::new(
            label,
        )))
    }
    fn set_source(body: &mut NativeAmxAttestationBodyV2, byte: u8) {
        body.source_id = [byte; Hash::LENGTH];
        body.tx_entrypoint_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
            Hash::prehashed(body.source_id),
        );
    }
    #[cfg(unix)]
    fn record_body(guard: &NativeAmxSigningGuard, body: &NativeAmxAttestationBodyV2) {
        guard.record_body_for_test(body).expect("record fixture body");
    }
    #[test]
    fn participant_leg_cap_reserves_one_slot_for_the_coordinator() {
        assert_eq!(
            MAX_NATIVE_AMX_PARTICIPANT_LEGS + 1,
            MAX_NATIVE_AMX_PLAN_LEGS
        );
        assert!(native_amx_participant_leg_count_within_limit(0));
        assert!(native_amx_participant_leg_count_within_limit(
            MAX_NATIVE_AMX_PARTICIPANT_LEGS
        ));
        assert!(!native_amx_participant_leg_count_within_limit(
            MAX_NATIVE_AMX_PLAN_LEGS
        ));
    }
    #[cfg(not(unix))]
    #[test]
    fn signing_guard_fails_closed_on_unsupported_filesystems() {
        let body = body(NativeAmxPhase::Prepare);
        let (_keypair, signer) = signing_guard_signer(0x70);
        assert!(matches!(
            NativeAmxSigningGuard::open(
                Path::new("."),
                body.authority_context_height,
                body.round.context_id,
                body.epoch,
                body.network_id,
                signer,
                signing_guard_limits(8),
            ),
            Err(NativeAmxSigningGuardError::UnsupportedPlatform)
        ));
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_rejects_legacy_signer_journal_instead_of_ignoring_it() {
        for (index, legacy_name) in NATIVE_AMX_LEGACY_SIGNING_GUARD_DIRECTORIES
            .iter()
            .enumerate()
        {
            let root = tempfile::tempdir().expect("temp dir");
            let body = body(NativeAmxPhase::Prepare);
            let seed = 0x6F_u8.saturating_add(u8::try_from(index).expect("legacy index fits u8"));
            let (_keypair, signer) = signing_guard_signer(seed);
            let owner_uid = native_amx_effective_user_id(root.path()).expect("effective uid");
            let signer_digest =
                native_amx_signer_directory_digest(root.path(), &signer).expect("signer digest");
            let legacy_root = root.path().join(*legacy_name);
            native_amx_ensure_secure_directory(&legacy_root, owner_uid)
                .expect("create secure legacy root");
            native_amx_ensure_secure_directory(
                &legacy_root.join(signer_digest.to_string()),
                owner_uid,
            )
            .expect("create secure legacy signer journal");
            assert!(matches!(
                open_signing_guard(root.path(), &body, signer, 8),
                Err(NativeAmxSigningGuardError::UnsafeJournal(message))
                    if message.contains("authenticated recovery")
            ));
            assert!(
                !root
                    .path()
                    .join(NATIVE_AMX_SIGNING_GUARD_DIRECTORY)
                    .exists(),
                "legacy directory {legacy_name} must fail before V5 journal creation"
            );
        }
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_migrates_v4_commits_and_quarantines_v4_prepare_view() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0xA5);
        let prepare = body(NativeAmxPhase::Prepare);
        let mut commit = prepare;
        commit.phase = NativeAmxPhase::Commit;
        let legacy_directory =
            write_v4_signing_journal(root.path(), &signer, &[prepare, commit], 8);
        let guard = open_guard(root.path(), &prepare, signer.clone(), 8);
        let inner = guard.inner.lock();
        assert_eq!(inner.anchor.record_count, 1);
        assert_eq!(inner.anchor.last_prepare_view, Some(prepare.round.view));
        assert_eq!(inner.prepare_quarantine_view, Some(prepare.round.view));
        assert_eq!(inner.records.len(), 1);
        drop(inner);
        record_body(&guard, &commit);
        assert_eq!(
            guard.record_body_for_test(&prepare),
            Err(NativeAmxSigningGuardError::PrepareViewQuarantined {
                view: prepare.round.view,
            })
        );
        assert!(!legacy_directory.exists());
        let retired = legacy_directory.with_file_name(format!(
            "{}{}",
            legacy_directory
                .file_name()
                .expect("legacy signer basename")
                .to_string_lossy(),
            NATIVE_AMX_V4_RETIRED_SUFFIX
        ));
        assert!(retired.exists(), "V4 evidence is atomically retired");
        drop(guard);
        open_guard(root.path(), &prepare, signer, 8);
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_v4_migration_preserves_authenticated_cross_view_order() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0xA9);
        let mut earlier = body(NativeAmxPhase::Commit);
        earlier.source_id = [0xFE; Hash::LENGTH];
        let mut later = earlier;
        later.round.view = earlier.round.view + 1;
        later.source_id = [0x01; Hash::LENGTH];
        write_v4_signing_journal(root.path(), &signer, &[earlier, later], 8);

        let guard = open_guard(root.path(), &earlier, signer, 8);
        let mut migrated = guard
            .inner
            .lock()
            .records
            .values()
            .map(|record| (record.sequence, record.body.round.view))
            .collect::<Vec<_>>();
        migrated.sort_by_key(|(sequence, _)| *sequence);
        assert_eq!(
            migrated,
            vec![(1, earlier.round.view), (2, later.round.view)]
        );
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_v4_upgrade_at_next_height_starts_empty() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0xA6);
        let prepare = body(NativeAmxPhase::Prepare);
        let mut commit = prepare;
        commit.phase = NativeAmxPhase::Commit;
        write_v4_signing_journal(root.path(), &signer, &[prepare, commit], 8);
        let next_context = another_context(b"V4-to-V5-next-height");
        let mut next = prepare;
        next.round.context_id = next_context;
        next.round.height += 1;
        next.round.view = 0;
        next.authority_context_height += 1;
        next.planned_coordinator_block_height += 1;
        next.coordinator_lane_block_view = 0;
        let guard = open_guard(root.path(), &next, signer, 8);
        {
            let inner = guard.inner.lock();
            assert_eq!(inner.anchor.record_count, 0);
            assert_eq!(inner.anchor.highest_view, None);
            assert_eq!(inner.anchor.last_prepare_view, None);
            assert_eq!(inner.prepare_quarantine_view, None);
        }
        record_body(&guard, &next);
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_v4_migration_recovers_an_unpublished_v5_prefix() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0xA7);
        let prepare = body(NativeAmxPhase::Prepare);
        let mut commit = prepare;
        commit.phase = NativeAmxPhase::Commit;
        write_v4_signing_journal(root.path(), &signer, &[prepare, commit], 8);
        let (directory, owner_uid, _) =
            native_amx_ensure_signer_directory(root.path(), &signer).expect("create V5 directory");
        let binding = NativeAmxHeightBindingV2 {
            active_height: prepare.authority_context_height,
            context_id: prepare.round.context_id,
            epoch: prepare.epoch,
            network_id: prepare.network_id,
            signer: signer.clone(),
            max_records: 8,
        };
        let empty_anchor = NativeAmxSigningAnchorV2::empty(binding).expect("empty V5 anchor");
        let prefix =
            NativeAmxSigningRecordV2::from_body(1, empty_anchor.head_hash, &commit, &signer)
                .expect("V5 migration prefix");
        write_secure_new(
            &NativeAmxSigningGuard::record_path(&directory, &prefix),
            &norito::encode_canonical(&prefix).expect("encode V5 prefix"),
        );
        native_amx_sync_directory_path(&directory).expect("sync V5 prefix");
        let guard = open_guard(root.path(), &prepare, signer, 8);
        assert_eq!(guard.inner.lock().anchor.record_count, 1);
        assert_eq!(
            native_amx_secure_file_identity(
                &NativeAmxSigningGuard::record_path(&directory, &prefix),
                signing_guard_limits(8).max_record_bytes.get(),
                owner_uid,
            )
            .is_ok(),
            true
        );
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_v4_migration_fails_closed_on_corrupt_evidence() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0xA8);
        let prepare = body(NativeAmxPhase::Prepare);
        let legacy_directory = write_v4_signing_journal(root.path(), &signer, &[prepare], 8);
        let record_path = fs::read_dir(&legacy_directory)
            .expect("read V4 journal")
            .map(|entry| entry.expect("V4 entry"))
            .find(|entry| native_amx_valid_record_filename(&entry.file_name().to_string_lossy()))
            .expect("V4 record")
            .path();
        let mut bytes = fs::read(&record_path).expect("read V4 record");
        let last = bytes.last_mut().expect("non-empty V4 record");
        *last ^= 0x01;
        fs::write(&record_path, bytes).expect("corrupt V4 record");
        assert_unsafe_open(root.path(), &prepare, signer, 8);
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_certified_view_retires_old_commit_claims_and_reclaims_capacity() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0xA9);
        let old = body(NativeAmxPhase::Commit);
        let guard = open_guard(root.path(), &old, signer.clone(), 1);
        record_body(&guard, &old);
        guard
            .advance_certified_view(old.round.view + 1)
            .expect("retire strictly older Commit claim");
        assert_eq!(guard.record_count_for_test(), 0);
        let mut next = old;
        next.round.view += 1;
        next.participant_proposal_hash = Hash::new(b"certified-view replacement proposal");
        record_body(&guard, &next);
        assert_eq!(guard.record_count_for_test(), 1);
        drop(guard);
        let restarted = open_guard(root.path(), &old, signer, 1);
        record_body(&restarted, &next);
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_certified_view_preserves_same_view_commit_on_retain_and_restart() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0xAA);
        let commit = body(NativeAmxPhase::Commit);
        let guard = open_guard(root.path(), &commit, signer.clone(), 2);
        record_body(&guard, &commit);
        guard
            .advance_certified_view(commit.round.view)
            .expect("retain certified-view Commit");
        guard
            .advance_certified_view(commit.round.view)
            .expect("same certified view is idempotent");
        assert_eq!(guard.record_count_for_test(), 1);
        let mut conflict = commit;
        conflict.participant_proposal_hash = Hash::new(b"same-view conflicting proposal");
        assert_eq!(
            guard.record_body_for_test(&conflict),
            Err(NativeAmxSigningGuardError::Equivocation)
        );
        drop(guard);
        let restarted = open_guard(root.path(), &commit, signer, 2);
        record_body(&restarted, &commit);
        assert_eq!(
            restarted.record_body_for_test(&conflict),
            Err(NativeAmxSigningGuardError::Equivocation)
        );
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_restart_cleans_retired_prefix_left_after_anchor_publication() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0xAB);
        let old = body(NativeAmxPhase::Commit);
        let guard = open_guard(root.path(), &old, signer.clone(), 4);
        record_body(&guard, &old);
        let mut retained = old;
        retained.round.view += 1;
        retained.source_id = [0xAB; Hash::LENGTH];
        retained.tx_entrypoint_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
            Hash::prehashed(retained.source_id),
        );
        retained.participant_lane_block_height += 1;
        retained.participant_previous_block_height = 1;
        retained.participant_previous_block_descriptor_hash =
            Some(Hash::new(b"certified-prefix predecessor"));
        retained.participant_proposal_hash = Hash::new(b"certified-prefix retained proposal");
        retained.participant_settlement_commitment = retained
            .computed_grouped_participant_settlement_commitment(&[retained.source_id])
            .expect("retained settlement is valid");
        record_body(&guard, &retained);
        let (old_path, old_record) = {
            let inner = guard.inner.lock();
            let old_key = NativeAmxSigningKeyV2::from_body(&old, &signer);
            let old_record = inner.records.get(&old_key).expect("old record").clone();
            let old_path = inner
                .record_identities
                .get(&old_key)
                .expect("old record identity")
                .0
                .clone();
            let mut checkpoint = inner.anchor.clone();
            checkpoint.certified_view = Some(retained.round.view);
            checkpoint.record_floor = old_record.sequence;
            checkpoint.floor_head = old_record.record_hash;
            NativeAmxSigningGuard::persist_anchor(
                &guard.directory,
                &guard.directory_handle,
                guard.owner_uid,
                &checkpoint,
                guard.limits.max_anchor_bytes.get(),
            )
            .expect("publish prefix checkpoint before simulated crash");
            (old_path, old_record)
        };
        assert!(
            old_path.exists(),
            "simulated crash leaves retired prefix file"
        );
        drop(guard);
        let restarted = open_guard(root.path(), &retained, signer, 4);
        assert!(!old_path.exists());
        assert_eq!(restarted.record_count_for_test(), 1);
        assert_eq!(
            restarted.inner.lock().anchor.floor_head,
            old_record.record_hash
        );
        record_body(&restarted, &retained);
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_prepare_restart_quarantines_same_view_and_releases_higher_view() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x71);
        let body = body(NativeAmxPhase::Prepare);
        let guard = open_guard(root.path(), &body, signer.clone(), 8);
        record_body(&guard, &body);
        record_body(&guard, &body);
        let mut conflict = body;
        conflict.coordinator_proposal_hash = Hash::new(b"conflicting coordinator proposal");
        assert_eq!(
            guard.record_body_for_test(&conflict),
            Err(NativeAmxSigningGuardError::Equivocation)
        );
        drop(guard);
        let restarted = open_guard(root.path(), &body, signer, 8);
        assert_eq!(
            restarted.record_body_for_test(&body),
            Err(NativeAmxSigningGuardError::PrepareViewQuarantined {
                view: body.round.view,
            })
        );
        assert_eq!(
            restarted.record_body_for_test(&conflict),
            Err(NativeAmxSigningGuardError::PrepareViewQuarantined {
                view: body.round.view,
            })
        );
        conflict.round.view = conflict.round.view.saturating_add(1);
        record_body(&restarted, &conflict);
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_fresh_reopen_without_prepare_marker_keeps_current_view_available() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x97);
        let body = body(NativeAmxPhase::Prepare);
        let guard = open_guard(root.path(), &body, signer.clone(), 8);
        assert_eq!(guard.inner.lock().anchor.last_prepare_view, None);
        drop(guard);

        let reopened = open_guard(root.path(), &body, signer, 8);
        record_body(&reopened, &body);
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_reuses_one_durable_decision_across_global_views() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x96);
        let base = body(NativeAmxPhase::Commit);
        let guard = open_guard(root.path(), &base, signer.clone(), 1);
        record_body(&guard, &base);
        let mut next_view = base;
        next_view.round.view = next_view.round.view.saturating_add(1);
        record_body(&guard, &next_view);
        assert_eq!(guard.record_count_for_test(), 1);
        drop(guard);

        let restarted = open_guard(root.path(), &base, signer, 1);
        let mut later_view = next_view;
        later_view.round.view = later_view.round.view.saturating_add(1);
        record_body(&restarted, &later_view);
        assert_eq!(restarted.record_count_for_test(), 1);
        assert_eq!(
            restarted.record_body_for_test(&next_view),
            Err(NativeAmxSigningGuardError::StaleView {
                attempted_view: next_view.round.view,
                highest_view: later_view.round.view,
            })
        );
    }
    include!("signing_guard_boundary_tests.rs");
    #[cfg(unix)]
    #[test]
    fn signing_guard_durable_commit_rejects_conflicting_later_prepares_across_restart() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x8D);
        let first = body(NativeAmxPhase::Commit);
        let guard = open_guard(root.path(), &first, signer.clone(), 8);
        record_body(&guard, &first);
        let mut conflicting_proposal = first;
        conflicting_proposal.phase = NativeAmxPhase::Prepare;
        conflicting_proposal.round.view += 1;
        conflicting_proposal.participant_proposal_hash = Hash::new(b"slot-conflicting proposal");
        assert_eq!(
            guard.record_body_for_test(&conflicting_proposal),
            Err(NativeAmxSigningGuardError::SlotEquivocation)
        );
        assert_eq!(guard.record_count_for_test(), 1);
        let mut conflicting_settlement = first;
        conflicting_settlement.phase = NativeAmxPhase::Prepare;
        conflicting_settlement.round.view += 1;
        conflicting_settlement.participant_settlement_commitment =
            Hash::new(b"slot-conflicting settlement only");
        assert_eq!(
            guard.record_body_for_test(&conflicting_settlement),
            Err(NativeAmxSigningGuardError::SlotEquivocation)
        );
        assert_eq!(guard.record_count_for_test(), 1);
        let mut conflicting = first;
        conflicting.phase = NativeAmxPhase::Prepare;
        conflicting.round.view += 1;
        conflicting.source_id = [0xEF; Hash::LENGTH];
        conflicting.tx_entrypoint_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
            Hash::prehashed(conflicting.source_id),
        );
        conflicting.participant_settlement_commitment = conflicting
            .computed_grouped_participant_settlement_commitment(&[conflicting.source_id])
            .expect("single-source test fixture settlement is valid");
        assert_eq!(
            guard.record_body_for_test(&conflicting),
            Err(NativeAmxSigningGuardError::SlotEquivocation)
        );
        assert_eq!(guard.record_count_for_test(), 1);
        drop(guard);
        conflicting.round.view += 1;
        let restarted = open_guard(root.path(), &first, signer, 8);
        assert_eq!(
            restarted.record_body_for_test(&conflicting_settlement),
            Err(NativeAmxSigningGuardError::SlotEquivocation)
        );
        assert_eq!(restarted.record_count_for_test(), 1);
        assert_eq!(
            restarted.record_body_for_test(&conflicting),
            Err(NativeAmxSigningGuardError::SlotEquivocation)
        );
        assert_eq!(restarted.record_count_for_test(), 1);
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_binds_context_epoch_and_monotonic_view_then_resets_next_height() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x73);
        let base = body(NativeAmxPhase::Commit);
        let guard = open_guard(root.path(), &base, signer.clone(), 8);
        guard.record_body_for_test(&base).expect("record base view");
        let mut high = base;
        high.round.view += 2;
        record_body(&guard, &high);
        drop(guard);
        assert!(matches!(
            NativeAmxSigningGuard::open(
                root.path(),
                base.authority_context_height,
                another_context(b"same-height-context-drift"),
                base.epoch,
                base.network_id,
                signer.clone(),
                signing_guard_limits(8),
            ),
            Err(NativeAmxSigningGuardError::ContextMismatch)
        ));
        assert!(matches!(
            NativeAmxSigningGuard::open(
                root.path(),
                base.authority_context_height,
                base.round.context_id,
                base.epoch + 1,
                base.network_id,
                signer.clone(),
                signing_guard_limits(8),
            ),
            Err(NativeAmxSigningGuardError::ContextMismatch)
        ));
        assert!(matches!(
            NativeAmxSigningGuard::open(
                root.path(),
                base.authority_context_height,
                base.round.context_id,
                base.epoch,
                network_id(b"same-height-foreign-genesis"),
                signer.clone(),
                signing_guard_limits(8),
            ),
            Err(NativeAmxSigningGuardError::ContextMismatch)
        ));
        let restarted = open_guard(root.path(), &base, signer.clone(), 8);
        let mut stale = base;
        set_source(&mut stale, 0xA2);
        stale.participant_settlement_commitment = stale
            .computed_grouped_participant_settlement_commitment(&[stale.source_id])
            .expect("single-source stale-view settlement is valid");
        stale.round.view += 1;
        assert_eq!(
            restarted.record_body_for_test(&stale),
            Err(NativeAmxSigningGuardError::StaleView {
                attempted_view: base.round.view + 1,
                highest_view: base.round.view + 2,
            })
        );
        drop(restarted);
        let next_context = another_context(b"next-height-context");
        let next_guard = NativeAmxSigningGuard::open(
            root.path(),
            base.authority_context_height + 1,
            next_context,
            base.epoch,
            base.network_id,
            signer.clone(),
            signing_guard_limits(8),
        )
        .expect("advance exact next height");
        let mut next = base;
        next.round.height += 1;
        next.round.context_id = next_context;
        next.round.view = 0;
        next.coordinator_lane_block_view = 0;
        next.authority_context_height += 1;
        next.planned_coordinator_block_height += 1;
        record_body(&next_guard, &next);
        drop(next_guard);
        assert!(matches!(
            open_signing_guard(root.path(), &next, signer.clone(), 16),
            Err(NativeAmxSigningGuardError::ContextMismatch)
        ));
        assert!(matches!(
            open_signing_guard(root.path(), &base, signer, 8),
            Err(NativeAmxSigningGuardError::HeightRegression { .. })
        ));
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_detects_plain_deletion_of_anchored_record() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x74);
        let body = body(NativeAmxPhase::Commit);
        let guard = open_guard(root.path(), &body, signer.clone(), 8);
        record_body(&guard, &body);
        let path = signing_record_path(&guard);
        drop(guard);
        fs::remove_file(&path).expect("delete anchored record");
        assert_unsafe_open(root.path(), &body, signer, 8);
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_detects_live_anchor_and_record_deletion_before_another_append() {
        for delete_anchor in [false, true] {
            let root = tempfile::tempdir().expect("temp dir");
            let (_keypair, signer) = signing_guard_signer(if delete_anchor { 0x87 } else { 0x86 });
            let first = body(NativeAmxPhase::Commit);
            let guard = open_guard(root.path(), &first, signer, 8);
            record_body(&guard, &first);
            let deleted_path = if delete_anchor {
                NativeAmxSigningGuard::anchor_path(&guard.directory)
            } else {
                signing_record_path(&guard)
            };
            fs::remove_file(&deleted_path).expect("delete live retained journal path");
            let mut second = first;
            set_source(&mut second, 0xD1);
            assert!(matches!(
                guard.record_body_for_test(&second),
                Err(NativeAmxSigningGuardError::UnsafeJournal(_))
            ));
            assert!(matches!(
                guard.record_body_for_test(&second),
                Err(NativeAmxSigningGuardError::Poisoned(_))
            ));
        }
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_detects_live_anchor_and_record_replacement_before_another_append() {
        for replace_anchor in [false, true] {
            let root = tempfile::tempdir().expect("temp dir");
            let (_keypair, signer) = signing_guard_signer(if replace_anchor { 0x89 } else { 0x88 });
            let first = body(NativeAmxPhase::Commit);
            let guard = open_guard(root.path(), &first, signer, 8);
            record_body(&guard, &first);
            let replaced_path = if replace_anchor {
                NativeAmxSigningGuard::anchor_path(&guard.directory)
            } else {
                signing_record_path(&guard)
            };
            let bytes = fs::read(&replaced_path).expect("read retained journal path");
            let replacement = guard.directory.join(if replace_anchor {
                "replacement-anchor"
            } else {
                "replacement-record"
            });
            write_secure_new(&replacement, &bytes);
            fs::rename(&replacement, &replaced_path).expect("replace retained journal path");
            let mut second = first;
            set_source(&mut second, 0xD2);
            assert!(matches!(
                guard.record_body_for_test(&second),
                Err(NativeAmxSigningGuardError::UnsafeJournal(_))
            ));
        }
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_rejects_anchor_deletion_or_wrong_v5_anchor_version() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x83);
        let body = body(NativeAmxPhase::Commit);
        let guard = open_guard(root.path(), &body, signer.clone(), 8);
        record_body(&guard, &body);
        let anchor_path = NativeAmxSigningGuard::anchor_path(&guard.directory);
        drop(guard);
        fs::remove_file(anchor_path).expect("delete chain anchor");
        assert_unsafe_open(root.path(), &body, signer, 8);
        let version_root = tempfile::tempdir().expect("wrong-version temp dir");
        let (_keypair, version_signer) = signing_guard_signer(0x93);
        let version_guard = open_guard(version_root.path(), &body, version_signer.clone(), 8);
        record_body(&version_guard, &body);
        let version_anchor_path = NativeAmxSigningGuard::anchor_path(&version_guard.directory);
        let version_anchor_bytes =
            fs::read(&version_anchor_path).expect("read canonical V5 anchor");
        let mut wrong_version_anchor =
            norito::decode_canonical::<NativeAmxSigningAnchorV2>(&version_anchor_bytes)
                .expect("decode canonical V5 anchor");
        wrong_version_anchor.version = NATIVE_AMX_SIGNING_GUARD_VERSION.saturating_sub(1);
        drop(version_guard);
        fs::write(
            &version_anchor_path,
            norito::encode_canonical(&wrong_version_anchor)
                .expect("encode wrong-version V5 anchor"),
        )
        .expect("replace anchor with wrong-version bytes");
        assert_unsafe_open(version_root.path(), &body, version_signer, 8);
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_detects_hardlink_move_of_anchored_record() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x75);
        let body = body(NativeAmxPhase::Commit);
        let guard = open_guard(root.path(), &body, signer.clone(), 8);
        record_body(&guard, &body);
        let path = signing_record_path(&guard);
        drop(guard);
        let escaped = root.path().join("escaped-record.norito");
        fs::hard_link(&path, &escaped).expect("hardlink record outside signer journal");
        fs::remove_file(&path).expect("unlink anchored journal path");
        assert_unsafe_open(root.path(), &body, signer, 8);
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_rejects_wrong_version_noncanonical_and_hardlinked_records() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x76);
        let body = body(NativeAmxPhase::Commit);
        let guard = open_guard(root.path(), &body, signer.clone(), 8);
        record_body(&guard, &body);
        let path = signing_record_path(&guard);
        drop(guard);
        let bytes = fs::read(&path).expect("read record");
        let record =
            norito::decode_from_bytes::<NativeAmxSigningRecordV2>(&bytes).expect("decode record");
        let mut wrong_version_record = record.clone();
        wrong_version_record.version = NATIVE_AMX_SIGNING_GUARD_VERSION.saturating_sub(1);
        fs::write(
            &path,
            norito::encode_canonical(&wrong_version_record)
                .expect("encode wrong-version V5 record"),
        )
        .expect("replace with wrong-version V5 record");
        assert_unsafe_open(root.path(), &body, signer.clone(), 8);
        fs::write(&path, &bytes).expect("restore canonical V5 record");
        fs::write(&path, record.encode()).expect("replace with bare Norito");
        assert_unsafe_open(root.path(), &body, signer.clone(), 8);
        fs::write(&path, bytes).expect("restore framed record");
        let escaped = root.path().join("record-hardlink");
        fs::hard_link(&path, &escaped).expect("create hardlink");
        assert_unsafe_open(root.path(), &body, signer, 8);
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_reconciles_only_one_unpublished_tail() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x77);
        let base = body(NativeAmxPhase::Commit);
        let guard = open_guard(root.path(), &base, signer.clone(), 8);
        record_body(&guard, &base);
        let anchor = guard.inner.lock().anchor.clone();
        let mut tail_body = base;
        set_source(&mut tail_body, 0xB1);
        let tail = NativeAmxSigningRecordV2::from_body(
            anchor.record_count + 1,
            anchor.head_hash,
            &tail_body,
            &signer,
        )
        .expect("build unpublished tail");
        let tail_path = NativeAmxSigningGuard::record_path(&guard.directory, &tail);
        write_secure_new(
            &tail_path,
            &norito::to_bytes(&tail).expect("encode unpublished tail"),
        );
        drop(guard);
        let restarted = open_guard(root.path(), &base, signer, 8);
        assert!(!tail_path.exists());
        assert_eq!(restarted.inner.lock().anchor.record_count, 1);
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_discards_crash_left_anchor_temp_without_losing_committed_head() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x8B);
        let base = body(NativeAmxPhase::Commit);
        let guard = open_guard(root.path(), &base, signer.clone(), 8);
        record_body(&guard, &base);
        let committed_anchor = guard.inner.lock().anchor.clone();
        let temp_path = NativeAmxSigningGuard::anchor_temp_path(&guard.directory);
        write_secure_new(
            &temp_path,
            &norito::to_bytes(&committed_anchor).expect("encode crash-left anchor temp"),
        );
        drop(guard);
        let restarted = open_guard(root.path(), &base, signer, 8);
        assert!(!temp_path.exists());
        assert_eq!(restarted.inner.lock().anchor, committed_anchor);
        record_body(&restarted, &base);
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_finishes_height_transition_after_anchor_publish_crash() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x8C);
        let base = body(NativeAmxPhase::Commit);
        let guard = open_guard(root.path(), &base, signer.clone(), 8);
        record_body(&guard, &base);
        let old_record = signing_record_path(&guard);
        let next_context = another_context(b"crash-boundary-next-height-context");
        let next_binding = NativeAmxHeightBindingV2 {
            active_height: base.authority_context_height + 1,
            context_id: next_context,
            epoch: base.epoch,
            network_id: base.network_id,
            signer: signer.clone(),
            max_records: 8,
        };
        let next_anchor =
            NativeAmxSigningAnchorV2::empty(next_binding).expect("build next-height empty anchor");
        NativeAmxSigningGuard::persist_anchor(
            &guard.directory,
            &guard.directory_handle,
            guard.owner_uid,
            &next_anchor,
            guard.limits.max_anchor_bytes.get(),
        )
        .expect("publish next-height anchor before simulated crash");
        drop(guard);
        let mut next = base;
        next.round.height += 1;
        next.round.context_id = next_context;
        next.round.view = 0;
        next.coordinator_lane_block_view = 0;
        next.authority_context_height += 1;
        next.planned_coordinator_block_height += 1;
        let restarted = open_guard(root.path(), &next, signer, 8);
        assert!(!old_record.exists());
        record_body(&restarted, &next);
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_rejects_multiple_unpublished_tails() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x78);
        let base = body(NativeAmxPhase::Commit);
        let guard = open_guard(root.path(), &base, signer.clone(), 8);
        let anchor = guard.inner.lock().anchor.clone();
        let mut first_body = base;
        set_source(&mut first_body, 0xB2);
        let first = NativeAmxSigningRecordV2::from_body(1, anchor.head_hash, &first_body, &signer)
            .expect("first tail");
        let mut second_body = base;
        set_source(&mut second_body, 0xB3);
        let second =
            NativeAmxSigningRecordV2::from_body(2, first.record_hash, &second_body, &signer)
                .expect("second tail");
        for record in [&first, &second] {
            let path = NativeAmxSigningGuard::record_path(&guard.directory, record);
            write_secure_new(&path, &norito::to_bytes(record).expect("encode tail"));
        }
        drop(guard);
        assert_unsafe_open(root.path(), &base, signer, 8);
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_latches_poison_after_lock_path_deletion() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x79);
        let body = body(NativeAmxPhase::Prepare);
        let guard = open_guard(root.path(), &body, signer, 8);
        fs::remove_file(&guard.lock_path).expect("delete retained lock path");
        assert!(matches!(
            guard.record_body_for_test(&body),
            Err(NativeAmxSigningGuardError::UnsafeJournal(_))
        ));
        assert!(matches!(
            guard.record_body_for_test(&body),
            Err(NativeAmxSigningGuardError::Poisoned(_))
        ));
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_latches_poison_after_directory_or_lock_replacement() {
        for replace_directory in [false, true] {
            let root = tempfile::tempdir().expect("temp dir");
            let (_keypair, signer) =
                signing_guard_signer(if replace_directory { 0x8C } else { 0x8B });
            let body = body(NativeAmxPhase::Prepare);
            let guard = open_guard(root.path(), &body, signer, 8);
            if replace_directory {
                let moved = root.path().join("moved-signer-directory");
                fs::rename(&guard.directory, moved).expect("move retained signer directory");
                let mut builder = DirBuilder::new();
                builder.mode(NATIVE_AMX_SIGNING_DIRECTORY_MODE);
                builder
                    .create(&guard.directory)
                    .expect("create replacement signer directory");
            } else {
                let replacement = guard.directory.join("replacement-owner-lock");
                write_secure_new(&replacement, b"");
                fs::rename(replacement, &guard.lock_path).expect("replace owner lock path");
            }
            assert!(matches!(
                guard.record_body_for_test(&body),
                Err(NativeAmxSigningGuardError::UnsafeJournal(_))
            ));
            assert!(matches!(
                guard.record_body_for_test(&body),
                Err(NativeAmxSigningGuardError::Poisoned(_))
            ));
        }
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_rejects_malformed_context_and_view_bodies() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x80);
        let body = body(NativeAmxPhase::Prepare);
        let guard = open_guard(root.path(), &body, signer, 8);
        let mut foreign_context = body;
        foreign_context.round.context_id =
            HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(Hash::new(
                b"foreign-signing-guard-context",
            )));
        assert_eq!(
            guard.record_body_for_test(&foreign_context),
            Err(NativeAmxSigningGuardError::ContextMismatch)
        );
        let mut zero_source = body;
        zero_source.source_id = [0; Hash::LENGTH];
        assert!(matches!(
            guard.record_body_for_test(&zero_source),
            Err(NativeAmxSigningGuardError::InvalidInput(_))
        ));
        let mut zero_planned_height = body;
        zero_planned_height.planned_coordinator_block_height = 0;
        assert!(matches!(
            guard.record_body_for_test(&zero_planned_height),
            Err(NativeAmxSigningGuardError::InvalidInput(_))
        ));
        record_body(&guard, &body);
        let mut entrypoint_drift = body;
        entrypoint_drift.phase = NativeAmxPhase::Commit;
        entrypoint_drift.tx_entrypoint_hash =
            HashOf::<TransactionEntrypoint>::from_untyped_unchecked(Hash::new(
                b"conflicting-signing-guard-entrypoint",
            ));
        assert_eq!(
            guard.record_body_for_test(&entrypoint_drift),
            Err(NativeAmxSigningGuardError::PlanEquivocation)
        );
        let mut mismatched_view = body;
        mismatched_view.coordinator_lane_block_view += 1;
        assert_eq!(
            guard.record_body_for_test(&mismatched_view),
            Err(NativeAmxSigningGuardError::Equivocation)
        );
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_rejects_unknown_and_symlink_temps() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x81);
        let body = body(NativeAmxPhase::Commit);
        let guard = open_guard(root.path(), &body, signer.clone(), 8);
        guard.record_body_for_test(&body).expect("record body");
        let record_path = signing_record_path(&guard);
        let directory = guard.directory.clone();
        drop(guard);
        let unknown = directory.join("unknown.tmp");
        write_secure_new(&unknown, b"unknown");
        assert_unsafe_open(root.path(), &body, signer.clone(), 8);
        fs::remove_file(&unknown).expect("remove unknown temp");
        let temp_link = record_path.with_extension(NATIVE_AMX_SIGNING_GUARD_TEMP_EXTENSION);
        symlink(&record_path, &temp_link).expect("create known-name temp symlink");
        assert_unsafe_open(root.path(), &body, signer, 8);
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_fails_closed_on_injected_future_record() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x82);
        let current = body(NativeAmxPhase::Commit);
        let guard = open_guard(root.path(), &current, signer.clone(), 8);
        let anchor = guard.inner.lock().anchor.clone();
        let mut future = current;
        future.round.height += 1;
        future.authority_context_height += 1;
        future.planned_coordinator_block_height += 1;
        let record = NativeAmxSigningRecordV2::from_body(1, anchor.head_hash, &future, &signer)
            .expect("future record");
        let path = NativeAmxSigningGuard::record_path(&guard.directory, &record);
        write_secure_new(
            &path,
            &norito::to_bytes(&record).expect("encode future record"),
        );
        drop(guard);
        assert_eq!(
            open_signing_guard(root.path(), &current, signer, 8)
                .expect_err("future record must fail closed"),
            NativeAmxSigningGuardError::FutureHeight {
                record_height: future.authority_context_height,
                active_height: current.authority_context_height,
            }
        );
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_enforces_configured_and_protocol_capacity() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x7A);
        let first = body(NativeAmxPhase::Prepare);
        let guard = open_guard(root.path(), &first, signer.clone(), 1);
        record_body(&guard, &first);
        let mut second = first;
        set_source(&mut second, 0xCE);
        assert_eq!(
            guard.record_body_for_test(&second),
            Err(NativeAmxSigningGuardError::Capacity)
        );
        drop(guard);
        let _ = signer;
        assert!(matches!(
            NativeAmxSigningGuardLimits::new(
                signing_guard_capacity(MAX_NATIVE_AMX_SIGNING_GUARD_RECORDS_HARD + 1),
                iroha_config::parameters::defaults::sumeragi::V2_NATIVE_AMX_SIGNING_GUARD_RECORD_BYTES,
                iroha_config::parameters::defaults::sumeragi::V2_NATIVE_AMX_SIGNING_GUARD_ANCHOR_BYTES,
            ),
            Err(NativeAmxSigningGuardError::InvalidInput(_))
        ));
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_uses_signer_specific_journals_for_key_rotation() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_first_keypair, first_signer) = signing_guard_signer(0x7B);
        let (_second_keypair, second_signer) = signing_guard_signer(0x7C);
        let body = body(NativeAmxPhase::Commit);
        let first = open_guard(root.path(), &body, first_signer.clone(), 8);
        record_body(&first, &body);
        let first_directory = first.directory.clone();
        drop(first);
        let second = open_guard(root.path(), &body, second_signer, 8);
        record_body(&second, &body);
        assert_ne!(first_directory, second.directory);
        drop(second);
        let first_restarted = open_guard(root.path(), &body, first_signer, 8);
        let mut conflict = body;
        conflict.coordinator_proposal_hash = Hash::new(b"first signer conflict");
        assert_eq!(
            first_restarted.record_body_for_test(&conflict),
            Err(NativeAmxSigningGuardError::Equivocation)
        );
    }
    #[cfg(unix)]
    #[test]
    fn corrupted_retired_signer_journal_does_not_brick_rotated_signer() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_first_keypair, first_signer) = signing_guard_signer(0x84);
        let (_second_keypair, second_signer) = signing_guard_signer(0x85);
        let body = body(NativeAmxPhase::Commit);
        let first = open_guard(root.path(), &body, first_signer.clone(), 8);
        record_body(&first, &body);
        let first_record = signing_record_path(&first);
        drop(first);
        fs::remove_file(first_record).expect("corrupt retired signer journal");
        let second = open_guard(root.path(), &body, second_signer, 8);
        record_body(&second, &body);
        drop(second);
        assert_unsafe_open(root.path(), &body, first_signer, 8);
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_sets_strict_directory_and_file_modes() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x7D);
        let body = body(NativeAmxPhase::Commit);
        let guard = open_guard(root.path(), &body, signer, 8);
        guard.record_body_for_test(&body).expect("record body");
        assert_eq!(
            fs::symlink_metadata(&guard.directory)
                .expect("directory metadata")
                .mode()
                & 0o777,
            NATIVE_AMX_SIGNING_DIRECTORY_MODE
        );
        for path in signing_record_paths(&guard).into_iter().chain([
            guard.lock_path.clone(),
            NativeAmxSigningGuard::anchor_path(&guard.directory),
        ]) {
            assert_eq!(
                fs::symlink_metadata(path).expect("file metadata").mode() & 0o777,
                NATIVE_AMX_SIGNING_FILE_MODE
            );
        }
    }
    #[cfg(unix)]
    #[test]
    fn signing_guard_rejects_foreign_uid_for_every_trusted_path_class() {
        let root = tempfile::tempdir().expect("temp dir");
        let (_keypair, signer) = signing_guard_signer(0x8A);
        let body = body(NativeAmxPhase::Commit);
        let guard = open_guard(root.path(), &body, signer, 8);
        guard.record_body_for_test(&body).expect("record body");
        assert_eq!(
            native_amx_effective_user_id(root.path()).expect("probe effective UID"),
            guard.owner_uid
        );
        let wrong_uid = guard.owner_uid ^ 1;
        let root_metadata = fs::symlink_metadata(root.path()).expect("store root metadata");
        assert_eq!(root_metadata.uid(), guard.owner_uid);
        assert!(matches!(
            native_amx_validate_uid(root.path(), &root_metadata, wrong_uid),
            Err(NativeAmxSigningGuardError::UnsafeJournal(_))
        ));
        let directory_metadata =
            fs::symlink_metadata(&guard.directory).expect("signer directory metadata");
        assert!(matches!(
            native_amx_validate_uid(&guard.directory, &directory_metadata, wrong_uid),
            Err(NativeAmxSigningGuardError::UnsafeJournal(_))
        ));
        for path in signing_record_paths(&guard).into_iter().chain([
            guard.lock_path.clone(),
            NativeAmxSigningGuard::anchor_path(&guard.directory),
        ]) {
            let metadata = fs::symlink_metadata(&path).expect("trusted file metadata");
            assert!(matches!(
                native_amx_validate_uid(&path, &metadata, wrong_uid),
                Err(NativeAmxSigningGuardError::UnsafeJournal(_))
            ));
        }
    }
    fn body_for_validator_set(
        phase: NativeAmxPhase,
        validator_set: &[PeerId],
    ) -> NativeAmxAttestationBodyV2 {
        let mut body = body(phase);
        body.participant_validator_set_hash = HashOf::new(&validator_set.to_vec());
        body.participant_validator_count =
            u32::try_from(validator_set.len()).expect("fixture validator count fits u32");
        body.participant_min_quorum = u32::try_from(
            crate::sumeragi::network_topology::commit_quorum_from_len(validator_set.len()).max(1),
        )
        .expect("fixture validator quorum fits u32");
        body
    }
    fn aligned_pops(validator_set: &[PeerId], keypairs: &[KeyPair]) -> Vec<Vec<u8>> {
        validator_set
            .iter()
            .map(|validator| {
                let keypair = keypairs
                    .iter()
                    .find(|keypair| keypair.public_key() == validator.public_key())
                    .expect("fixture validator has key material");
                iroha_crypto::bls_normal_pop_prove(keypair.private_key())
                    .expect("prove fixture PoP")
            })
            .collect()
    }
    fn full_plan_request(
        mut body: NativeAmxAttestationBodyV2,
        coordinator_validator_set: Vec<PeerId>,
    ) -> NativeAmxAttestationRequestV2 {
        let coordinator =
            RoutingDecision::new(body.coordinator_lane_id, body.coordinator_dataspace_id);
        let participant =
            RoutingDecision::new(body.participant_lane_id, body.participant_dataspace_id);
        let routing_plan = RoutingPlan::native_amx(
            coordinator,
            vec![RouteLeg::new(participant, RouteLegRole::Participant)],
        );
        body.plan_digest = routing_plan.digest();
        let validator_count = u32::try_from(coordinator_validator_set.len())
            .expect("fixture coordinator validator count fits u32");
        let min_quorum = u32::try_from(
            crate::sumeragi::network_topology::commit_quorum_from_len(
                coordinator_validator_set.len(),
            )
            .max(1),
        )
        .expect("fixture coordinator quorum fits u32");
        let participant_validator_set = coordinator_validator_set.clone();
        let mut descriptor = iroha_data_model::block::consensus::LaneBlockDescriptorV1 {
            lane_id: body.coordinator_lane_id,
            dataspace_id: body.coordinator_dataspace_id,
            lane_incarnation: body.coordinator_lane_incarnation,
            proposal_height: body.authority_context_height,
            previous_lane_block_height: body.planned_coordinator_block_height.saturating_sub(1),
            previous_lane_block_descriptor_hash: (body.planned_coordinator_block_height > 1)
                .then(|| Hash::new(b"native-amx-v2-test-previous-descriptor")),
            lane_block_height: body.planned_coordinator_block_height,
            lane_block_view: body.coordinator_lane_block_view,
            subject_hash: Hash::new(b"native-amx-v2-test-subject"),
            payload_ownership_hash: Hash::new(b"native-amx-v2-test-ownership"),
            rbc_instance_hash: Hash::new(b"native-amx-v2-test-rbc"),
            accepted_candidate_indices: vec![0],
            accepted_transaction_hashes: vec![Hash::from(body.tx_entrypoint_hash)],
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set_hash: HashOf::new(&coordinator_validator_set),
            validator_set: coordinator_validator_set,
            validator_count,
            min_quorum,
            qc_mode_tag: "permissioned:native-amx-v2-test".to_owned(),
            descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
        };
        descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
        let mut coordinator_proposal = LaneBlockProposalV1 {
            descriptor,
            proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
            payload_block_hint: None,
        };
        coordinator_proposal.proposal_hash = coordinator_proposal.computed_proposal_hash();
        body.coordinator_proposal_hash = coordinator_proposal.proposal_hash;
        let mut participant_descriptor =
            iroha_data_model::block::consensus::LaneBlockDescriptorV1 {
                lane_id: body.participant_lane_id,
                dataspace_id: body.participant_dataspace_id,
                lane_incarnation: body.participant_lane_incarnation,
                proposal_height: body.authority_context_height,
                previous_lane_block_height: body.participant_previous_block_height,
                previous_lane_block_descriptor_hash: body
                    .participant_previous_block_descriptor_hash,
                lane_block_height: body.participant_lane_block_height,
                lane_block_view: body.participant_lane_block_view,
                subject_hash: Hash::new(b"native-amx-v2-test-participant-subject"),
                payload_ownership_hash: Hash::new(b"native-amx-v2-test-participant-ownership"),
                rbc_instance_hash: Hash::new(b"native-amx-v2-test-participant-rbc"),
                accepted_candidate_indices: vec![0],
                accepted_transaction_hashes: vec![Hash::from(body.tx_entrypoint_hash)],
                validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
                validator_set_hash: HashOf::new(&participant_validator_set),
                validator_set: participant_validator_set,
                validator_count: body.participant_validator_count,
                min_quorum: body.participant_min_quorum,
                qc_mode_tag: "permissioned:native-amx-v2-test".to_owned(),
                descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
            };
        participant_descriptor.descriptor_hash = participant_descriptor.computed_descriptor_hash();
        let mut participant_proposal = LaneBlockProposalV1 {
            descriptor: participant_descriptor,
            proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
            payload_block_hint: None,
        };
        participant_proposal.proposal_hash = participant_proposal.computed_proposal_hash();
        body.participant_proposal_hash = participant_proposal.proposal_hash;
        let participant_settlement = body
            .computed_grouped_participant_settlement(&[body.source_id])
            .expect("single-source test fixture settlement is valid");
        body.participant_settlement_commitment = Hash::from(
            iroha_data_model::nexus::compute_settlement_hash(&participant_settlement)
                .expect("fixture participant settlement hashes"),
        );
        NativeAmxAttestationRequestV2 {
            body,
            plan_legs: routing_plan.legs(),
            coordinator_proposal,
            participant_proposal,
            participant_settlement,
        }
    }
    fn vote(phase: NativeAmxPhase) -> NativeAmxVoteV2 {
        let keypair = checked_random_ed25519_keypair();
        NativeAmxVoteV2 {
            body: body(phase),
            signer: PeerId::new(keypair.public_key().clone()),
            bls_signature: vec![0xA5; 96],
        }
    }
    #[test]
    fn session_cache_rejects_duplicate_signer() {
        let mut cache = NativeAmxSessionCache::new(NonZeroUsize::new(4).expect("nonzero"));
        let vote = vote(NativeAmxPhase::Prepare);
        cache
            .insert_vote(vote.clone())
            .expect("first vote should insert");
        assert!(matches!(
            cache.insert_vote(vote),
            Err(NativeAmxSessionError::DuplicateSigner)
        ));
    }
    #[test]
    fn session_cache_rejects_live_source_plan_equivocation() {
        let mut cache = NativeAmxSessionCache::new(NonZeroUsize::new(4).expect("nonzero"));
        let first = vote(NativeAmxPhase::Prepare);
        cache.insert_vote(first.clone()).expect("first plan claim");
        let mut equivocation = first;
        equivocation.body.plan_digest = Hash::new(b"equivocating-native-amx-plan");
        assert_eq!(
            cache.insert_vote(equivocation),
            Err(NativeAmxSessionError::PlanEquivocation)
        );
    }
    #[test]
    fn full_plan_request_binds_canonical_routes_and_coordinator_proposal() {
        let keypair = checked_bls_keypair(0x77);
        let validators = vec![PeerId::new(keypair.public_key().clone())];
        let request = full_plan_request(
            body_for_validator_set(NativeAmxPhase::Prepare, &validators),
            validators,
        );
        assert_eq!(request.validate_plan_binding(), Ok(()));
        let mut coordinator_participates = request.clone();
        coordinator_participates.body.participant_lane_id =
            coordinator_participates.body.coordinator_lane_id;
        coordinator_participates.body.participant_dataspace_id =
            coordinator_participates.body.coordinator_dataspace_id;
        coordinator_participates.body.participant_lane_incarnation =
            coordinator_participates.body.coordinator_lane_incarnation;
        coordinator_participates
            .body
            .participant_previous_block_height = coordinator_participates
            .coordinator_proposal
            .descriptor
            .previous_lane_block_height;
        coordinator_participates
            .body
            .participant_previous_block_descriptor_hash = coordinator_participates
            .coordinator_proposal
            .descriptor
            .previous_lane_block_descriptor_hash;
        coordinator_participates.body.participant_lane_block_height = coordinator_participates
            .coordinator_proposal
            .descriptor
            .lane_block_height;
        coordinator_participates.body.participant_lane_block_view = coordinator_participates
            .coordinator_proposal
            .descriptor
            .lane_block_view;
        coordinator_participates.participant_proposal =
            coordinator_participates.coordinator_proposal.clone();
        coordinator_participates.body.participant_proposal_hash =
            coordinator_participates.participant_proposal.proposal_hash;
        let coordinator_route = RoutingDecision::new(
            coordinator_participates.body.coordinator_lane_id,
            coordinator_participates.body.coordinator_dataspace_id,
        );
        let overlapping_plan = RoutingPlan::native_amx(
            coordinator_route,
            vec![RouteLeg::new(coordinator_route, RouteLegRole::Participant)],
        );
        coordinator_participates.body.plan_digest = overlapping_plan.digest();
        coordinator_participates.plan_legs = overlapping_plan.legs();
        coordinator_participates.participant_settlement = coordinator_participates
            .body
            .computed_grouped_participant_settlement(&[coordinator_participates.body.source_id])
            .expect("single-source test fixture settlement is valid");
        coordinator_participates
            .body
            .participant_settlement_commitment = Hash::from(
            iroha_data_model::nexus::compute_settlement_hash(
                &coordinator_participates.participant_settlement,
            )
            .expect("overlapping participant settlement hashes"),
        );
        assert_eq!(
            coordinator_participates.validate_plan_binding(),
            Ok(()),
            "the coordinator route may also own one participant leg"
        );
        let mut stale_same_route = coordinator_participates.clone();
        let stale_incarnation = Hash::new(b"stale same-route participant incarnation");
        stale_same_route.body.participant_lane_incarnation = stale_incarnation;
        stale_same_route
            .participant_proposal
            .descriptor
            .lane_incarnation = stale_incarnation;
        stale_same_route
            .participant_proposal
            .descriptor
            .descriptor_hash = stale_same_route
            .participant_proposal
            .descriptor
            .computed_descriptor_hash();
        stale_same_route.participant_proposal.proposal_hash = stale_same_route
            .participant_proposal
            .computed_proposal_hash();
        stale_same_route.body.participant_proposal_hash =
            stale_same_route.participant_proposal.proposal_hash;
        stale_same_route.participant_settlement = stale_same_route
            .body
            .computed_grouped_participant_settlement(&[stale_same_route.body.source_id])
            .expect("stale same-route settlement fixture remains structurally valid");
        stale_same_route.body.participant_settlement_commitment = Hash::from(
            iroha_data_model::nexus::compute_settlement_hash(
                &stale_same_route.participant_settlement,
            )
            .expect("stale same-route settlement hashes"),
        );
        assert_eq!(
            stale_same_route.validate_plan_binding(),
            Err(NativeAmxRequestError::ParticipantProposalMismatch),
            "a same-route participant cannot drift to another lane incarnation"
        );
        let mut omitted_participant = request.clone();
        omitted_participant.plan_legs.truncate(1);
        assert_eq!(
            omitted_participant.validate_plan_binding(),
            Err(NativeAmxRequestError::IncompletePlan)
        );
        let mut substituted_proposal = request;
        substituted_proposal.body.coordinator_proposal_hash =
            Hash::new(b"substituted-native-amx-coordinator-proposal");
        assert_eq!(
            substituted_proposal.validate_plan_binding(),
            Err(NativeAmxRequestError::CoordinatorProposalMismatch)
        );
    }
    #[test]
    fn session_cache_allows_same_signer_for_retried_body() {
        let mut cache = NativeAmxSessionCache::new(NonZeroUsize::new(4).expect("nonzero"));
        let vote = vote(NativeAmxPhase::Prepare);
        let key = NativeAmxSessionKey::from_body(&vote.body);
        let mut retried_vote = vote.clone();
        retried_vote.body.planned_coordinator_block_height = retried_vote
            .body
            .planned_coordinator_block_height
            .saturating_add(1);
        cache.insert_vote(vote.clone()).expect("first body vote");
        cache
            .insert_vote(retried_vote.clone())
            .expect("same signer may vote on a retried body");
        assert_eq!(cache.sorted_votes_for_body(key, &vote.body), vec![vote]);
        assert_eq!(
            cache.sorted_votes_for_body(key, &retried_vote.body),
            vec![retried_vote]
        );
        assert_eq!(cache.sorted_votes(key, NativeAmxPhase::Prepare).len(), 2);
    }
    #[test]
    fn session_cache_allows_same_signer_for_different_participant_legs() {
        let mut cache = NativeAmxSessionCache::new(NonZeroUsize::new(4).expect("nonzero"));
        let vote = vote(NativeAmxPhase::Prepare);
        let key = NativeAmxSessionKey::from_body(&vote.body);
        let mut other_leg = vote.clone();
        other_leg.body.participant_lane_id = LaneId::new(9);
        other_leg.body.participant_dataspace_id = DataSpaceId::new(10);
        cache.insert_vote(vote.clone()).expect("first leg vote");
        cache
            .insert_vote(other_leg.clone())
            .expect("same signer may vote on another participant leg");
        assert_eq!(cache.sorted_votes_for_body(key, &vote.body), vec![vote]);
        assert_eq!(
            cache.sorted_votes_for_body(key, &other_leg.body),
            vec![other_leg]
        );
    }
    #[test]
    fn session_cache_filters_exact_body_votes_to_validator_set() {
        let mut cache = NativeAmxSessionCache::new(NonZeroUsize::new(4).expect("nonzero"));
        let allowed_keypair = KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
            .expect("generate checked allowed native AMX BLS fixture keypair");
        let unknown_keypair = KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
            .expect("generate checked unknown native AMX BLS fixture keypair");
        let allowed = PeerId::new(allowed_keypair.public_key().clone());
        let unknown = PeerId::new(unknown_keypair.public_key().clone());
        let body = body(NativeAmxPhase::Prepare);
        let allowed_vote = NativeAmxVoteV2 {
            body,
            signer: allowed.clone(),
            bls_signature: vec![1],
        };
        let unknown_vote = NativeAmxVoteV2 {
            body,
            signer: unknown,
            bls_signature: vec![2],
        };
        let key = NativeAmxSessionKey::from_body(&body);
        cache
            .insert_vote(allowed_vote.clone())
            .expect("allowed signer vote");
        cache
            .insert_vote(unknown_vote)
            .expect("unknown signer vote");
        assert_eq!(
            cache.sorted_votes_for_body_from(key, &body, &[allowed]),
            vec![allowed_vote]
        );
    }
    #[test]
    fn session_cache_capacity_does_not_evict_source_plan_claims() {
        let mut cache = NativeAmxSessionCache::new(NonZeroUsize::new(1).expect("nonzero"));
        let first = vote(NativeAmxPhase::Prepare);
        let first_key = NativeAmxSessionKey::from_body(&first.body);
        cache.insert_vote(first.clone()).expect("first vote");
        let mut second = vote(NativeAmxPhase::Prepare);
        second.body.source_id = [0xAC; iroha_crypto::Hash::LENGTH];
        let second_key = NativeAmxSessionKey::from_body(&second.body);
        assert_eq!(
            cache.insert_vote(second),
            Err(NativeAmxSessionError::Capacity)
        );
        assert_eq!(
            cache.sorted_votes(first_key, NativeAmxPhase::Prepare).len(),
            1
        );
        assert!(
            cache
                .sorted_votes(second_key, NativeAmxPhase::Prepare)
                .is_empty()
        );
        let mut conflicting_plan = first;
        conflicting_plan.body.plan_digest = Hash::new(b"claim must survive capacity failure");
        assert_eq!(
            cache.insert_vote(conflicting_plan),
            Err(NativeAmxSessionError::PlanEquivocation)
        );
    }
    #[test]
    fn session_cache_body_capacity_fails_without_fifo_eviction() {
        let mut cache = NativeAmxSessionCache::with_limits(
            NonZeroUsize::new(4).expect("nonzero sessions"),
            NonZeroUsize::new(2).expect("nonzero body buckets"),
        );
        let first = vote(NativeAmxPhase::Prepare);
        let key = NativeAmxSessionKey::from_body(&first.body);
        let mut second = first.clone();
        second.body.planned_coordinator_block_height = 43;
        let mut third = first.clone();
        third.body.planned_coordinator_block_height = 44;
        cache.insert_vote(first.clone()).expect("first vote");
        cache.insert_vote(second.clone()).expect("second vote");
        assert_eq!(
            cache.insert_vote(third.clone()),
            Err(NativeAmxSessionError::Capacity)
        );
        assert_eq!(cache.sorted_votes_for_body(key, &first.body), vec![first]);
        assert_eq!(cache.sorted_votes_for_body(key, &second.body), vec![second]);
        assert!(cache.sorted_votes_for_body(key, &third.body).is_empty());
        assert_eq!(cache.sorted_votes(key, NativeAmxPhase::Prepare).len(), 2);
    }
    #[test]
    fn session_cache_certified_view_supersedes_stale_body_buckets() {
        let mut cache = NativeAmxSessionCache::with_limits(
            NonZeroUsize::new(1).expect("nonzero sessions"),
            NonZeroUsize::new(1).expect("nonzero body buckets"),
        );
        let stale = vote(NativeAmxPhase::Prepare);
        let key = NativeAmxSessionKey::from_body(&stale.body);
        cache.insert_vote(stale.clone()).expect("stale view vote");
        cache.retain_view(stale.body.round.view.saturating_add(1));
        assert!(cache.sorted_votes_for_body(key, &stale.body).is_empty());

        let mut current = stale;
        current.body.round.view = current.body.round.view.saturating_add(1);
        cache
            .insert_vote(current.clone())
            .expect("superseded bucket releases exact session capacity");
        assert_eq!(
            cache.sorted_votes_for_body(key, &current.body),
            vec![current]
        );
    }
    fn signed_vote(body: &NativeAmxAttestationBodyV2, keypair: &KeyPair) -> NativeAmxVoteV2 {
        NativeAmxVoteV2 {
            body: *body,
            signer: PeerId::new(keypair.public_key().clone()),
            bls_signature: checked_bls_signature_payload(keypair, &body.signature_preimage()),
        }
    }
    #[test]
    fn vote_ingress_validation_accepts_matching_signed_bls_vote() {
        let keypair = checked_bls_keypair(0xE1);
        let body = body(NativeAmxPhase::Prepare);
        let vote = signed_vote(&body, &keypair);
        let sender = vote.signer.clone();
        assert_eq!(
            vote.validate_ingress(NativeAmxPhase::Prepare, Some(&sender)),
            Ok(())
        );
    }
    #[test]
    fn vote_ingress_validation_rejects_phase_and_sender_mismatches() {
        let keypair = checked_bls_keypair(0xE2);
        let other_keypair = checked_bls_keypair(0xE3);
        let body = body(NativeAmxPhase::Prepare);
        let vote = signed_vote(&body, &keypair);
        let sender = vote.signer.clone();
        let other_sender = PeerId::new(other_keypair.public_key().clone());
        assert_eq!(
            vote.validate_ingress(NativeAmxPhase::Commit, Some(&sender)),
            Err(NativeAmxVoteIngressError::PhaseMismatch {
                expected: NativeAmxPhase::Commit,
                actual: NativeAmxPhase::Prepare
            })
        );
        assert_eq!(
            vote.validate_ingress(NativeAmxPhase::Prepare, Some(&other_sender)),
            Err(NativeAmxVoteIngressError::SenderMismatch)
        );
    }
    #[test]
    fn vote_ingress_validation_rejects_non_bls_and_bad_signatures() {
        let ed25519_keypair = checked_random_ed25519_keypair();
        let body = body(NativeAmxPhase::Commit);
        let ed25519_signature =
            Signature::try_new(ed25519_keypair.private_key(), &body.signature_preimage())
                .expect("checked Ed25519 fixture signature")
                .payload()
                .to_vec();
        let ed25519_vote = NativeAmxVoteV2 {
            body,
            signer: PeerId::new(ed25519_keypair.public_key().clone()),
            bls_signature: ed25519_signature,
        };
        assert_eq!(
            ed25519_vote.validate_ingress(NativeAmxPhase::Commit, None),
            Err(NativeAmxVoteIngressError::InvalidSignature),
            "the fixed-width signature gate runs before signer-algorithm inspection"
        );
        let mut non_bls_vote = ed25519_vote;
        non_bls_vote.bls_signature = vec![0_u8; NATIVE_AMX_BLS_PROOF_BYTES];
        assert_eq!(
            non_bls_vote.validate_ingress(NativeAmxPhase::Commit, None),
            Err(NativeAmxVoteIngressError::SignerNotBlsNormal)
        );
        let bls_keypair = checked_bls_keypair(0xE4);
        let mut bad_signature_vote = signed_vote(&body, &bls_keypair);
        bad_signature_vote.bls_signature = vec![0_u8; 96];
        assert_eq!(
            bad_signature_vote.validate_ingress_shape(NativeAmxPhase::Commit, None),
            Ok(()),
            "the cheap envelope gate must not parse attacker-controlled BLS bytes"
        );
        assert_eq!(
            bad_signature_vote.verify_signature(),
            Err(NativeAmxVoteIngressError::InvalidSignature)
        );
        assert_eq!(
            bad_signature_vote.validate_ingress(NativeAmxPhase::Commit, None),
            Err(NativeAmxVoteIngressError::InvalidSignature)
        );
    }
    #[test]
    fn aggregate_votes_to_qc_orders_votes_by_validator_set() {
        let keypairs = [
            checked_bls_keypair(0xA1),
            checked_bls_keypair(0xB2),
            checked_bls_keypair(0xC3),
        ];
        let mut validator_set = keypairs
            .iter()
            .map(|keypair| PeerId::new(keypair.public_key().clone()))
            .collect::<Vec<_>>();
        validator_set.sort();
        let body = body_for_validator_set(NativeAmxPhase::Commit, &validator_set);
        let validator_set_pops = aligned_pops(&validator_set, &keypairs);
        let votes = vec![
            signed_vote(&body, &keypairs[2]),
            signed_vote(&body, &keypairs[0]),
            signed_vote(&body, &keypairs[1]),
        ];
        let qc = aggregate_votes_to_qc(
            body,
            validator_set.clone(),
            validator_set_pops.clone(),
            &votes,
            3,
        )
        .expect("valid quorum should aggregate");
        assert_eq!(qc.body, body);
        assert_eq!(qc.validator_set(), validator_set.as_slice());
        assert_eq!(qc.validator_set_pops(), validator_set_pops.as_slice());
        let mut expected_bitmap = vec![0_u8; validator_set.len().div_ceil(8)];
        for keypair in [&keypairs[2], &keypairs[0], &keypairs[1]] {
            let signer = PeerId::new(keypair.public_key().clone());
            let index = validator_set
                .iter()
                .position(|validator| validator == &signer)
                .expect("vote signer belongs to fixture committee");
            expected_bitmap[index / 8] |= 1_u8 << (index % 8);
        }
        assert_eq!(qc.signers_bitmap, expected_bitmap);
        let individual_signatures = [
            signed_vote(&body, &keypairs[0]).bls_signature,
            signed_vote(&body, &keypairs[1]).bls_signature,
            signed_vote(&body, &keypairs[2]).bls_signature,
        ];
        let signature_refs = individual_signatures
            .iter()
            .map(Vec::as_slice)
            .collect::<Vec<_>>();
        let expected_aggregate = iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
            .expect("aggregate reference signatures");
        assert_eq!(qc.bls_aggregate_signature, expected_aggregate);
    }
    #[test]
    fn aggregate_votes_to_qc_rejects_bad_vote_sets() {
        let keypairs = [checked_bls_keypair(0xD1), checked_bls_keypair(0xD2)];
        let mut validator_set = keypairs
            .iter()
            .map(|keypair| PeerId::new(keypair.public_key().clone()))
            .collect::<Vec<_>>();
        validator_set.sort();
        let body = body_for_validator_set(NativeAmxPhase::Prepare, &validator_set);
        let validator_set_pops = aligned_pops(&validator_set, &keypairs);
        let vote = signed_vote(&body, &keypairs[0]);
        assert_eq!(
            aggregate_votes_to_qc(
                body,
                validator_set.clone(),
                validator_set_pops.clone(),
                &[],
                2,
            ),
            Err(NativeAmxQcBuildError::EmptyVotes)
        );
        assert_eq!(
            aggregate_votes_to_qc(
                body,
                validator_set.clone(),
                validator_set_pops.clone(),
                &[vote.clone()],
                2,
            ),
            Err(NativeAmxQcBuildError::QuorumNotMet)
        );
        assert_eq!(
            aggregate_votes_to_qc(
                body,
                validator_set.clone(),
                validator_set_pops.clone(),
                &[vote.clone(), vote.clone()],
                2
            ),
            Err(NativeAmxQcBuildError::DuplicateSigner)
        );
        let outsider = checked_bls_keypair(0xD3);
        assert_eq!(
            aggregate_votes_to_qc(
                body,
                validator_set.clone(),
                validator_set_pops.clone(),
                &[signed_vote(&body, &outsider)],
                2
            ),
            Err(NativeAmxQcBuildError::SignerNotInValidatorSet)
        );
        let ed25519_keypair = checked_random_ed25519_keypair();
        let ed25519_signer = PeerId::new(ed25519_keypair.public_key().clone());
        let ed25519_body = body_for_validator_set(
            NativeAmxPhase::Prepare,
            std::slice::from_ref(&ed25519_signer),
        );
        let ed25519_vote = NativeAmxVoteV2 {
            body: ed25519_body,
            signer: ed25519_signer.clone(),
            bls_signature: Signature::try_new(
                ed25519_keypair.private_key(),
                &ed25519_body.signature_preimage(),
            )
            .expect("checked Ed25519 fixture signature")
            .payload()
            .to_vec(),
        };
        assert_eq!(
            aggregate_votes_to_qc(
                ed25519_body,
                vec![ed25519_signer],
                vec![vec![0; NATIVE_AMX_BLS_PROOF_BYTES]],
                &[ed25519_vote],
                1,
            ),
            Err(NativeAmxQcBuildError::SignerNotBlsNormal)
        );
        let mut bad_signature_vote = vote.clone();
        bad_signature_vote.bls_signature = vec![0_u8; 96];
        assert_eq!(
            aggregate_votes_to_qc(
                body,
                validator_set.clone(),
                validator_set_pops.clone(),
                &[bad_signature_vote],
                2
            ),
            Err(NativeAmxQcBuildError::InvalidSignature)
        );
        let mut wrong_body_vote = vote;
        wrong_body_vote.body.phase = NativeAmxPhase::Commit;
        assert_eq!(
            aggregate_votes_to_qc(
                body,
                validator_set,
                validator_set_pops,
                &[wrong_body_vote],
                2,
            ),
            Err(NativeAmxQcBuildError::BodyMismatch)
        );
        let keypairs = [
            checked_bls_keypair(0xD4),
            checked_bls_keypair(0xD5),
            checked_bls_keypair(0xD6),
            checked_bls_keypair(0xD7),
        ];
        let mut validator_set = keypairs
            .iter()
            .map(|keypair| PeerId::new(keypair.public_key().clone()))
            .collect::<Vec<_>>();
        validator_set.sort();
        let mut lowered_body = body_for_validator_set(NativeAmxPhase::Prepare, &validator_set);
        lowered_body.participant_min_quorum = 2;
        let lowered_votes = keypairs
            .iter()
            .map(|keypair| signed_vote(&lowered_body, keypair))
            .collect::<Vec<_>>();
        assert_eq!(
            aggregate_votes_to_qc(
                lowered_body,
                validator_set.clone(),
                aligned_pops(&validator_set, &keypairs),
                &lowered_votes,
                2,
            ),
            Err(NativeAmxQcBuildError::InvalidValidatorSet),
            "a signed committee context must not lower the canonical threshold"
        );
        let canonical_body = body_for_validator_set(NativeAmxPhase::Prepare, &validator_set);
        let mut reversed = validator_set.clone();
        reversed.reverse();
        assert_eq!(
            aggregate_votes_to_qc(
                canonical_body,
                reversed,
                aligned_pops(&validator_set, &keypairs),
                &lowered_votes,
                3,
            ),
            Err(NativeAmxQcBuildError::InvalidValidatorSet)
        );
    }
    // Commit-request and QC replay-validation tests retain their stable libtest paths.
    include!("commit_validation_tail_tests.rs");
