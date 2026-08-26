use super::{default_oracle, *};
use norito::codec::{DecodeAll, Encode};
use norito::json::{self, JsonDeserialize, JsonSerialize};
use std::{collections::BTreeMap, marker::PhantomData, sync::OnceLock};
#[cfg(test)]
std::thread_local! {
    static SNAPSHOT_NORITO_CANONICAL_PASSES: std::cell::Cell<usize> = const {
        std::cell::Cell::new(0)
    };
}

enum SnapshotJsonField<'a> {
    Borrowed { raw: &'a str },
    #[cfg(test)]
    Owned(json::Value),
}
impl<'a> SnapshotJsonField<'a> {
    fn decode_canonical<T>(self, field: &str) -> Result<T, json::Error>
    where
        T: JsonDeserialize + JsonSerialize,
    {
        let decoded: Result<T, json::Error> = match self {
            #[cfg(test)]
            Self::Owned(value) => json::value::from_value(value),
            Self::Borrowed { raw } => (|| {
                let value = json::from_str::<T>(raw)?;
                // TODO: Teach Norito JSON serialization to target a comparison sink so
                // canonical verification does not need one field-sized temporary String.
                let canonical = json::to_json(&value)?;
                if canonical.as_bytes() != raw.as_bytes() {
                    return Err(json::Error::Message(
                        "snapshot field is not canonically encoded".to_owned(),
                    ));
                }
                Ok(value)
            })(),
        };
        decoded.map_err(|error| json::Error::InvalidField {
            field: field.to_owned(),
            message: error.to_string(),
        })
    }
    fn into_object(self, field: &str) -> Result<SnapshotJsonMap<'a>, json::Error> {
        match self {
            Self::Borrowed { raw } => SnapshotJsonMap::parse(raw, field),
            #[cfg(test)]
            Self::Owned(json::Value::Object(map)) => Ok(SnapshotJsonMap::from_owned(map)),
            #[cfg(test)]
            Self::Owned(_) => Err(json::Error::InvalidField {
                field: field.to_owned(),
                message: "expected object".to_owned(),
            }),
        }
    }
    fn validate_sccp_registry(&self) -> Result<(), json::Error> {
        match self {
            Self::Borrowed { raw } => validate_sccp_registry_cell_json_str(raw),
            #[cfg(test)]
            Self::Owned(value) => validate_sccp_registry_cell_json(value),
        }
        .map_err(|message| json::Error::InvalidField {
            field: "sccp_registry".to_owned(),
            message,
        })
    }
}
struct SnapshotJsonMap<'a> {
    fields: BTreeMap<String, SnapshotJsonField<'a>>,
    source_order: Option<Vec<String>>,
}
impl<'a> SnapshotJsonMap<'a> {
    #[cfg(test)]
    fn from_owned(map: json::native::Map) -> Self {
        Self {
            fields: map
                .into_iter()
                .map(|(key, value)| (key, SnapshotJsonField::Owned(value)))
                .collect(),
            source_order: None,
        }
    }
    fn parse(input: &'a str, field: &str) -> Result<Self, json::Error> {
        let mut parser = json::Parser::new(input);
        parser
            .expect(b'{')
            .map_err(|error| json::Error::InvalidField {
                field: field.to_owned(),
                message: error.to_string(),
            })?;
        parser.skip_ws();
        let mut fields = BTreeMap::new();
        let mut source_order = Vec::new();
        if parser.peek() == Some(b'}') {
            parser.bump();
        } else {
            loop {
                let key = parser
                    .parse_string()
                    .map_err(|error| json::Error::InvalidField {
                        field: field.to_owned(),
                        message: error.to_string(),
                    })?;
                parser
                    .expect(b':')
                    .map_err(|error| json::Error::InvalidField {
                        field: field.to_owned(),
                        message: error.to_string(),
                    })?;
                parser.skip_ws();
                let start = parser.position();
                parser
                    .skip_value()
                    .map_err(|error| json::Error::InvalidField {
                        field: field.to_owned(),
                        message: error.to_string(),
                    })?;
                let end = parser.position();
                if fields
                    .insert(
                        key.clone(),
                        SnapshotJsonField::Borrowed {
                            raw: &input[start..end],
                        },
                    )
                    .is_some()
                {
                    return Err(json::Error::InvalidField {
                        field: field.to_owned(),
                        message: format!("duplicate field `{key}`"),
                    });
                }
                source_order.push(key);
                parser.skip_ws();
                match parser.bump() {
                    Some(b',') => {}
                    Some(b'}') => break,
                    _ => {
                        return Err(json::Error::InvalidField {
                            field: field.to_owned(),
                            message: "expected comma or object end".to_owned(),
                        });
                    }
                }
            }
        }
        parser.skip_ws();
        if !parser.eof() {
            return Err(json::Error::InvalidField {
                field: field.to_owned(),
                message: "trailing bytes after snapshot object".to_owned(),
            });
        }
        Ok(Self {
            fields,
            source_order: Some(source_order),
        })
    }
    fn remove(&mut self, key: &str) -> Option<SnapshotJsonField<'a>> {
        self.fields.remove(key)
    }
    fn get(&self, key: &str) -> Option<&SnapshotJsonField<'a>> {
        self.fields.get(key)
    }
    fn contains_key(&self, key: &str) -> bool {
        self.fields.contains_key(key)
    }
    #[cfg(test)]
    fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }
    fn first_key(&self) -> Option<&str> {
        self.fields.keys().next().map(String::as_str)
    }
    fn require_source_order(&self, expected: &[&str], field: &str) -> Result<(), json::Error> {
        let Some(actual) = self.source_order.as_ref() else {
            return Ok(());
        };
        if let Some(unknown) = actual.iter().find(|key| !expected.contains(&key.as_str())) {
            return Err(json::Error::InvalidField {
                field: format!("{field}.{unknown}"),
                message: "unknown field is not permitted in a signed first-release snapshot"
                    .to_owned(),
            });
        }
        if actual
            .iter()
            .map(String::as_str)
            .eq(expected.iter().copied())
        {
            Ok(())
        } else {
            Err(json::Error::InvalidField {
                field: field.to_owned(),
                message: "snapshot object fields are not in canonical schema order".to_owned(),
            })
        }
    }
}
fn canonical_world_field_order() -> &'static [String] {
    static ORDER: OnceLock<Vec<String>> = OnceLock::new();
    ORDER.get_or_init(|| {
        let encoded = json::to_json(&World::default())
            .expect("default World must have a canonical JSON representation");
        SnapshotJsonMap::parse(&encoded, "default world")
            .expect("default World JSON must be a canonical object")
            .source_order
            .expect("borrowed default World JSON retains source order")
    })
}
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
    fn parse_trigger_set(self, value: SnapshotJsonField<'_>) -> Result<TriggerSet, json::Error> {
        value.decode_canonical("triggers")
    }
}
pub struct KuraSeed {
    pub kura: Arc<Kura>,
    pub query_handle: LiveQueryStoreHandle,
    #[cfg(feature = "telemetry")]
    pub telemetry: StateTelemetry,
}
impl KuraSeed {
    #[cfg(test)]
    pub fn into_state_from_json(self, value: json::Value) -> Result<State, json::Error> {
        self.into_state_from_json_with_recovery_mode(value, true)
    }
    /// Decode a canonical snapshot directly from its authenticated JSON bytes.
    ///
    /// The borrowed field map retains only schema keys and raw value slices;
    /// each field is decoded into its final typed owner before the next field,
    /// so restoration never constructs a recursive full-state JSON tree.
    pub(crate) fn into_state_from_json_str(self, input: &str) -> Result<State, json::Error> {
        let map = SnapshotJsonMap::parse(input, "state")?;
        self.into_state_from_snapshot_map(map, true)
    }
    /// Construct the deliberately minimal State authenticated by a compact emergency manifest.
    ///
    /// The caller has already authenticated the manifest signature. This constructor binds its
    /// exact height and terminal hash to Kura, maps the matching hash prefix read-only, and leaves
    /// World, transaction history, consensus topology, and runtime Nexus state unopened. The
    /// signed SCCP policy hash is retained separately because Fast mode never constructs the
    /// potentially large governed registry.
    pub(crate) fn into_state_from_emergency_fast_manifest(
        self,
        chain_id: ChainId,
        network_id: NetworkId,
        snapshot_height: usize,
        snapshot_tip: Option<HashOf<BlockHeader>>,
        sccp_policy_hash: [u8; 32],
    ) -> Result<State, json::Error> {
        let block_hashes =
            emergency_fast_block_hashes(self.kura.as_ref(), snapshot_height, snapshot_tip)?;
        let nexus = iroha_config::parameters::actual::Nexus::default();
        let lane_incarnations = derive_static_lane_incarnations(&nexus.lane_catalog);
        let lane_incarnation_activation_heights = lane_incarnations
            .keys()
            .copied()
            .map(|lane_id| (lane_id, 0))
            .collect::<BTreeMap<_, _>>();
        let lane_incarnation_lineage = lane_incarnations
            .iter()
            .map(|(&lane_id, &incarnation)| {
                (
                    lane_id,
                    LaneIncarnationLineage {
                        generation: 0,
                        incarnation,
                        activation_height: 0,
                    },
                )
            })
            .collect();
        let state = build_state(
            BuildStateInputs {
                world: World::default(),
                block_hashes,
                transactions: TransactionsStorage::new(),
                commit_topology: Cell::new(Vec::new()),
                prev_commit_topology: Cell::new(Vec::new()),
                ivm: IVM::new(0),
                nexus,
                lane_incarnations,
                lane_incarnation_activation_heights,
                lane_incarnation_lineage,
                autoscale_sample_history: VecDeque::new(),
                chain_id,
                network_id,
                snapshot_v2_bootstrap_candidate: None,
                nexus_runtime_restored_from_snapshot: false,
                kura: self.kura,
                query_handle: self.query_handle,
                #[cfg(feature = "telemetry")]
                telemetry: self.telemetry,
            },
            false,
            true,
        )
        .map_err(|error| json::Error::InvalidField {
            field: "state.durable_merge_ledger".to_owned(),
            message: error.to_string(),
        })?;
        state.install_emergency_fast_sccp_policy_hash(sccp_policy_hash);
        Ok(state)
    }
    /// Decode a State without loading, promoting, truncating, or otherwise
    /// recovering any durable Kura-adjacent journal.
    ///
    /// Replay prevalidation uses this constructor for an isolated dry run;
    /// its in-memory merge and query authority is populated explicitly
    /// from the already authenticated live State.
    /// Decode canonical snapshot bytes for isolated replay prevalidation.
    pub(crate) fn into_state_from_json_str_without_durable_recovery(
        self,
        input: &str,
    ) -> Result<State, json::Error> {
        let map = SnapshotJsonMap::parse(input, "state")?;
        self.into_state_from_snapshot_map(map, false)
    }
    #[cfg(test)]
    fn into_state_from_json_with_recovery_mode(
        self,
        value: json::Value,
        allow_durable_recovery: bool,
    ) -> Result<State, json::Error> {
        let json::Value::Object(map) = value else {
            return Err(json::Error::InvalidField {
                field: "state".into(),
                message: "expected object".into(),
            });
        };
        self.into_state_from_snapshot_map(SnapshotJsonMap::from_owned(map), allow_durable_recovery)
    }
    fn into_state_from_snapshot_map(
        self,
        mut map: SnapshotJsonMap<'_>,
        allow_durable_recovery: bool,
    ) -> Result<State, json::Error> {
        const WITHOUT_BOOTSTRAP: &[&str] = &[
            "chain_id",
            "network_id",
            "world",
            "nexus_runtime",
            "block_hashes",
            "transactions",
            "public_lane_validators",
            "public_lane_stake_shares",
            "public_lane_rewards",
            "public_lane_reward_claims",
            "space_directory_manifests",
            "commit_topology",
            "prev_commit_topology",
        ];
        const WITH_BOOTSTRAP: &[&str] = &[
            "chain_id",
            "network_id",
            "sumeragi_v2_bootstrap",
            "world",
            "nexus_runtime",
            "block_hashes",
            "transactions",
            "public_lane_validators",
            "public_lane_stake_shares",
            "public_lane_rewards",
            "public_lane_reward_claims",
            "space_directory_manifests",
            "commit_topology",
            "prev_commit_topology",
        ];
        let expected_order = if map.contains_key("sumeragi_v2_bootstrap") {
            WITH_BOOTSTRAP
        } else {
            WITHOUT_BOOTSTRAP
        };
        map.require_source_order(expected_order, "state")?;
        let world_value = map
            .remove("world")
            .ok_or_else(|| json::Error::missing_field("world"))?;
        let world_map = world_value.into_object("world")?;
        if !world_map.contains_key("contract_subject_bindings") {
            return Err(json::Error::missing_field(
                "world.contract_subject_bindings",
            ));
        }
        let ivm_runtime = IVM::new(0);
        let ivm_seed = IvmSeed {
            ivm: &ivm_runtime,
            _marker: PhantomData,
        };
        let mut world = parse_world(world_map, &ivm_seed, false)?;
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
        let snapshot_nexus_runtime: SnapshotNexusRuntime =
            take_required(&mut map, "nexus_runtime")?;
        let chain_id: ChainId = take_required(&mut map, "chain_id")?;
        let network_id: NetworkId = take_required(&mut map, "network_id")?;
        let world_view = world.view();
        for (_receipt_id, receipt) in world_view
            .soracloud_private_uploaded_model_execution_receipts()
            .iter()
        {
            if receipt.network_id != network_id {
                return Err(json::Error::InvalidField {
                    field: "state.world.soracloud_private_uploaded_model_execution_receipts"
                        .to_owned(),
                    message: "private receipt network_id must match the snapshot network_id"
                        .to_owned(),
                });
            }
        }
        drop(world_view);
        let block_hashes: Vec<HashOf<BlockHeader>> = take_required(&mut map, "block_hashes")?;
        let committed_height =
            u64::try_from(block_hashes.len()).map_err(|_| json::Error::InvalidField {
                field: "state.block_hashes".to_owned(),
                message: "committed height does not fit u64".to_owned(),
            })?;
        validate_replication_order_completion_anchors(&world, &block_hashes)?;
        validate_private_uploaded_model_execution_height_anchors(&world, committed_height)?;
        validate_musubi_resolver_checkpoint_anchors(&world, &block_hashes)?;
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
        ) = nexus_from_snapshot_runtime(snapshot_nexus_runtime, &block_hashes)?;
        let nexus_runtime_restored_from_snapshot = true;
        let transactions = take_required(&mut map, "transactions")?;
        let commit_topology = take_topology_cell(&mut map, "commit_topology")?;
        let prev_commit_topology = take_topology_cell(&mut map, "prev_commit_topology")?;
        let snapshot_v2_bootstrap_candidate: Option<SnapshotV2BootstrapRecord> =
            take_optional(&mut map, "sumeragi_v2_bootstrap")?;
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
        let public_lane_validator_records: Vec<PublicLaneValidatorRecord> =
            decode_snapshot_records(public_lane_validators, "public_lane_validators", true)?;
        let public_lane_stake_share_records: Vec<PublicLaneStakeShare> =
            decode_snapshot_records(public_lane_stake_shares, "public_lane_stake_shares", true)?;
        let public_lane_reward_records = decode_snapshot_records::<PublicLaneRewardRecord>(
            public_lane_rewards,
            "public_lane_rewards",
            true,
        )?;
        validate_canonical_snapshot_record_order(
            &public_lane_validator_records,
            "public_lane_validators",
            |record| (record.lane_id, record.validator.clone()),
        )?;
        validate_canonical_snapshot_record_order(
            &public_lane_stake_share_records,
            "public_lane_stake_shares",
            |record| {
                (
                    record.lane_id,
                    record.validator.clone(),
                    record.staker.clone(),
                )
            },
        )?;
        validate_canonical_snapshot_record_order(
            &public_lane_reward_records,
            "public_lane_rewards",
            |record| (record.lane_id, record.epoch),
        )?;
        validate_canonical_snapshot_record_order(
            &public_lane_reward_claims,
            "public_lane_reward_claims",
            |record| (record.lane_id, record.account.clone(), record.asset.clone()),
        )?;
        validate_canonical_snapshot_record_order(
            &space_directory_manifests,
            "space_directory_manifests",
            |record| record.uaid,
        )?;
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
        world.public_lane_rewards = public_lane_reward_records
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
            decode_space_directory_manifest_sets(space_directory_manifests, true)?;
        world
            .validate_quantity_ledger_invariants()
            .map_err(|message| json::Error::InvalidField {
                field: "state.world.numeric_ledgers".to_owned(),
                message,
            })?;
        let state = build_state(
            BuildStateInputs {
                world,
                block_hashes: BlockHashes::new(block_hashes),
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
                network_id,
                snapshot_v2_bootstrap_candidate,
                nexus_runtime_restored_from_snapshot,
                kura: self.kura,
                query_handle: self.query_handle,
                #[cfg(feature = "telemetry")]
                telemetry: self.telemetry,
            },
            allow_durable_recovery,
            false,
        )
        .map_err(|error| json::Error::InvalidField {
            field: "state.durable_merge_ledger".to_owned(),
            message: error.to_string(),
        })?;
        super::validate_sccp_state_local_profile(&state).map_err(|message| {
            json::Error::InvalidField {
                field: "state.world.sccp".to_owned(),
                message,
            }
        })?;
        Ok(state)
    }

}
fn emergency_fast_block_hashes(
    kura: &Kura,
    snapshot_height: usize,
    snapshot_tip: Option<HashOf<BlockHeader>>,
) -> Result<BlockHashes, json::Error> {
    let (durable_height, durable_tip) = kura
        .emergency_fast_snapshot_boundary(snapshot_height)
        .map_err(|error| json::Error::InvalidField {
            field: "state.block_hashes".to_owned(),
            message: format!("failed to bind the Kura Fast boundary: {error}"),
        })?;
    if durable_height != snapshot_height || durable_tip != snapshot_tip {
        return Err(json::Error::InvalidField {
            field: "state.block_hashes".to_owned(),
            message: format!(
                "snapshot boundary ({snapshot_height}, {snapshot_tip:?}) differs from durable Kura ({durable_height}, {durable_tip:?})"
            ),
        });
    }
    Ok(
        match kura
            .emergency_fast_snapshot_hash_mapping(snapshot_height)
            .map_err(|error| json::Error::InvalidField {
                field: "state.block_hashes".to_owned(),
                message: format!("failed to map the Kura Fast hash prefix: {error}"),
            })? {
            Some(mapping) => BlockHashes::new_emergency_fast_mapped(mapping, snapshot_height),
            None => BlockHashes::default(),
        },
    )
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
    if runtime
        .lanes
        .windows(2)
        .any(|pair| pair[0].id >= pair[1].id)
    {
        return Err(json::Error::InvalidField {
            field: "nexus_runtime.lanes".to_owned(),
            message: "lanes must be in strict canonical lane-id order".to_owned(),
        });
    }
    if runtime
        .lane_incarnation_lineage
        .windows(2)
        .any(|pair| pair[0].lane_id >= pair[1].lane_id)
    {
        return Err(json::Error::InvalidField {
            field: "nexus_runtime.lane_incarnation_lineage".to_owned(),
            message: "lineage entries must be in strict canonical lane-id order".to_owned(),
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
    validate_canonical: bool,
) -> Result<Vec<T>, json::Error>
where
    T: DecodeAll + Encode,
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
            let decoded = T::decode_all(&mut cursor).map_err(|err| json::Error::InvalidField {
                field: field.to_owned(),
                message: format!("record {index} norito decode failed: {err}"),
            })?;
            if validate_canonical {
                #[cfg(test)]
                SNAPSHOT_NORITO_CANONICAL_PASSES.with(|passes| passes.set(passes.get() + 1));
                if decoded.encode() != bytes {
                    return Err(json::Error::InvalidField {
                        field: field.to_owned(),
                        message: format!("record {index} is not canonical Norito"),
                    });
                }
            }
            Ok(decoded)
        })
        .collect()
}
fn validate_canonical_snapshot_record_order<T, K>(
    records: &[T],
    field: &str,
    key: impl Fn(&T) -> K,
) -> Result<(), json::Error>
where
    K: Ord,
{
    let mut previous = None;
    for (index, record) in records.iter().enumerate() {
        let current = key(record);
        if previous
            .as_ref()
            .is_some_and(|previous| previous >= &current)
        {
            return Err(json::Error::InvalidField {
                field: field.to_owned(),
                message: format!(
                    "record {index} is duplicated or not in canonical semantic key order"
                ),
            });
        }
        previous = Some(current);
    }
    Ok(())
}
fn decode_space_directory_manifest_sets(
    records: Vec<SnapshotSpaceDirectoryManifestSet>,
    validate_canonical: bool,
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
        if validate_canonical {
            #[cfg(test)]
            SNAPSHOT_NORITO_CANONICAL_PASSES.with(|passes| passes.set(passes.get() + 1));
            if manifest_set.encode() != bytes {
                return Err(json::Error::InvalidField {
                    field: "space_directory_manifests".to_owned(),
                    message: format!("record {index} is not canonical Norito"),
                });
            }
        }
        if storage.insert(record.uaid, manifest_set).is_some() {
            return Err(json::Error::InvalidField {
                field: "space_directory_manifests".to_owned(),
                message: format!("duplicate UAID at record {index}"),
            });
        }
    }
    Ok(storage)
}
fn take_required<T>(map: &mut SnapshotJsonMap<'_>, key: &str) -> Result<T, json::Error>
where
    T: JsonDeserialize + JsonSerialize,
{
    let value = map
        .remove(key)
        .ok_or_else(|| json::Error::missing_field(key))?;
    value.decode_canonical(key)
}
fn take_optional<T>(map: &mut SnapshotJsonMap<'_>, key: &str) -> Result<Option<T>, json::Error>
where
    T: JsonDeserialize + JsonSerialize,
{
    map.remove(key)
        .map(|value| value.decode_canonical(key))
        .transpose()
}
fn take_optional_default<T>(map: &mut SnapshotJsonMap<'_>, key: &str) -> Result<T, json::Error>
where
    T: JsonDeserialize + JsonSerialize + Default,
{
    map.remove(key)
        .map_or_else(|| Ok(T::default()), |value| value.decode_canonical(key))
}
fn take_musubi_namespace_bindings(
    map: &mut SnapshotJsonMap<'_>,
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
    map: &mut SnapshotJsonMap<'_>,
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
    map: &mut SnapshotJsonMap<'_>,
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
    map: &mut SnapshotJsonMap<'_>,
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
    map: &mut SnapshotJsonMap<'_>,
) -> Result<Storage<MusubiResolverIndexRevisionV1, MusubiRegistrySnapshotV1>, json::Error> {
    take_required(map, "musubi_resolver_index_checkpoints")
}
fn take_musubi_replication_shortfall_releases(
    map: &mut SnapshotJsonMap<'_>,
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
fn validate_capacity_declarations(
    declarations: &Storage<ProviderId, CapacityDeclarationRecord>,
    provider_owners: &Storage<ProviderId, AccountId>,
) -> Result<(), json::Error> {
    let provider_owners = provider_owners.view();
    let owner_metadata_key: Name = "sorafs.owner_account_id"
        .parse()
        .expect("static capacity owner metadata key");
    for (provider_id, record) in declarations.view().iter() {
        let provider_label = hex::encode(provider_id.as_bytes());
        if record.provider_id != *provider_id {
            return Err(json::Error::InvalidField {
                field: "world.capacity_declarations".to_owned(),
                message: format!(
                    "capacity declaration key {provider_label} does not match its stored provider"
                ),
            });
        }
        crate::smartcontracts::isi::sorafs::validate_stored_capacity_declaration(
            record,
            &provider_label,
        )
        .map_err(|error| json::Error::InvalidField {
            field: "world.capacity_declarations".to_owned(),
            message: error.to_string(),
        })?;
        let provider_owner = provider_owners.get(provider_id).ok_or_else(|| {
            json::Error::InvalidField {
                field: "world.capacity_declarations".to_owned(),
                message: format!(
                    "capacity declaration {provider_label} has no governance-established provider owner"
                ),
            }
        })?;
        let owner_literal = record.metadata.get(&owner_metadata_key).ok_or_else(|| {
            json::Error::InvalidField {
                field: "world.capacity_declarations".to_owned(),
                message: format!(
                    "capacity declaration {provider_label} omits metadata `sorafs.owner_account_id`"
                ),
            }
        })?;
        let owner_literal: String = owner_literal.try_into_any().map_err(|error| {
            json::Error::InvalidField {
                field: "world.capacity_declarations".to_owned(),
                message: format!(
                    "capacity declaration {provider_label} owner metadata must be a canonical account string: {error}"
                ),
            }
        })?;
        if owner_literal != provider_owner.to_string() {
            return Err(json::Error::InvalidField {
                field: "world.capacity_declarations".to_owned(),
                message: format!(
                    "capacity declaration {provider_label} owner metadata does not exactly match its governance-established provider owner"
                ),
            });
        }
    }
    Ok(())
}
fn validate_replication_order_completion_anchors(
    world: &World,
    block_hashes: &[HashOf<BlockHeader>],
) -> Result<(), json::Error> {
    for (order_id, order) in world.replication_orders.view().iter() {
        let order_label = hex::encode(order_id.as_bytes());
        for completion in &order.provider_completions {
            let height = completion.finalized_anchor.height;
            let index = usize::try_from(height)
                .ok()
                .and_then(|height| height.checked_sub(1))
                .ok_or_else(|| json::Error::InvalidField {
                    field: "state.world.replication_orders".to_owned(),
                    message: format!(
                        "replication order {order_label} completion for provider {} has a finalized anchor height outside the committed block prefix",
                        hex::encode(completion.provider_id.as_bytes()),
                    ),
                })?;
            let Some(committed_hash) = block_hashes.get(index) else {
                return Err(json::Error::InvalidField {
                    field: "state.world.replication_orders".to_owned(),
                    message: format!(
                        "replication order {order_label} completion for provider {} anchors unavailable committed height {height}",
                        hex::encode(completion.provider_id.as_bytes()),
                    ),
                });
            };
            if *committed_hash.as_ref() != completion.finalized_anchor.block_hash {
                return Err(json::Error::InvalidField {
                    field: "state.world.replication_orders".to_owned(),
                    message: format!(
                        "replication order {order_label} completion for provider {} finalized anchor hash does not match committed block height {height}",
                        hex::encode(completion.provider_id.as_bytes()),
                    ),
                });
            }
        }
    }
    Ok(())
}
fn validate_private_uploaded_model_execution_height_anchors(
    world: &World,
    committed_height: u64,
) -> Result<(), json::Error> {
    for ((service_name, request_id), claim) in world
        .soracloud_private_uploaded_model_execution_claims
        .view()
        .iter()
    {
        if claim.claimed_block_height > committed_height {
            return Err(json::Error::InvalidField {
                field: "state.world.soracloud_private_uploaded_model_execution_claims".to_owned(),
                message: format!(
                    "private execution claim {service_name}/{request_id} anchors future block height {} beyond snapshot committed height {committed_height}",
                    claim.claimed_block_height,
                ),
            });
        }
    }
    for (receipt_id, receipt) in world
        .soracloud_private_uploaded_model_execution_receipts
        .view()
        .iter()
    {
        if receipt.authorization_claim_block_height > committed_height {
            return Err(json::Error::InvalidField {
                field: "state.world.soracloud_private_uploaded_model_execution_receipts".to_owned(),
                message: format!(
                    "private execution receipt {receipt_id} anchors future authorization-claim block height {} beyond snapshot committed height {committed_height}",
                    receipt.authorization_claim_block_height,
                ),
            });
        }
        if receipt.emitted_block_height > committed_height {
            return Err(json::Error::InvalidField {
                field: "state.world.soracloud_private_uploaded_model_execution_receipts".to_owned(),
                message: format!(
                    "private execution receipt {receipt_id} anchors future emission block height {} beyond snapshot committed height {committed_height}",
                    receipt.emitted_block_height,
                ),
            });
        }
    }
    Ok(())
}
fn validate_automatic_replication_capacity_state(
    declarations: &Storage<ProviderId, CapacityDeclarationRecord>,
    provider_owners: &Storage<ProviderId, AccountId>,
    completion_authorities: &Storage<ProviderId, ProviderIngestCompletionAuthorityV1>,
    pin_manifests: &Storage<ManifestDigest, PinManifestRecord>,
    replication_orders: &Storage<ReplicationOrderId, ReplicationOrderRecord>,
) -> Result<(), json::Error> {
    let invalid = |message: String| json::Error::InvalidField {
        field: "world.replication_orders".to_owned(),
        message,
    };
    let declarations = declarations.view();
    let provider_owners = provider_owners.view();
    let completion_authorities = completion_authorities.view();
    let pin_manifests = pin_manifests.view();
    let mut allocations = BTreeMap::<(ProviderId, String), u64>::new();
    for (order_id, order) in replication_orders.view().iter() {
        if !order_id.is_auto() {
            continue;
        }
        let order_label = hex::encode(order_id.as_bytes());
        let pin = pin_manifests.get(&order.manifest_digest).ok_or_else(|| {
            invalid(format!(
                "automatic replication order {order_label} references a missing pin manifest"
            ))
        })?;
        let payload =
            crate::smartcontracts::isi::sorafs::validate_stored_automatic_replication_order(
                pin,
                order,
                &order_label,
            )
            .map_err(|error| invalid(error.to_string()))?;
        if !matches!(pin.status, PinStatus::Approved(_))
            || !matches!(
                order.status,
                ReplicationOrderStatus::Pending | ReplicationOrderStatus::Completed(_)
            )
        {
            continue;
        }
        for assignment in &payload.assignments {
            let provider_id = ProviderId::new(assignment.provider_id);
            let provider_label = hex::encode(provider_id.as_bytes());
            let declaration = declarations.get(&provider_id).ok_or_else(|| {
                invalid(format!(
                    "automatic replication order {order_label} assigns provider {provider_label} without a retained capacity declaration"
                ))
            })?;
            let Some(profile_capacity) =
                crate::smartcontracts::isi::sorafs::automatic_replication_profile_capacity_gib(
                    declaration,
                    pin,
                    order.issued_epoch,
                    order.deadline_epoch,
                )
                .map_err(|error| invalid(error.to_string()))?
            else {
                return Err(invalid(format!(
                    "automatic replication order {order_label} assigns provider {provider_label} without exact profile, storage-class, and deadline capacity"
                )));
            };
            let provider_owner = provider_owners.get(&provider_id).ok_or_else(|| {
                invalid(format!(
                    "automatic replication order {order_label} assigns provider {provider_label} without a governed owner"
                ))
            })?;
            // A retained completion is immutable self-contained evidence and remains valid across
            // a later governed owner rotation. Only an assignment that still needs completion
            // depends on the current owner-bound authority.
            if order.provider_completion(provider_id).is_none()
                && !completion_authorities
                    .get(&provider_id)
                    .is_some_and(|authority| {
                        authority.is_valid() && &authority.provider_owner == provider_owner
                    })
            {
                return Err(invalid(format!(
                    "pending automatic replication order {order_label} assigns provider {provider_label} without a valid owner-bound completion authority"
                )));
            }
            let allocated = allocations
                .entry((provider_id, payload.chunking_profile.clone()))
                .or_default();
            *allocated = allocated.checked_add(assignment.slice_gib).ok_or_else(|| {
                invalid(format!(
                    "automatic replication allocation overflowed for provider {provider_label}"
                ))
            })?;
            if *allocated > profile_capacity {
                return Err(invalid(format!(
                    "automatic replication allocations oversubscribe provider {provider_label} profile `{}`: allocated {} GiB, committed {profile_capacity} GiB",
                    payload.chunking_profile, *allocated
                )));
            }
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
    map: &mut SnapshotJsonMap<'_>,
) -> Result<Storage<RamLfeProgramId, RamLfeProgramPolicy>, json::Error> {
    take_required(map, "ram_lfe_program_policies")
}
fn take_parameters_cell(
    map: &mut SnapshotJsonMap<'_>,
    key: &str,
) -> Result<Cell<Parameters>, json::Error> {
    take_required(map, key)
}
fn take_topology_cell(
    map: &mut SnapshotJsonMap<'_>,
    key: &str,
) -> Result<Cell<Vec<PeerId>>, json::Error> {
    let value = map
        .remove(key)
        .ok_or_else(|| json::Error::missing_field(key))?;
    match value {
        SnapshotJsonField::Borrowed { raw } if raw.as_bytes().first() == Some(&b'[') => {
            SnapshotJsonField::Borrowed { raw }
                .decode_canonical(key)
                .map(Cell::new)
        }
        #[cfg(test)]
        SnapshotJsonField::Owned(json::Value::Array(values)) => {
            SnapshotJsonField::Owned(json::Value::Array(values))
                .decode_canonical(key)
                .map(Cell::new)
        }
        other => other.decode_canonical(key),
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
#[allow(clippy::too_many_lines)]
pub(crate) fn validate_musubi_location_reverse_indices(
    archives: &Storage<ArchiveId, MusubiArchiveRecordV1>,
    locations: &Storage<MusubiArchiveLocationKeyV1, MusubiArchiveLocationV1>,
    pin_manifests: &Storage<ManifestDigest, PinManifestRecord>,
    replication_orders: &Storage<ReplicationOrderId, ReplicationOrderRecord>,
    by_pin: &Storage<ManifestDigest, MusubiPinLocationReferenceV1>,
    by_order: &Storage<ReplicationOrderId, MusubiReplicationOrderLocationReferenceV1>,
    by_provider: &Storage<MusubiProviderLocationKeyV1, ()>,
) -> Result<(), json::Error> {
    let invalid = |message: String| json::Error::InvalidField {
        field: "world.musubi_location_reverse_indices".to_owned(),
        message,
    };
    let archives = archives.view();
    let locations = locations.view();
    let pin_manifests = pin_manifests.view();
    let replication_orders = replication_orders.view();
    let by_pin = by_pin.view();
    let by_order = by_order.view();
    let by_provider = by_provider.view();
    for (order, record) in replication_orders.iter() {
        let pin = pin_manifests
            .get(&record.manifest_digest)
            .ok_or_else(|| invalid("replication order targets a missing pin manifest".into()))?;
        let order_label = hex::encode(order.as_bytes());
        let approved_epoch =
            crate::smartcontracts::isi::sorafs::validate_stored_pin_approval_history(
                pin,
                &hex::encode(pin.digest.as_bytes()),
            )
            .map_err(|error| invalid(error.to_string()))?
            .ok_or_else(|| {
                invalid(format!(
                    "replication order {order_label} targets a pin that was never approved"
                ))
            })?;
        if record.issued_epoch < approved_epoch {
            return Err(invalid(format!(
                "replication order {order_label} predates its target pin approval epoch {approved_epoch}"
            )));
        }
        if let PinStatus::Retired(retired_epoch) = pin.status {
            if record.issued_epoch > retired_epoch
                || matches!(record.status, ReplicationOrderStatus::Pending)
                || matches!(record.status, ReplicationOrderStatus::Completed(epoch) | ReplicationOrderStatus::Expired(epoch) if epoch > retired_epoch)
                || order.is_auto()
                    && matches!(record.status, ReplicationOrderStatus::Completed(_))
                    && retired_epoch < pin.policy.retention_epoch
            {
                return Err(invalid(format!(
                    "replication order {order_label} lifecycle falls outside its target pin retirement epoch {retired_epoch}"
                )));
            }
        }
        let canonical_order = if order.is_auto() {
            crate::smartcontracts::isi::sorafs::validate_stored_automatic_replication_order(
                pin,
                record,
                &order_label,
            )
        } else {
            crate::smartcontracts::isi::sorafs::validate_stored_replication_order(
                record,
                &order_label,
            )
        }
        .map_err(|error| invalid(error.to_string()))?;
        if record.order_id != *order
            || pin.digest != record.manifest_digest
            || pin.root_cid != record.manifest_root_cid
            || canonical_order.chunking_profile != pin.chunker.to_handle()
            || canonical_order.target_replicas < pin.policy.min_replicas
            || record.deadline_epoch >= pin.policy.retention_epoch
        {
            return Err(invalid(
                "replication order does not match its immutable pin commitment or retention policy"
                    .into(),
            ));
        }
        if let ReplicationOrderStatus::Cancelled(cancelled_epoch) = record.status
            && !matches!(pin.status, PinStatus::Retired(retired_epoch) if retired_epoch == cancelled_epoch)
        {
            return Err(invalid(
                "cancelled replication order must exactly match its target pin retirement epoch"
                    .into(),
            ));
        }
        let reference = by_order.get(order);
        match (record.musubi_archive, reference) {
            (None, None) => {}
            (Some(archive_id), Some(reference))
                if reference.binding.replication_order == *order
                    && reference.binding.archive_id == archive_id => {}
            (Some(_), None) => {
                return Err(invalid(
                    "Musubi-purpose replication order is missing its archive binding".into(),
                ));
            }
            (None, Some(_)) => {
                return Err(invalid(
                    "generic replication order cannot carry a Musubi archive binding".into(),
                ));
            }
            (Some(_), Some(_)) => {
                return Err(invalid(
                    "replication-order Musubi purpose does not match its archive binding".into(),
                ));
            }
        }
    }
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
        if order != &reference.binding.replication_order {
            return Err(invalid(
                "order reverse-index key does not match its duplicated order identity".into(),
            ));
        }
        let archive = archives
            .get(&reference.binding.archive_id)
            .ok_or_else(|| invalid("order binding targets a missing Musubi archive".into()))?;
        archive
            .validate()
            .map_err(|error| invalid(error.to_string()))?;
        if archive.archive_id != reference.binding.archive_id
            || archive.commitment != reference.binding.commitment
        {
            return Err(invalid(
                "order binding does not match the authoritative archive commitment".into(),
            ));
        }
        let order_record = replication_orders
            .get(order)
            .ok_or_else(|| invalid("order binding targets a missing replication order".into()))?;
        if order_record.order_id != *order
            || order_record.musubi_archive != Some(reference.binding.archive_id)
            || order_record.manifest_root_cid != archive.commitment.root_cid
        {
            return Err(invalid(
                "order binding does not match its authoritative replication order".into(),
            ));
        }
        let canonical_order =
            crate::smartcontracts::isi::sorafs::validate_stored_replication_order(
                order_record,
                &hex::encode(order.as_bytes()),
            )
            .map_err(|error| invalid(error.to_string()))?;
        let pin = pin_manifests
            .get(&order_record.manifest_digest)
            .ok_or_else(|| {
                invalid("order binding replication order targets a missing pin manifest".into())
            })?;
        if pin.digest != order_record.manifest_digest
            || pin.root_cid != archive.commitment.root_cid
            || pin.chunker != archive.commitment.chunker
            || pin.chunk_digest_sha3_256 != *archive.commitment.chunk_plan_digest.as_bytes()
            || pin.por_root != *archive.commitment.por_root.as_bytes()
            || pin.content_length != archive.commitment.content_length
            || canonical_order.chunking_profile != pin.chunker.to_handle()
            || canonical_order.target_replicas
                < iroha_data_model::musubi::MUSUBI_MIN_HEALTHY_REPLICAS_V1
            || canonical_order.target_replicas < pin.policy.min_replicas
            || pin.policy.min_replicas < iroha_data_model::musubi::MUSUBI_MIN_HEALTHY_REPLICAS_V1
            || order_record.deadline_epoch >= pin.policy.retention_epoch
        {
            return Err(invalid(
                "order binding does not match its immutable pin commitment or retention policy"
                    .into(),
            ));
        }
        let mut completed_providers = order_record
            .provider_completions
            .iter()
            .map(|completion| completion.provider_id)
            .collect::<Vec<_>>();
        completed_providers.sort();
        match &reference.lifecycle {
            MusubiReplicationOrderLocationLifecycleV1::PreLocation => {}
            MusubiReplicationOrderLocationLifecycleV1::Active(location) => {
                let target = locations.get(location).ok_or_else(|| {
                    invalid("active order binding targets a missing location".into())
                })?;
                if !matches!(
                    target.state,
                    MusubiArchiveLocationStateV1::Healthy | MusubiArchiveLocationStateV1::Degraded
                ) || target.archive_id != archive.archive_id
                    || target.replication_order != *order
                    || !matches!(
                        order_record.status,
                        iroha_data_model::sorafs::pin_registry::ReplicationOrderStatus::Completed(
                            _
                        )
                    )
                    || completed_providers != target.providers
                {
                    return Err(invalid(
                        "active order binding does not match its completed provider location"
                            .into(),
                    ));
                }
            }
            MusubiReplicationOrderLocationLifecycleV1::Retired(retired) => {
                let target = locations.get(&retired.location).ok_or_else(|| {
                    invalid("retired order binding targets a missing location".into())
                })?;
                if target.archive_id != archive.archive_id
                    || (target.state != MusubiArchiveLocationStateV1::Retired
                        && target.replication_order == *order)
                    || !matches!(
                        order_record.status,
                        iroha_data_model::sorafs::pin_registry::ReplicationOrderStatus::Completed(
                            _
                        )
                    )
                    || completed_providers != retired.providers
                    || (target.state == MusubiArchiveLocationStateV1::Retired
                        && target.replication_order == *order
                        && target.providers != retired.providers)
                {
                    return Err(invalid(
                        "retired order binding does not match its completed historical location"
                            .into(),
                    ));
                }
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
                    .is_some_and(|reference| reference.retired_location() == Some(*key))
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
                .is_some_and(|reference| reference.active_location() == Some(*key))
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
    for (manifest_digest, pin) in pin_manifests.iter() {
        if manifest_digest != &pin.digest {
            return Err(invalid(
                "pin-manifest key does not match its embedded manifest digest".into(),
            ));
        }
        let approval_epoch =
            crate::smartcontracts::isi::sorafs::validate_stored_pin_approval_history(
                pin,
                &hex::encode(manifest_digest.as_bytes()),
            )
            .map_err(|error| invalid(error.to_string()))?;
        let expected_order_id =
            iroha_data_model::sorafs::pin_registry::derive_sorafs_auto_replication_order_id_v1(
                &pin.digest,
            );
        if approval_epoch.is_none() {
            if replication_orders.get(&expected_order_id).is_some() {
                return Err(invalid(format!(
                    "never-approved pin manifest {} has an automatic replication order {}",
                    hex::encode(manifest_digest.as_bytes()),
                    hex::encode(expected_order_id.as_bytes()),
                )));
            }
            continue;
        }
        let record = replication_orders.get(&expected_order_id).ok_or_else(|| {
            invalid(format!(
                "approved pin history for manifest {} is missing its mandatory automatic replication order {}",
                hex::encode(manifest_digest.as_bytes()),
                hex::encode(expected_order_id.as_bytes()),
            ))
        })?;
        crate::smartcontracts::isi::sorafs::validate_stored_automatic_replication_order(
            pin,
            record,
            &hex::encode(expected_order_id.as_bytes()),
        )
        .map_err(|error| invalid(error.to_string()))?;
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
