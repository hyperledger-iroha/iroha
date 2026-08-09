use std::marker::PhantomData;

use norito::codec::DecodeAll;
use norito::json::{self, JsonDeserialize};

use super::{default_oracle, *};

#[derive(Clone, Copy)]
pub struct IvmSeed<'e, T> {
    pub ivm: &'e IVM,
    _marker: PhantomData<T>,
}
impl<'e, T> IvmSeed<'e, T> {
    pub fn cast<U>(&self) -> IvmSeed<'e, U> {
        IvmSeed {
            ivm: self.ivm,
            _marker: PhantomData,
        }
    }
}

impl IvmSeed<'_, TriggerSet> {
    #[allow(clippy::unused_self)]
    pub fn parse_trigger_set(self, value: &json::Value) -> Result<TriggerSet, json::Error> {
        let json = json::to_json(value)?;
        let mut parser = json::Parser::new(&json);
        TriggerSet::json_deserialize(&mut parser)
    }
}

pub struct KuraSeed {
    pub kura: Arc<Kura>,
    pub query_handle: LiveQueryStoreHandle,
    #[cfg(feature = "telemetry")]
    pub telemetry: StateTelemetry,
}

impl KuraSeed {
    pub fn into_state_from_json(self, value: json::Value) -> Result<State, json::Error> {
        self.into_state_from_json_with_recovery_mode(value, true)
    }

    /// Decode a State without loading, promoting, truncating, or otherwise
    /// recovering any durable Kura-adjacent journal.
    ///
    /// Replay prevalidation uses this constructor for an isolated dry run;
    /// its in-memory merge and query authority is populated explicitly
    /// from the already authenticated live State.
    pub(crate) fn into_state_from_json_without_durable_recovery(
        self,
        value: json::Value,
    ) -> Result<State, json::Error> {
        self.into_state_from_json_with_recovery_mode(value, false)
    }

    fn into_state_from_json_with_recovery_mode(
        self,
        value: json::Value,
        allow_durable_recovery: bool,
    ) -> Result<State, json::Error> {
        let json::Value::Object(mut map) = value else {
            return Err(json::Error::InvalidField {
                field: "state".into(),
                message: "expected object".into(),
            });
        };

        let world_value = map
            .remove("world")
            .ok_or_else(|| json::Error::missing_field("world"))?;
        if world_value
            .as_object()
            .is_none_or(|world| !world.contains_key("contract_subject_bindings"))
        {
            return Err(json::Error::missing_field(
                "world.contract_subject_bindings",
            ));
        }
        let ivm_runtime = IVM::new(0);
        let ivm_seed = IvmSeed {
            ivm: &ivm_runtime,
            _marker: PhantomData,
        };
        let mut world = parse_world(world_value, &ivm_seed)?;
        let public_lane_validators: Vec<SnapshotNoritoBlob> =
            take_required(&mut map, "public_lane_validators")?;
        let public_lane_stake_shares: Vec<SnapshotNoritoBlob> =
            take_required(&mut map, "public_lane_stake_shares")?;
        let public_lane_rewards: Vec<SnapshotNoritoBlob> =
            take_required(&mut map, "public_lane_rewards")?;
        let public_lane_reward_claims: Vec<SnapshotPublicLaneRewardClaim> =
            take_required(&mut map, "public_lane_reward_claims")?;
        let space_directory_manifests: Vec<SnapshotSpaceDirectoryManifestSet> =
            take_required(&mut map, "space_directory_manifests")?;
        let snapshot_nexus_runtime: SnapshotNexusRuntime = map
            .remove("nexus_runtime")
            .ok_or_else(|| json::Error::missing_field("nexus_runtime"))
            .and_then(|value| {
                json::value::from_value(value).map_err(|err| json::Error::InvalidField {
                    field: "nexus_runtime".to_owned(),
                    message: err.to_string(),
                })
            })?;

        let chain_id: ChainId = take_required(&mut map, "chain_id")?;
        let block_hashes_vec: Vec<HashOf<BlockHeader>> = take_required(&mut map, "block_hashes")?;
        let committed_height =
            u64::try_from(block_hashes_vec.len()).map_err(|_| json::Error::InvalidField {
                field: "state.block_hashes".to_owned(),
                message: "committed height does not fit u64".to_owned(),
            })?;
        validate_musubi_resolver_checkpoint_anchors(&world, &block_hashes_vec)?;
        world
            .privacy_consensus_policy
            .view()
            .get()
            .validate_at_committed_height(committed_height)
            .map_err(|error| json::Error::InvalidField {
                field: "state.world.privacy_consensus_policy".to_owned(),
                message: error.to_string(),
            })?;
        crate::privacy_state::validate_privacy_activations_at_committed_height_v1(
            &world.privacy_activations.view(),
            committed_height,
        )
        .map_err(|message| json::Error::InvalidField {
            field: "state.world.privacy_activations".to_owned(),
            message,
        })?;
        let (
            restored_nexus,
            lane_incarnations,
            lane_incarnation_activation_heights,
            lane_incarnation_lineage,
            autoscale_sample_history,
        ) = nexus_from_snapshot_runtime(snapshot_nexus_runtime, &block_hashes_vec)?;
        let nexus_runtime_restored_from_snapshot = true;
        let transactions: TransactionsStorage = take_required(&mut map, "transactions")?;
        let commit_topology = take_topology_cell(&mut map, "commit_topology")?;
        let prev_commit_topology = take_topology_cell(&mut map, "prev_commit_topology")?;
        let snapshot_v2_bootstrap_candidate: Option<SnapshotV2BootstrapRecord> = map
            .remove("sumeragi_v2_bootstrap")
            .map(json::value::from_value)
            .transpose()
            .map_err(|err| json::Error::InvalidField {
                field: "sumeragi_v2_bootstrap".to_owned(),
                message: err.to_string(),
            })?;

        reject_unknown(&map, "state")?;

        crate::smartcontracts::code::rebuild_contract_subject_addresses(&mut world).map_err(
            |message| json::Error::InvalidField {
                field: "contract_subject_bindings".into(),
                message,
            },
        )?;
        crate::smartcontracts::code::validate_contract_subject_bindings(&world).map_err(
            |message| json::Error::InvalidField {
                field: "contract_subject_bindings".into(),
                message,
            },
        )?;

        let public_lane_validator_records =
            decode_public_lane_validator_records(public_lane_validators)?;
        let public_lane_stake_share_records =
            decode_public_lane_stake_share_records(public_lane_stake_shares)?;

        world.public_lane_validators = public_lane_validator_records
            .into_iter()
            .map(|record| ((record.lane_id, record.validator.clone()), record))
            .collect();
        world.public_lane_stake_shares = public_lane_stake_share_records
            .into_iter()
            .map(|record| {
                (
                    (
                        record.lane_id,
                        record.validator.clone(),
                        record.staker.clone(),
                    ),
                    record,
                )
            })
            .collect();
        world.public_lane_rewards = decode_snapshot_records::<PublicLaneRewardRecord>(
            public_lane_rewards,
            "public_lane_rewards",
        )?
        .into_iter()
        .map(|record| ((record.lane_id, record.epoch), record))
        .collect();
        world.public_lane_reward_claims = public_lane_reward_claims
            .into_iter()
            .map(|record| {
                (
                    (record.lane_id, record.account.clone(), record.asset.clone()),
                    record.last_claimed_epoch,
                )
            })
            .collect();
        world.space_directory_manifests =
            decode_space_directory_manifest_sets(space_directory_manifests)?;

        world
            .validate_quantity_ledger_invariants()
            .map_err(|message| json::Error::InvalidField {
                field: "state.world.numeric_ledgers".to_owned(),
                message,
            })?;

        let state = build_state(
            BuildStateInputs {
                world,
                block_hashes: BlockHashes::new(block_hashes_vec),
                transactions,
                commit_topology,
                prev_commit_topology,
                ivm: ivm_runtime,
                nexus: restored_nexus,
                lane_incarnations,
                lane_incarnation_activation_heights,
                lane_incarnation_lineage,
                autoscale_sample_history,
                chain_id,
                snapshot_v2_bootstrap_candidate,
                nexus_runtime_restored_from_snapshot,
                kura: self.kura,
                query_handle: self.query_handle,
                #[cfg(feature = "telemetry")]
                telemetry: self.telemetry,
            },
            allow_durable_recovery,
        )
        .map_err(|error| json::Error::InvalidField {
            field: "state.durable_merge_ledger".to_owned(),
            message: error.to_string(),
        })?;
        validate_restored_commit_qcs(&state)?;
        super::validate_sccp_state_local_profile(&state).map_err(|message| {
            json::Error::InvalidField {
                field: "state.world.sccp".to_owned(),
                message,
            }
        })?;
        Ok(state)
    }
}

fn validate_restored_commit_qcs(state: &State) -> Result<(), json::Error> {
    let block_hashes = state.block_hashes.view();
    let commit_qcs = state.world.commit_qcs.view();
    for (archive_key, commit_qc) in commit_qcs.iter() {
        let canonical_hash = commit_qc
            .height
            .checked_sub(1)
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| block_hashes.get(index))
            .copied();
        if canonical_hash != Some(*archive_key)
            || !super::commit_qc_matches_block(commit_qc, commit_qc.height, *archive_key)
        {
            return Err(json::Error::InvalidField {
                field: "world.commit_qcs".to_owned(),
                message: format!(
                    "commit-QC archive entry {archive_key} is not an exact commit-phase certificate for its canonical height {}",
                    commit_qc.height
                ),
            });
        }
    }
    Ok(())
}

fn nexus_from_snapshot_runtime(
    runtime: SnapshotNexusRuntime,
    committed_block_hashes: &[HashOf<BlockHeader>],
) -> Result<
    (
        iroha_config::parameters::actual::Nexus,
        BTreeMap<LaneId, Hash>,
        BTreeMap<LaneId, u64>,
        BTreeMap<LaneId, LaneIncarnationLineage>,
        VecDeque<AutoscaleSampleRecord>,
    ),
    json::Error,
> {
    if runtime.version != SnapshotNexusRuntime::VERSION {
        return Err(json::Error::InvalidField {
            field: "nexus_runtime.version".to_owned(),
            message: format!(
                "unsupported Nexus runtime snapshot version {}; expected {}",
                runtime.version,
                SnapshotNexusRuntime::VERSION
            ),
        });
    }
    let autoscale_scale_out_window_blocks =
        std::num::NonZeroU16::new(runtime.autoscale_scale_out_window_blocks).ok_or_else(|| {
            json::Error::InvalidField {
                field: "nexus_runtime.autoscale_scale_out_window_blocks".to_owned(),
                message: "autoscale scale-out window must be non-zero".to_owned(),
            }
        })?;
    let autoscale_scale_in_window_blocks =
        std::num::NonZeroU16::new(runtime.autoscale_scale_in_window_blocks).ok_or_else(|| {
            json::Error::InvalidField {
                field: "nexus_runtime.autoscale_scale_in_window_blocks".to_owned(),
                message: "autoscale scale-in window must be non-zero".to_owned(),
            }
        })?;
    let autoscale_sample_history =
        validate_snapshot_autoscale_sample_history(&runtime, committed_block_hashes)?;
    let lane_count =
        std::num::NonZeroU32::new(runtime.lane_count).ok_or_else(|| json::Error::InvalidField {
            field: "nexus_runtime.lane_count".to_owned(),
            message: "lane_count must be non-zero".to_owned(),
        })?;
    let catalog =
        LaneCatalog::new(lane_count, runtime.lanes).map_err(|err| json::Error::InvalidField {
            field: "nexus_runtime.lanes".to_owned(),
            message: err.to_string(),
        })?;
    if !catalog.lanes().iter().any(|lane| lane.id == LaneId::SINGLE) {
        return Err(json::Error::InvalidField {
            field: "nexus_runtime.lanes".to_owned(),
            message: "routing default lane 0 is missing".to_owned(),
        });
    }
    let mut lane_incarnation_lineage = BTreeMap::new();
    for entry in runtime.lane_incarnation_lineage {
        if lane_incarnation_is_zero(entry.incarnation) {
            return Err(json::Error::InvalidField {
                field: "nexus_runtime.lane_incarnation_lineage".to_owned(),
                message: format!(
                    "lane {} has an all-zero incarnation commitment",
                    entry.lane_id
                ),
            });
        }
        if lane_incarnation_lineage
            .insert(
                entry.lane_id,
                LaneIncarnationLineage {
                    generation: entry.generation,
                    incarnation: entry.incarnation,
                    activation_height: entry.activation_height,
                },
            )
            .is_some()
        {
            return Err(json::Error::InvalidField {
                field: "nexus_runtime.lane_incarnation_lineage".to_owned(),
                message: format!("duplicate entry for lane {}", entry.lane_id),
            });
        }
    }
    let mut lane_incarnations = BTreeMap::new();
    let mut lane_incarnation_activation_heights = BTreeMap::new();
    for lane in catalog.lanes() {
        let entry =
            lane_incarnation_lineage
                .get(&lane.id)
                .ok_or_else(|| json::Error::InvalidField {
                    field: "nexus_runtime.lane_incarnation_lineage".to_owned(),
                    message: format!("active lane {} is missing lineage", lane.id),
                })?;
        lane_incarnations.insert(lane.id, entry.incarnation);
        lane_incarnation_activation_heights.insert(lane.id, entry.activation_height);
    }
    let committed_height = u64::try_from(committed_block_hashes.len()).unwrap_or(u64::MAX);
    validate_lane_incarnation_lineage(
        &catalog,
        &lane_incarnations,
        &lane_incarnation_activation_heights,
        &lane_incarnation_lineage,
    )
    .map_err(|err| json::Error::InvalidField {
        field: "nexus_runtime.lane_incarnation_lineage".to_owned(),
        message: err.to_string(),
    })?;
    if lane_incarnation_activation_heights.get(&LaneId::SINGLE) != Some(&0) {
        return Err(json::Error::InvalidField {
            field: "nexus_runtime.lane_incarnation_lineage".to_owned(),
            message: "physical primary lane 0 must have activation height 0".to_owned(),
        });
    }
    if let Some((lane_id, activation_height)) = lane_incarnation_lineage
        .iter()
        .map(|(lane_id, entry)| (lane_id, entry.activation_height))
        .find(|(_, activation_height)| *activation_height > committed_height)
    {
        return Err(json::Error::InvalidField {
            field: "nexus_runtime.lane_incarnation_lineage".to_owned(),
            message: format!(
                "lane {lane_id} activation height {activation_height} exceeds snapshot height {committed_height}"
            ),
        });
    }
    if runtime.autoscale_last_transition_height > committed_height {
        return Err(json::Error::InvalidField {
            field: "nexus_runtime.autoscale_last_transition_height".to_owned(),
            message: format!(
                "transition height {} exceeds snapshot height {committed_height}",
                runtime.autoscale_last_transition_height
            ),
        });
    }
    let mut latest_managed_creation_height = 0_u64;
    for lane in catalog.lanes() {
        if !lane_uses_reserved_autoscale_metadata(lane) {
            continue;
        }
        ensure_autoscale_managed_lane_shape(lane)
            .and_then(|()| {
                ensure_autoscale_managed_lane_created_height_not_future(lane, committed_height)
            })
            .and_then(|()| ensure_autoscale_lane_drain_close_not_future(lane, committed_height))
            .map_err(|err| json::Error::InvalidField {
                field: format!("nexus_runtime.lanes[{}]", lane.id.as_u32()),
                message: err.to_string(),
            })?;
        let committee = decode_autoscale_lane_committee(lane)
            .ok()
            .flatten()
            .expect("validated autoscale lane carries a canonical committee pin");
        validate_autoscale_lane_committee_pops(&committee).map_err(|reason| {
            json::Error::InvalidField {
                field: format!("nexus_runtime.lanes[{}]", lane.id.as_u32()),
                message: reason.to_owned(),
            }
        })?;
        latest_managed_creation_height = latest_managed_creation_height.max(
            lane.autoscale_created_height()
                .expect("validated managed lane carries a creation height"),
        );
    }
    if runtime.autoscale_last_transition_height < latest_managed_creation_height {
        return Err(json::Error::InvalidField {
            field: "nexus_runtime.autoscale_last_transition_height".to_owned(),
            message: format!(
                "transition height {} precedes managed lane creation height {latest_managed_creation_height}",
                runtime.autoscale_last_transition_height
            ),
        });
    }

    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    nexus.lane_config = iroha_config::parameters::actual::LaneConfig::from_catalog(&catalog);
    nexus.lane_catalog = catalog;
    // These windows determine both the serialized history cap and which retained samples
    // influence the first post-restart autoscale decision. Restore the snapshot-authenticated
    // values before canonical reserialization; substituting process defaults here makes a
    // writer-created snapshot non-canonical whenever operators tune either window.
    nexus.autoscale.scale_out_window_blocks = autoscale_scale_out_window_blocks;
    nexus.autoscale.scale_in_window_blocks = autoscale_scale_in_window_blocks;
    nexus.autoscale.last_transition_height = runtime.autoscale_last_transition_height;
    Ok((
        nexus,
        lane_incarnations,
        lane_incarnation_activation_heights,
        lane_incarnation_lineage,
        autoscale_sample_history,
    ))
}

fn validate_snapshot_autoscale_sample_history(
    runtime: &SnapshotNexusRuntime,
    committed_block_hashes: &[HashOf<BlockHeader>],
) -> Result<VecDeque<AutoscaleSampleRecord>, json::Error> {
    let field = "nexus_runtime.autoscale_sample_history";
    let cap = usize::try_from(runtime.autoscale_sample_history_cap).map_err(|_| {
        json::Error::InvalidField {
            field: "nexus_runtime.autoscale_sample_history_cap".to_owned(),
            message: "history cap does not fit this platform".to_owned(),
        }
    })?;
    if !(2..=MAX_AUTOSCALE_SAMPLE_HISTORY_ENTRIES).contains(&cap) {
        return Err(json::Error::InvalidField {
            field: "nexus_runtime.autoscale_sample_history_cap".to_owned(),
            message: format!(
                "history cap {cap} is outside the supported range 2..={MAX_AUTOSCALE_SAMPLE_HISTORY_ENTRIES}"
            ),
        });
    }
    let scale_out_window = usize::from(runtime.autoscale_scale_out_window_blocks);
    let scale_in_window = usize::from(runtime.autoscale_scale_in_window_blocks);
    if scale_out_window == 0 || scale_in_window == 0 {
        return Err(json::Error::InvalidField {
            field: "nexus_runtime.autoscale_sample_history_cap".to_owned(),
            message: "autoscale snapshot windows must be non-zero".to_owned(),
        });
    }
    let required_cap = scale_out_window.max(scale_in_window).saturating_add(1);
    if cap != required_cap {
        return Err(json::Error::InvalidField {
            field: "nexus_runtime.autoscale_sample_history_cap".to_owned(),
            message: format!(
                "history cap {cap} does not match the configured snapshot windows (required {required_cap})"
            ),
        });
    }
    let history = &runtime.autoscale_sample_history;
    if history.len() > cap {
        return Err(json::Error::InvalidField {
            field: field.to_owned(),
            message: format!(
                "history contains {} records but its declared cap is {cap}",
                history.len()
            ),
        });
    }
    if committed_block_hashes.is_empty() {
        if history.is_empty() {
            return Ok(VecDeque::new());
        }
        return Err(json::Error::InvalidField {
            field: field.to_owned(),
            message: "history must be empty at snapshot height zero".to_owned(),
        });
    }
    if history.is_empty() {
        return Err(json::Error::InvalidField {
            field: field.to_owned(),
            message: "history must retain the latest committed block".to_owned(),
        });
    }

    let committed_height =
        u64::try_from(committed_block_hashes.len()).map_err(|_| json::Error::InvalidField {
            field: field.to_owned(),
            message: "committed height does not fit u64".to_owned(),
        })?;
    let history_len = u64::try_from(history.len()).map_err(|_| json::Error::InvalidField {
        field: field.to_owned(),
        message: "history length does not fit u64".to_owned(),
    })?;
    let expected_first_height = committed_height
        .checked_sub(history_len.saturating_sub(1))
        .ok_or_else(|| json::Error::InvalidField {
            field: field.to_owned(),
            message: "history is longer than the committed chain".to_owned(),
        })?;
    if expected_first_height == 0 {
        return Err(json::Error::InvalidField {
            field: field.to_owned(),
            message: "history contains a zero block height".to_owned(),
        });
    }

    let mut previous_timestamp = None;
    for (index, record) in history.iter().enumerate() {
        let offset = u64::try_from(index).map_err(|_| json::Error::InvalidField {
            field: field.to_owned(),
            message: "history index does not fit u64".to_owned(),
        })?;
        let expected_height =
            expected_first_height
                .checked_add(offset)
                .ok_or_else(|| json::Error::InvalidField {
                    field: field.to_owned(),
                    message: "history height overflow".to_owned(),
                })?;
        if record.block_height != expected_height {
            return Err(json::Error::InvalidField {
                field: field.to_owned(),
                message: format!(
                    "record {index} has height {}; expected consecutive height {expected_height}",
                    record.block_height
                ),
            });
        }
        let hash_index = usize::try_from(record.block_height.saturating_sub(1)).map_err(|_| {
            json::Error::InvalidField {
                field: field.to_owned(),
                message: format!("record {index} height does not fit this platform"),
            }
        })?;
        if committed_block_hashes.get(hash_index) != Some(&record.block_hash) {
            return Err(json::Error::InvalidField {
                field: field.to_owned(),
                message: format!(
                    "record {index} hash does not match committed height {}",
                    record.block_height
                ),
            });
        }
        if record.creation_time_ms == 0 || record.creation_time_ms == u64::MAX {
            return Err(json::Error::InvalidField {
                field: field.to_owned(),
                message: format!(
                    "record {index} has an invalid creation timestamp {}",
                    record.creation_time_ms
                ),
            });
        }
        if previous_timestamp.is_some_and(|previous| record.creation_time_ms <= previous) {
            return Err(json::Error::InvalidField {
                field: field.to_owned(),
                message: format!(
                    "record {index} creation timestamp {} is not strictly increasing",
                    record.creation_time_ms
                ),
            });
        }
        if record.work_count == u64::MAX {
            return Err(json::Error::InvalidField {
                field: field.to_owned(),
                message: format!("record {index} work count is outside the supported range"),
            });
        }
        previous_timestamp = Some(record.creation_time_ms);
    }

    Ok(history.iter().copied().collect())
}

fn decode_snapshot_records<T>(
    records: Vec<SnapshotNoritoBlob>,
    field: &str,
) -> Result<Vec<T>, json::Error>
where
    T: DecodeAll,
{
    records
        .into_iter()
        .enumerate()
        .map(|(index, record)| {
            let bytes =
                hex::decode(&record.encoded_hex).map_err(|err| json::Error::InvalidField {
                    field: field.to_owned(),
                    message: format!("record {index} hex decode failed: {err}"),
                })?;
            let mut cursor = bytes.as_slice();
            T::decode_all(&mut cursor).map_err(|err| json::Error::InvalidField {
                field: field.to_owned(),
                message: format!("record {index} norito decode failed: {err}"),
            })
        })
        .collect()
}

fn decode_public_lane_validator_records(
    records: Vec<SnapshotNoritoBlob>,
) -> Result<Vec<PublicLaneValidatorRecord>, json::Error> {
    let mut decoded = Vec::with_capacity(records.len());
    for (index, record) in records.into_iter().enumerate() {
        let bytes = hex::decode(&record.encoded_hex).map_err(|err| json::Error::InvalidField {
            field: "public_lane_validators".to_owned(),
            message: format!("record {index} hex decode failed: {err}"),
        })?;
        let mut cursor = bytes.as_slice();
        decoded.push(
            PublicLaneValidatorRecord::decode_all(&mut cursor).map_err(|err| {
                json::Error::InvalidField {
                    field: "public_lane_validators".to_owned(),
                    message: format!("record {index} norito decode failed: {err}"),
                }
            })?,
        );
    }
    Ok(decoded)
}

fn decode_public_lane_stake_share_records(
    records: Vec<SnapshotNoritoBlob>,
) -> Result<Vec<PublicLaneStakeShare>, json::Error> {
    let mut decoded = Vec::with_capacity(records.len());
    for (index, record) in records.into_iter().enumerate() {
        let bytes = hex::decode(&record.encoded_hex).map_err(|err| json::Error::InvalidField {
            field: "public_lane_stake_shares".to_owned(),
            message: format!("record {index} hex decode failed: {err}"),
        })?;
        let mut cursor = bytes.as_slice();
        decoded.push(
            PublicLaneStakeShare::decode_all(&mut cursor).map_err(|err| {
                json::Error::InvalidField {
                    field: "public_lane_stake_shares".to_owned(),
                    message: format!("record {index} norito decode failed: {err}"),
                }
            })?,
        );
    }
    Ok(decoded)
}

fn decode_space_directory_manifest_sets(
    records: Vec<SnapshotSpaceDirectoryManifestSet>,
) -> Result<Storage<UniversalAccountId, SpaceDirectoryManifestSet>, json::Error> {
    let mut storage = Storage::default();
    for (index, record) in records.into_iter().enumerate() {
        let bytes = hex::decode(&record.encoded_hex).map_err(|err| json::Error::InvalidField {
            field: "space_directory_manifests".to_owned(),
            message: format!("record {index} hex decode failed: {err}"),
        })?;
        let mut cursor = bytes.as_slice();
        let manifest_set = SpaceDirectoryManifestSet::decode_all(&mut cursor).map_err(|err| {
            json::Error::InvalidField {
                field: "space_directory_manifests".to_owned(),
                message: format!("record {index} norito decode failed: {err}"),
            }
        })?;
        if storage.insert(record.uaid, manifest_set).is_some() {
            return Err(json::Error::InvalidField {
                field: "space_directory_manifests".to_owned(),
                message: format!("duplicate UAID at record {index}"),
            });
        }
    }
    Ok(storage)
}

fn take_required<T: JsonDeserialize>(
    map: &mut json::native::Map,
    key: &str,
) -> Result<T, json::Error> {
    let value = map
        .remove(key)
        .ok_or_else(|| json::Error::missing_field(key))?;
    json::value::from_value(value).map_err(|err| json::Error::InvalidField {
        field: key.to_owned(),
        message: err.to_string(),
    })
}

fn take_optional_default<T>(map: &mut json::native::Map, key: &str) -> Result<T, json::Error>
where
    T: JsonDeserialize + Default,
{
    map.remove(key).map_or_else(
        || Ok(T::default()),
        |value| {
            json::value::from_value(value).map_err(|err| json::Error::InvalidField {
                field: key.to_owned(),
                message: err.to_string(),
            })
        },
    )
}

fn take_musubi_namespace_bindings(
    map: &mut json::native::Map,
) -> Result<Storage<MusubiNamespaceV1, MusubiNamespaceBindingV1>, json::Error> {
    let bindings: Storage<MusubiNamespaceV1, MusubiNamespaceBindingV1> =
        take_required(map, "musubi_namespace_bindings")?;
    for (namespace, binding) in bindings.view().iter() {
        if namespace != &binding.namespace {
            return Err(json::Error::InvalidField {
                field: "musubi_namespace_bindings".to_owned(),
                message: format!(
                    "binding key '{namespace}' does not match embedded namespace '{}'",
                    binding.namespace
                ),
            });
        }
        binding
            .validate()
            .map_err(|error| json::Error::InvalidField {
                field: "musubi_namespace_bindings".to_owned(),
                message: error.to_string(),
            })?;
    }
    Ok(bindings)
}

fn take_musubi_domain_ownership_generations(
    map: &mut json::native::Map,
) -> Result<Storage<DomainId, u64>, json::Error> {
    let generations: Storage<DomainId, u64> =
        take_required(map, "musubi_domain_ownership_generations")?;
    for (domain, generation) in generations.view().iter() {
        if *generation < 2 {
            return Err(json::Error::InvalidField {
                field: "musubi_domain_ownership_generations".to_owned(),
                message: format!(
                    "domain '{domain}' stores noncanonical generation {generation}; absent means generation 1 and persisted entries start at 2"
                ),
            });
        }
    }
    Ok(generations)
}

fn take_musubi_registry_policy(
    map: &mut json::native::Map,
) -> Result<Cell<MusubiRegistryPolicyV1>, json::Error> {
    let policy: Cell<MusubiRegistryPolicyV1> = take_required(map, "musubi_registry_policy")?;
    policy
        .view()
        .get()
        .validate()
        .map_err(|error| json::Error::InvalidField {
            field: "musubi_registry_policy".to_owned(),
            message: error.to_string(),
        })?;
    Ok(policy)
}

fn take_musubi_resolver_index_revision(
    map: &mut json::native::Map,
) -> Result<Cell<MusubiResolverIndexRevisionV1>, json::Error> {
    let revision: Cell<MusubiResolverIndexRevisionV1> =
        take_required(map, "musubi_resolver_index_revision")?;
    if revision.view().get().get() == 0 {
        return Err(json::Error::InvalidField {
            field: "musubi_resolver_index_revision".to_owned(),
            message: "Musubi resolver-index revision must be non-zero".to_owned(),
        });
    }
    Ok(revision)
}

fn take_musubi_resolver_index_checkpoints(
    map: &mut json::native::Map,
) -> Result<Storage<MusubiResolverIndexRevisionV1, MusubiRegistrySnapshotV1>, json::Error> {
    take_required(map, "musubi_resolver_index_checkpoints")
}

fn take_musubi_replication_shortfall_releases(
    map: &mut json::native::Map,
) -> Result<Cell<u64>, json::Error> {
    take_required(map, "musubi_replication_shortfall_releases")
}

fn validate_provider_ingest_completion_authorities(
    provider_owners: &Storage<ProviderId, AccountId>,
    authorities: &Storage<ProviderId, ProviderIngestCompletionAuthorityV1>,
) -> Result<(), json::Error> {
    let owners = provider_owners.view();
    for (provider_id, authority) in authorities.view().iter() {
        if !authority.is_valid() || owners.get(provider_id) != Some(&authority.provider_owner) {
            return Err(json::Error::InvalidField {
                field: "provider_ingest_completion_authorities".to_owned(),
                message: format!(
                    "provider {} has a noncanonical or owner-mismatched completion authority",
                    hex::encode(provider_id.as_bytes())
                ),
            });
        }
    }
    Ok(())
}

pub(super) fn validate_ram_lfe_program_policies(
    policies: &Storage<RamLfeProgramId, RamLfeProgramPolicy>,
) -> Result<(), json::Error> {
    for (program_id, policy) in policies.view().iter() {
        crate::smartcontracts::isi::ram_lfe::validate_program_policy(policy).map_err(|err| {
            json::Error::InvalidField {
                field: format!("world.ram_lfe_program_policies.{program_id}"),
                message: err.to_string(),
            }
        })?;
    }
    Ok(())
}

fn validate_sccp_inbound_messages(
    messages: &Storage<SccpInboundMessageKeyV1, SccpInboundMessageRecordV1>,
) -> Result<(), json::Error> {
    for (key, record) in messages.view().iter() {
        if !key.is_well_formed() {
            return Err(json::Error::InvalidField {
                field: "world.sccp_inbound_messages".to_owned(),
                message:
                    "replay key is not an exact external-to-SORA lane with a nonzero message id"
                        .to_owned(),
            });
        }
        if !record.is_well_formed_for_lane(key.lane) {
            return Err(json::Error::InvalidField {
                field: "world.sccp_inbound_messages".to_owned(),
                message: format!(
                    "replay record for {} -> {} contains invalid evidence or a mismatched native backend",
                    key.lane.source.profile_key(),
                    key.lane.target.profile_key()
                ),
            });
        }
    }
    Ok(())
}

fn validate_sccp_outbound_pending_messages(
    messages: &Storage<SccpOutboundMessageKeyV1, SccpOutboundPendingMessageRecordV1>,
) -> Result<(), json::Error> {
    for (key, record) in messages.view().iter() {
        crate::bridge::validate_sccp_outbound_message_record_v1(key, record).ok_or_else(|| {
                json::Error::InvalidField {
                    field: "world.sccp_outbound_pending_messages".to_owned(),
                    message: "outbound replay entry must carry one bounded canonical payload bound to its exact lane, governed context, message id, and payload hash".to_owned(),
                }
            })?;
    }
    Ok(())
}

fn validate_sccp_outbound_pending_usage(
    messages: &Storage<SccpOutboundMessageKeyV1, SccpOutboundPendingMessageRecordV1>,
    usage: &Cell<SccpOutboundPendingUsageV1>,
) -> Result<(), json::Error> {
    let mut expected = SccpOutboundPendingUsageV1::default();
    for (_, record) in messages.view().iter() {
        expected = expected
            .checked_add_payload(record.payload_bytes.len())
            .ok_or_else(|| json::Error::InvalidField {
                field: "world.sccp_outbound_pending_usage".to_owned(),
                message: "pending outbound usage overflows its fixed counters".to_owned(),
            })?;
    }
    let actual = *usage.view().get();
    if !actual.is_structurally_valid() || actual != expected {
        return Err(json::Error::InvalidField {
            field: "world.sccp_outbound_pending_usage".to_owned(),
            message: format!(
                "pending outbound usage does not match payload-bearing records: expected {expected:?}, found {actual:?}"
            ),
        });
    }
    Ok(())
}

fn validate_sccp_outbound_proofs(
    proofs: &Storage<SccpOutboundMessageKeyV1, SccpOutboundProofRecordV1>,
    locator: &Storage<[u8; 32], SccpOutboundMessageKeyV1>,
) -> Result<(), json::Error> {
    let locator = locator.view();
    for (key, proof) in proofs.view().iter() {
        if !proof.is_well_formed_for_key(key) {
            return Err(json::Error::InvalidField {
                    field: "world.sccp_outbound_proofs".to_owned(),
                    message: "outbound proof replay entry must use one exact outbound lane/message key, ordered nonzero heights, and six distinct nonzero hash roles".to_owned(),
                });
        }
        if locator.get(&key.message_id) != Some(key) {
            return Err(json::Error::InvalidField {
                    field: "world.sccp_outbound_proofs".to_owned(),
                    message: "outbound proof replay key is inconsistent with the global outbound message locator".to_owned(),
                });
        }
    }
    Ok(())
}

fn validate_sccp_outbound_indexes(
    pending: &Storage<SccpOutboundMessageKeyV1, SccpOutboundPendingMessageRecordV1>,
    terminal: &Storage<SccpOutboundMessageKeyV1, SccpOutboundProofRecordV1>,
    locator: &Storage<[u8; 32], SccpOutboundMessageKeyV1>,
    ordered: &Storage<SccpOutboundMessageIndexKeyV1, ()>,
) -> Result<(), json::Error> {
    let pending = pending.view();
    let terminal = terminal.view();
    let locator = locator.view();
    let ordered = ordered.view();
    let union_len = pending
        .iter()
        .count()
        .checked_add(terminal.iter().count())
        .ok_or_else(|| json::Error::InvalidField {
            field: "world.sccp_outbound_message_index".to_owned(),
            message: "outbound replay union cardinality overflows".to_owned(),
        })?;
    if union_len != locator.iter().count() || union_len != ordered.iter().count() {
        return Err(json::Error::InvalidField {
                field: "world.sccp_outbound_message_index".to_owned(),
                message: "pending/terminal outbound replay union, global locator, and ordered index cardinalities differ".to_owned(),
            });
    }
    for (key, record) in pending.iter() {
        if terminal.get(key).is_some() {
            return Err(json::Error::InvalidField {
                field: "world.sccp_outbound_proofs".to_owned(),
                message: "one outbound replay key appears in both pending and terminal state"
                    .to_owned(),
            });
        }
        if locator.get(&key.message_id) != Some(key) {
            return Err(json::Error::InvalidField {
                field: "world.sccp_outbound_message_locator".to_owned(),
                message: "global message-id locator is missing or aliases another replay key"
                    .to_owned(),
            });
        }
        let index_key = SccpOutboundMessageIndexKeyV1::new(*key, record).ok_or_else(|| {
            json::Error::InvalidField {
                field: "world.sccp_outbound_message_index".to_owned(),
                message: "authoritative outbound entry cannot form a valid ordered locator"
                    .to_owned(),
            }
        })?;
        if ordered.get(&index_key).is_none() {
            return Err(json::Error::InvalidField {
                field: "world.sccp_outbound_message_index".to_owned(),
                message: "ordered outbound locator is missing".to_owned(),
            });
        }
    }
    for (key, record) in terminal.iter() {
        if locator.get(&key.message_id) != Some(key) {
            return Err(json::Error::InvalidField {
                field: "world.sccp_outbound_message_locator".to_owned(),
                message:
                    "global message-id locator is missing or aliases another terminal replay key"
                        .to_owned(),
            });
        }
        let index_key =
            SccpOutboundMessageIndexKeyV1::from_terminal(*key, record).ok_or_else(|| {
                json::Error::InvalidField {
                    field: "world.sccp_outbound_message_index".to_owned(),
                    message: "terminal outbound entry cannot form a valid ordered locator"
                        .to_owned(),
                }
            })?;
        if ordered.get(&index_key).is_none() {
            return Err(json::Error::InvalidField {
                field: "world.sccp_outbound_message_index".to_owned(),
                message: "ordered terminal outbound locator is missing".to_owned(),
            });
        }
    }
    for (message_id, key) in locator.iter() {
        let present =
            usize::from(pending.get(key).is_some()) + usize::from(terminal.get(key).is_some());
        if *message_id != key.message_id || present != 1 {
            return Err(json::Error::InvalidField {
                field: "world.sccp_outbound_message_locator".to_owned(),
                message: "global locator must name exactly one pending or terminal replay entry"
                    .to_owned(),
            });
        }
    }
    let mut current_height = None;
    let mut expected_commitment_index = 0_u32;
    for (index_key, ()) in ordered.iter() {
        if current_height != Some(index_key.recorded_at_height) {
            current_height = Some(index_key.recorded_at_height);
            expected_commitment_index = 0;
        }
        if expected_commitment_index
            >= iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1
        {
            return Err(json::Error::InvalidField {
                field: "world.sccp_outbound_message_index".to_owned(),
                message: format!(
                    "height {} exceeds the fixed {}-message SCCP outbox bound",
                    index_key.recorded_at_height,
                    iroha_data_model::bridge::SCCP_OUTBOUND_MESSAGES_MAX_PER_BLOCK_V1
                ),
            });
        }
        if !index_key.is_well_formed() {
            return Err(json::Error::InvalidField {
                field: "world.sccp_outbound_message_index".to_owned(),
                message: "ordered locator is malformed".to_owned(),
            });
        }
        if index_key.commitment_index != expected_commitment_index {
            return Err(json::Error::InvalidField {
                field: "world.sccp_outbound_message_index".to_owned(),
                message: format!(
                    "height {} commitment indices must be dense from zero: expected {}, found {}",
                    index_key.recorded_at_height,
                    expected_commitment_index,
                    index_key.commitment_index
                ),
            });
        }
        let key = index_key.message_key();
        let pending_index = pending
            .get(&key)
            .and_then(|record| SccpOutboundMessageIndexKeyV1::new(key, record));
        let terminal_index = terminal
            .get(&key)
            .and_then(|record| SccpOutboundMessageIndexKeyV1::from_terminal(key, record));
        if !matches!((pending_index, terminal_index), (Some(actual), None) | (None, Some(actual)) if actual == *index_key)
        {
            return Err(json::Error::InvalidField {
                field: "world.sccp_outbound_message_index".to_owned(),
                message: "ordered locator height, commitment index, or replay key is inconsistent"
                    .to_owned(),
            });
        }
        expected_commitment_index += 1;
    }
    Ok(())
}

fn take_ram_lfe_program_policies(
    map: &mut json::native::Map,
) -> Result<Storage<RamLfeProgramId, RamLfeProgramPolicy>, json::Error> {
    let value = map
        .remove("ram_lfe_program_policies")
        .ok_or_else(|| json::Error::missing_field("ram_lfe_program_policies"))?;
    json::value::from_value(value).map_err(|err| json::Error::InvalidField {
        field: "ram_lfe_program_policies".to_owned(),
        message: err.to_string(),
    })
}

fn take_parameters_cell(
    map: &mut json::native::Map,
    key: &str,
) -> Result<Cell<Parameters>, json::Error> {
    let value = map
        .remove(key)
        .ok_or_else(|| json::Error::missing_field(key))?;
    json::value::from_value(value).map_err(|err| json::Error::InvalidField {
        field: key.to_owned(),
        message: err.to_string(),
    })
}

fn take_topology_cell(
    map: &mut json::native::Map,
    key: &str,
) -> Result<Cell<Vec<PeerId>>, json::Error> {
    let value = map
        .remove(key)
        .ok_or_else(|| json::Error::missing_field(key))?;
    match value {
        json::Value::Array(_) => {
            let peers: Vec<PeerId> = json::value::from_value(value)?;
            Ok(Cell::new(peers))
        }
        other => json::value::from_value(other),
    }
}

fn reject_legacy_musubi_state(
    smart_contract_state: &Storage<StatePath, Vec<u8>>,
) -> Result<(), json::Error> {
    let legacy = smart_contract_state.view().iter().find_map(|(key, _)| {
        let key = key.as_ref();
        is_legacy_musubi_state_path(key).then_some(key.to_owned())
    });
    if let Some(key) = legacy {
        return Err(json::Error::InvalidField {
            field: "smart_contract_state".to_owned(),
            message: format!(
                "legacy pre-release Musubi state `{key}` is unsupported; reset registry state"
            ),
        });
    }
    Ok(())
}

fn is_legacy_musubi_state_path(path: &str) -> bool {
    path == "musubi"
        || path.starts_with("musubi_")
        || path.starts_with("musubi/")
        || path.starts_with("musubi.")
        || path.starts_with("musubi:")
}

pub(super) fn validate_musubi_location_reverse_indices(
    locations: &Storage<MusubiArchiveLocationKeyV1, MusubiArchiveLocationV1>,
    by_pin: &Storage<ManifestDigest, MusubiPinLocationReferenceV1>,
    by_order: &Storage<ReplicationOrderId, MusubiReplicationOrderLocationReferenceV1>,
    by_provider: &Storage<MusubiProviderLocationKeyV1, ()>,
) -> Result<(), json::Error> {
    let invalid = |message: String| json::Error::InvalidField {
        field: "world.musubi_location_reverse_indices".to_owned(),
        message,
    };
    let locations = locations.view();
    let by_pin = by_pin.view();
    let by_order = by_order.view();
    let by_provider = by_provider.view();

    for (digest, reference) in by_pin.iter() {
        reference
            .validate()
            .map_err(|error| invalid(error.to_string()))?;
        if digest != &reference.pin_manifest {
            return Err(invalid(
                "pin reverse-index key does not match its duplicated manifest digest".into(),
            ));
        }
        let target = locations
            .get(&reference.location)
            .ok_or_else(|| invalid("pin reverse reference targets a missing location".into()))?;
        if reference.active {
            if target.state == MusubiArchiveLocationStateV1::Retired
                || target.pin_manifest != *digest
            {
                return Err(invalid(
                    "active pin reverse reference does not match its current location".into(),
                ));
            }
        }
    }
    for (order, reference) in by_order.iter() {
        reference
            .validate()
            .map_err(|error| invalid(error.to_string()))?;
        if order != &reference.replication_order {
            return Err(invalid(
                "order reverse-index key does not match its duplicated order identity".into(),
            ));
        }
        let target = locations
            .get(&reference.location)
            .ok_or_else(|| invalid("order reverse reference targets a missing location".into()))?;
        if reference.active {
            if target.state == MusubiArchiveLocationStateV1::Retired
                || target.replication_order != *order
            {
                return Err(invalid(
                    "active order reverse reference does not match its current location".into(),
                ));
            }
        }
    }
    for (key, ()) in by_provider.iter() {
        key.validate().map_err(|error| invalid(error.to_string()))?;
        let location = locations.get(&key.location).ok_or_else(|| {
            invalid("provider reverse reference targets a missing location".into())
        })?;
        if location.state == MusubiArchiveLocationStateV1::Retired
            || location.providers.binary_search(&key.provider_id).is_err()
        {
            return Err(invalid(
                "provider reverse reference does not match its current location".into(),
            ));
        }
    }
    for (key, location) in locations.iter() {
        location
            .validate()
            .map_err(|error| invalid(error.to_string()))?;
        if key != &location.key() {
            return Err(invalid(
                "archive-location key does not match the stored record".into(),
            ));
        }
        if location.state == MusubiArchiveLocationStateV1::Retired {
            if !by_pin
                .get(&location.pin_manifest)
                .is_some_and(|reference| !reference.active && reference.location == *key)
                || !by_order
                    .get(&location.replication_order)
                    .is_some_and(|reference| !reference.active && reference.location == *key)
            {
                return Err(invalid(
                    "retired archive location is missing an immutable reuse tombstone".into(),
                ));
            }
            continue;
        }
        if !by_pin
            .get(&location.pin_manifest)
            .is_some_and(|reference| reference.active && reference.location == *key)
            || !by_order
                .get(&location.replication_order)
                .is_some_and(|reference| reference.active && reference.location == *key)
            || location.providers.iter().any(|provider| {
                by_provider
                    .get(&MusubiProviderLocationKeyV1::new(*provider, *key))
                    .is_none()
            })
        {
            return Err(invalid(
                "current archive location is missing an exact reverse-index entry".into(),
            ));
        }
    }
    Ok(())
}

fn invalid_musubi_state(field: &str, message: impl Into<String>) -> json::Error {
    json::Error::InvalidField {
        field: format!("world.{field}"),
        message: message.into(),
    }
}

fn validate_musubi_resolver_checkpoint_structure(
    checkpoints: &Storage<MusubiResolverIndexRevisionV1, MusubiRegistrySnapshotV1>,
    current_revision: u64,
) -> Result<(), json::Error> {
    let checkpoints = checkpoints.view();
    let mut previous_height = None;
    let mut latest_revision = None;
    for (revision, checkpoint) in checkpoints.iter() {
        checkpoint.validate().map_err(|error| {
            invalid_musubi_state("musubi_resolver_index_checkpoints", error.to_string())
        })?;
        if revision.get() != checkpoint.index_revision {
            return Err(invalid_musubi_state(
                "musubi_resolver_index_checkpoints",
                "resolver checkpoint key does not match its embedded revision",
            ));
        }
        if previous_height.is_none() && checkpoint.finalized_height != 1 {
            return Err(invalid_musubi_state(
                "musubi_resolver_index_checkpoints",
                "resolver checkpoint history must begin at genesis",
            ));
        }
        if previous_height.is_some_and(|height| height >= checkpoint.finalized_height) {
            return Err(invalid_musubi_state(
                "musubi_resolver_index_checkpoints",
                "resolver checkpoint activation heights must increase strictly",
            ));
        }
        previous_height = Some(checkpoint.finalized_height);
        latest_revision = Some(revision.get());
    }
    if latest_revision.is_some_and(|revision| revision != current_revision) {
        return Err(invalid_musubi_state(
            "musubi_resolver_index_checkpoints",
            "latest resolver checkpoint does not match the current resolver-index revision",
        ));
    }
    Ok(())
}

fn validate_musubi_resolver_checkpoint_anchors(
    world: &World,
    block_hashes: &[HashOf<BlockHeader>],
) -> Result<(), json::Error> {
    let current_revision = world.musubi_resolver_index_revision.view().get().get();
    validate_musubi_resolver_checkpoint_structure(
        &world.musubi_resolver_index_checkpoints,
        current_revision,
    )?;

    let checkpoints = world.musubi_resolver_index_checkpoints.view();
    let history_is_empty = checkpoints.is_empty();
    if block_hashes.is_empty() {
        if !history_is_empty {
            return Err(invalid_musubi_state(
                "musubi_resolver_index_checkpoints",
                "pregenesis state cannot contain resolver checkpoints",
            ));
        }
        return Ok(());
    }
    if history_is_empty {
        return Err(invalid_musubi_state(
            "musubi_resolver_index_checkpoints",
            "committed state must retain the genesis resolver checkpoint",
        ));
    }
    for (_, checkpoint) in checkpoints.iter() {
        let index = checkpoint
            .finalized_height
            .checked_sub(1)
            .and_then(|index| usize::try_from(index).ok());
        let canonical_hash = index
            .and_then(|index| block_hashes.get(index))
            .map(|hash| *hash.as_ref());
        if canonical_hash != Some(checkpoint.finalized_block_hash) {
            return Err(invalid_musubi_state(
                "musubi_resolver_index_checkpoints",
                "resolver checkpoint is not anchored to its canonical finalized block",
            ));
        }
    }
    Ok(())
}
