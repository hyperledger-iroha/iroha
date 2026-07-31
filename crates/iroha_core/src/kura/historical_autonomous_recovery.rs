const HISTORICAL_AUTONOMOUS_RECOVERY_DIRECTORY_V1: &str =
    "historical_autonomous_recoveries_v1";
const HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_VERSION_V1: u16 = 1;
const HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_MAX_BYTES: usize = 4 * 1024;

/// Immutable Kura seal proving that one historical autonomous payload crossed
/// the durable execution-input boundary before Queue ownership reopened.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(deny_unknown_fields)]
struct HistoricalAutonomousRecoveryRecordV1 {
    version: u16,
    recovery_id: Hash,
    install_hash: Hash,
    canonical_body: crate::sumeragi::message::CanonicalExecutedBlockNeedV1,
    historical_context_hash: HashOf<HeightContext>,
    payload_hash: Hash,
    reservation_group_hash: Hash,
}

macro_rules! kura_historical_autonomous_recovery_methods {
    () => {
        fn historical_autonomous_recovery_directory(&self) -> PathBuf {
            self.store_root
                .join(HISTORICAL_AUTONOMOUS_RECOVERY_DIRECTORY_V1)
        }

        fn historical_autonomous_recovery_path(
            &self,
            recovery_id: Hash,
        ) -> PathBuf {
            self.historical_autonomous_recovery_directory()
                .join(format!("{}.norito", hex::encode(recovery_id.as_ref())))
        }

        fn invalid_historical_autonomous_recovery(
            path: PathBuf,
            detail: impl Into<String>,
        ) -> Error {
            Error::IO(
                std::io::Error::new(ErrorKind::InvalidData, detail.into()),
                path,
            )
        }

        fn historical_autonomous_payload_without_carrier_hint(
            payload: &LaneExecutablePayloadV1,
        ) -> LaneExecutablePayloadV1 {
            let mut normalized = payload.clone();
            normalized.origin_proposal.payload_block_hint = None;
            normalized
        }

        fn historical_autonomous_expected_record(
            install: &crate::sumeragi::v2_apply::HistoricalAutonomousReservationInstallV1,
        ) -> HistoricalAutonomousRecoveryRecordV1 {
            HistoricalAutonomousRecoveryRecordV1 {
                version: HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_VERSION_V1,
                recovery_id: install.recovery_id,
                install_hash: HashOf::new(install).into(),
                canonical_body: install.canonical_body,
                historical_context_hash: install.historical_context_hash,
                payload_hash: install.payload.payload_hash,
                reservation_group_hash: HashOf::new(&install.reservation_group).into(),
            }
        }

        fn validate_historical_autonomous_install_base(
            &self,
            install: &crate::sumeragi::v2_apply::HistoricalAutonomousReservationInstallV1,
        ) -> Result<HistoricalAutonomousRecoveryRecordV1> {
            let path = self.historical_autonomous_recovery_path(install.recovery_id);
            if !install.has_valid_identity() {
                return Err(Self::invalid_historical_autonomous_recovery(
                    path,
                    "historical autonomous install has an invalid immutable identity",
                ));
            }
            Self::validate_autonomous_reservation_reconciliation_group(
                &install.reservation_group,
            )
            .map_err(|error| {
                Self::invalid_historical_autonomous_recovery(
                    path.clone(),
                    format!("historical autonomous reservation group is invalid: {error}"),
                )
            })?;

            let descriptor = &install.payload.origin_proposal.descriptor;
            let identity = &install.reservation_group.identity;
            let hint = install
                .payload
                .origin_proposal
                .payload_block_hint
                .ok_or_else(|| {
                    Self::invalid_historical_autonomous_recovery(
                        path.clone(),
                        "historical autonomous payload has no canonical carrier hint",
                    )
                })?;
            if install.canonical_body.height == 0
                || install.historical_context.height != install.canonical_body.height
                || install.historical_context.id() != install.historical_context_id
                || HashOf::new(&install.historical_context) != install.historical_context_hash
                || install.carrier_view != hint.proposal_view
                || hint.proposal_height != install.canonical_body.height
                || hint.proposal_block_hash != install.canonical_body.block_hash
                || descriptor.lane_id != identity.lane_id
                || descriptor.dataspace_id != identity.dataspace_id
                || descriptor.lane_incarnation != identity.lane_incarnation
                || descriptor.proposal_height != identity.proposal_height
                || descriptor.lane_block_height != identity.lane_block_height
                || descriptor.lane_block_view != identity.lane_block_view
                || install.payload.reservation_keys != install.reservation_group.ordered_keys
            {
                return Err(Self::invalid_historical_autonomous_recovery(
                    path,
                    "historical autonomous install has conflicting carrier, context, slot, or reservation bindings",
                ));
            }
            install
                .payload
                .validate(install.payload.chain_id_hash, install.payload.epoch)
                .map_err(|error| {
                    Self::invalid_historical_autonomous_recovery(
                        path.clone(),
                        format!("historical autonomous payload is invalid: {error}"),
                    )
                })?;

            let (reservation_owner_hash, proposal_identity_hash) =
                crate::sumeragi::lane_planner::autonomous_lane_reservation_identity_hashes_for_proposal(
                    install.payload.chain_id_hash,
                    install.historical_context_id,
                    install.payload.epoch,
                    &install.payload.origin_proposal,
                    &install.payload.producer,
                )
                .map_err(|error| {
                    Self::invalid_historical_autonomous_recovery(
                        path.clone(),
                        format!("historical autonomous reservation identity is invalid: {error}"),
                    )
                })?;
            if install.payload.reservation_keys.iter().any(|key| {
                key.reservation_owner_hash != reservation_owner_hash
                    || key.proposal_identity_hash != proposal_identity_hash
            }) {
                return Err(Self::invalid_historical_autonomous_recovery(
                    path,
                    "historical autonomous reservations differ from the finalized context identity",
                ));
            }

            let (header, finality) = self
                .v2_finality_artifact_with_header(install.canonical_body.height)?
                .ok_or_else(|| {
                    Self::invalid_historical_autonomous_recovery(
                        path.clone(),
                        "historical autonomous carrier has no verified finality artifact",
                    )
                })?;
            if header.height().get() != install.canonical_body.height
                || header.hash() != install.canonical_body.block_hash
                || header.view_change_index() != install.carrier_view
                || finality.height_context != install.historical_context
                || HashOf::new(&finality) != install.canonical_body.finality_artifact_hash
                || finality.block_hash != install.canonical_body.block_hash
                || finality.commit_qc.execution_commitment
                    != install.canonical_body.execution_commitment
                || finality
                    .commit_qc
                    .execution_commitment
                    .executed_block_wire_hash
                    != install.canonical_body.executed_block_wire_hash
            {
                return Err(Self::invalid_historical_autonomous_recovery(
                    path,
                    "historical autonomous carrier differs from verified Kura finality",
                ));
            }

            let height = NonZeroUsize::new(usize::try_from(install.canonical_body.height)?)
                .ok_or_else(|| {
                    Self::invalid_historical_autonomous_recovery(
                        path.clone(),
                        "historical autonomous carrier height is zero",
                    )
                })?;
            let block = self
                .get_block_without_merge_sidecar(height)
                .ok_or_else(|| {
                    Self::invalid_historical_autonomous_recovery(
                        path.clone(),
                        "historical autonomous carrier body is unavailable",
                    )
                })?;
            if block.header() != header
                || block.hash() != install.canonical_body.block_hash
                || !block.executed_block_wire_hash().is_ok_and(|hash| {
                    hash == install.canonical_body.executed_block_wire_hash
                })
            {
                return Err(Self::invalid_historical_autonomous_recovery(
                    path,
                    "historical autonomous carrier body differs from the signed execution commitment",
                ));
            }

            let normalized =
                Self::historical_autonomous_payload_without_carrier_hint(&install.payload);
            let bundle = block.execution_context().ok_or_else(|| {
                Self::invalid_historical_autonomous_recovery(
                    path.clone(),
                    "historical autonomous carrier has no execution context",
                )
            })?;
            let mut exact_matches = 0_usize;
            for envelope in &bundle.autonomous_lane_payloads {
                let decoded = crate::lane_consensus::decode_autonomous_lane_payload_envelope(
                    envelope,
                    install.payload.chain_id_hash,
                    install.payload.epoch,
                )
                .map_err(|error| {
                    Self::invalid_historical_autonomous_recovery(
                        path.clone(),
                        format!(
                            "historical autonomous carrier contains an invalid payload envelope: {error}"
                        ),
                    )
                })?;
                let same_slot = decoded.origin_proposal.descriptor.lane_id == identity.lane_id
                    && decoded.origin_proposal.descriptor.lane_block_height
                        == identity.lane_block_height
                    && decoded.origin_proposal.descriptor.proposal_height
                        == identity.proposal_height;
                let overlaps = decoded.reservation_keys.iter().any(|candidate| {
                    install.reservation_group.ordered_keys.iter().any(|expected| {
                        candidate.signed_transaction_hash == expected.signed_transaction_hash
                            || candidate.entrypoint_hash == expected.entrypoint_hash
                    })
                });
                if same_slot || overlaps {
                    if decoded != normalized {
                        return Err(Self::invalid_historical_autonomous_recovery(
                            path,
                            "historical autonomous carrier contains a conflicting payload at the recovered slot",
                        ));
                    }
                    exact_matches = exact_matches.checked_add(1).ok_or_else(|| {
                        Self::invalid_historical_autonomous_recovery(
                            path.clone(),
                            "historical autonomous carrier payload match count overflowed",
                        )
                    })?;
                }
            }
            if exact_matches != 1 {
                return Err(Self::invalid_historical_autonomous_recovery(
                    path,
                    "historical autonomous carrier does not contain exactly one recovered payload",
                ));
            }

            let Some((durable_payload, _)) = self.current_autonomous_lane_payload(
                identity.lane_id,
                identity.lane_block_height,
                install.payload.chain_id_hash,
                install.payload.epoch,
            ) else {
                return Err(Self::invalid_historical_autonomous_recovery(
                    path,
                    "historical autonomous payload is not independently durable in Kura",
                ));
            };
            if Self::historical_autonomous_payload_without_carrier_hint(&durable_payload)
                != normalized
            {
                return Err(Self::invalid_historical_autonomous_recovery(
                    path,
                    "historical autonomous payload differs from the exact durable Kura payload",
                ));
            }
            Ok(Self::historical_autonomous_expected_record(install))
        }

        fn validate_historical_autonomous_execution_input(
            &self,
            install: &crate::sumeragi::v2_apply::HistoricalAutonomousReservationInstallV1,
            path: &Path,
        ) -> Result<()> {
            let normalized =
                Self::historical_autonomous_payload_without_carrier_hint(&install.payload);
            let descriptor = &normalized.origin_proposal.descriptor;
            let input = self
                .read_lane_block_execution_input(
                    descriptor.lane_id,
                    descriptor.lane_block_height,
                )
                .ok_or_else(|| {
                    Self::invalid_historical_autonomous_recovery(
                        path.to_path_buf(),
                        "historical autonomous execution input is not durably readable",
                    )
                })?;
            if input.proposal != normalized.origin_proposal
                || input.autonomous_chain_id_hash != Some(normalized.chain_id_hash)
                || input.autonomous_epoch != Some(normalized.epoch)
                || input.autonomous_payload_hash != Some(normalized.payload_hash)
                || input.entrypoint_hashes != normalized.entrypoint_hashes
                || input.reservation_keys != normalized.reservation_keys
                || input.routing_plans != normalized.routing_plans
                || input.native_amx_receipts != normalized.native_amx_receipts
            {
                return Err(Self::invalid_historical_autonomous_recovery(
                    path.to_path_buf(),
                    "historical autonomous execution input differs from the recovered payload",
                ));
            }
            Ok(())
        }

        fn read_historical_autonomous_recovery_record(
            &self,
            path: &Path,
            directory: &Path,
        ) -> Result<Option<HistoricalAutonomousRecoveryRecordV1>> {
            let Some(snapshot) = self.read_regular_sidecar_snapshot(
                path,
                directory,
                HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_MAX_BYTES,
            )? else {
                return Ok(None);
            };
            let mut cursor = snapshot.bytes.as_slice();
            let record = HistoricalAutonomousRecoveryRecordV1::decode_all(&mut cursor)
                .map_err(Error::NoritoFrame)?;
            if record.encode() != snapshot.bytes
                || record.version != HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_VERSION_V1
                || path.file_stem().and_then(std::ffi::OsStr::to_str)
                    != Some(hex::encode(record.recovery_id.as_ref()).as_str())
            {
                return Err(Self::invalid_historical_autonomous_recovery(
                    path.to_path_buf(),
                    "historical autonomous recovery record is noncanonical, unsupported, or mis-associated",
                ));
            }
            Ok(Some(record))
        }

        /// Return whether the exact immutable historical recovery seal exists
        /// and all of its independently durable dependencies still agree.
        pub(crate) fn historical_autonomous_lane_recovery_matches(
            &self,
            install: &crate::sumeragi::v2_apply::HistoricalAutonomousReservationInstallV1,
        ) -> Result<bool> {
            let expected = self.validate_historical_autonomous_install_base(install)?;
            let directory = self.historical_autonomous_recovery_directory();
            let path = self.historical_autonomous_recovery_path(install.recovery_id);
            let Some(record) =
                self.read_historical_autonomous_recovery_record(&path, &directory)?
            else {
                return Ok(false);
            };
            if record != expected {
                return Err(Self::invalid_historical_autonomous_recovery(
                    path,
                    "historical autonomous recovery record conflicts with the requested install",
                ));
            }
            self.validate_historical_autonomous_execution_input(install, &path)?;
            let confirmed = self
                .read_historical_autonomous_recovery_record(&path, &directory)?
                .ok_or_else(|| {
                    Self::invalid_historical_autonomous_recovery(
                        path.clone(),
                        "historical autonomous recovery record disappeared during read-back",
                    )
                })?;
            if confirmed != expected {
                return Err(Self::invalid_historical_autonomous_recovery(
                    path,
                    "historical autonomous recovery record changed during read-back",
                ));
            }
            Ok(true)
        }

        /// Install the exact execution input and immutable recovery seal for a
        /// finalized historical autonomous payload.
        pub(crate) fn persist_historical_autonomous_lane_recovery(
            &self,
            install: &crate::sumeragi::v2_apply::HistoricalAutonomousReservationInstallV1,
        ) -> Result<()> {
            self.ensure_prune_recovery_not_required()?;
            self.durable_mutation_authorized()?;
            let _ = self.validate_historical_autonomous_install_base(install)?;
            let normalized =
                Self::historical_autonomous_payload_without_carrier_hint(&install.payload);
            let recovered = self
                .recover_autonomous_lane_block_payload(
                    &normalized.origin_proposal,
                    normalized.chain_id_hash,
                    normalized.epoch,
                )
                .map_err(|error| {
                    Self::invalid_historical_autonomous_recovery(
                        self.historical_autonomous_recovery_path(install.recovery_id),
                        format!(
                            "historical autonomous execution input recovery failed: {error:?}"
                        ),
                    )
                })?;
            if recovered.reservation_keys != normalized.reservation_keys
                || recovered.routing_plans != normalized.routing_plans
                || recovered.native_amx_receipts != normalized.native_amx_receipts
            {
                return Err(Self::invalid_historical_autonomous_recovery(
                    self.historical_autonomous_recovery_path(install.recovery_id),
                    "historical autonomous recovered execution input is not exact",
                ));
            }
            self.persist_lane_block_execution_input(&recovered)?;

            let expected = self.validate_historical_autonomous_install_base(install)?;
            let directory = self.historical_autonomous_recovery_directory();
            let path = self.historical_autonomous_recovery_path(install.recovery_id);
            self.validate_historical_autonomous_execution_input(install, &path)?;
            let bytes = expected.encode();
            if bytes.len() > HISTORICAL_AUTONOMOUS_RECOVERY_RECORD_MAX_BYTES {
                return Err(Self::invalid_historical_autonomous_recovery(
                    path,
                    "historical autonomous recovery record exceeds its hard byte limit",
                ));
            }

            let accounting_mutation = self.begin_total_disk_usage_mutation();
            if self.canonical_sidecar_directory(&directory)?.is_none() {
                create_dir_all_with_context(&directory)?;
                self.canonical_sidecar_directory(&directory)?.ok_or_else(|| {
                    Self::invalid_historical_autonomous_recovery(
                        directory.clone(),
                        "historical autonomous recovery directory disappeared after creation",
                    )
                })?;
                if let Some(parent) = directory.parent() {
                    sync_dir(parent).map_err(|error| Error::IO(error, parent.to_path_buf()))?;
                }
            }

            let _sidecar_guard = self.sidecar_lock.lock();
            if let Some(existing) =
                self.read_historical_autonomous_recovery_record(&path, &directory)?
            {
                if existing != expected {
                    return Err(Self::invalid_historical_autonomous_recovery(
                        path,
                        "immutable historical autonomous recovery record conflicts with an existing seal",
                    ));
                }
                accounting_mutation.finish();
                return Ok(());
            }
            let wrote = self.write_atomic_synced_noclobber(&path, &bytes)?;
            if wrote {
                self.add_total_disk_usage_bytes(u64::try_from(bytes.len())?);
            } else {
                let existing = self
                    .read_historical_autonomous_recovery_record(&path, &directory)?
                    .ok_or_else(|| {
                        Self::invalid_historical_autonomous_recovery(
                            path.clone(),
                            "historical autonomous recovery lost a no-clobber publication race",
                        )
                    })?;
                if existing != expected {
                    return Err(Self::invalid_historical_autonomous_recovery(
                        path,
                        "historical autonomous recovery no-clobber race published conflicting bytes",
                    ));
                }
            }
            accounting_mutation.finish();
            drop(_sidecar_guard);
            if !self.historical_autonomous_lane_recovery_matches(install)? {
                return Err(Self::invalid_historical_autonomous_recovery(
                    path,
                    "historical autonomous recovery seal is absent after publication",
                ));
            }
            Ok(())
        }
    };
}
