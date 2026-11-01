//! Field-level view and comparison of Nexus/AMX context preimages.
//!
//! A node whose own context differs from the one its signed genesis commits sees only two
//! hashes. [`nexus_amx_context_fields_v1`] walks the exact grammar of
//! [`super::decode_nexus_amx_context_v1`] and names every field by the node configuration key it
//! projects, or by its projection tag when genesis execution or the protocol fixes it, with a canonical rendering
//! of the value. [`diff_nexus_amx_context_v1`] compares two preimages field by field. Lanes,
//! dataspaces and lane lineage are matched by identity and routing rules by position, so an added
//! or removed entry is reported once instead of misaligning every later field.
use super::{
    Fields, LaneConsensusProjectionV1, NexusAmxContextCatalogV1, NexusAmxContextError,
    decode_nexus_amx_context_v1, tag,
};
use crate::{block::consensus::GenesisActiveNexusLaneRecord, nexus::FeeSponsorProgramId};
use iroha_crypto::Hash;
use iroha_model_base::topology::{DataSpaceId, LaneId};
use norito::codec::{Decode, Encode};
use std::collections::{BTreeMap, BTreeSet};

/// Where the value of a Nexus/AMX context field comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NexusAmxContextFieldOriginV1 {
    /// A key of the node configuration; the field path names it.
    NodeConfig,
    /// Fixed by genesis execution or the protocol: the root fee policy and pipeline validity policy it commits
    /// (or their protocol defaults), the public-lane validators and lane lineage it registers,
    /// the autoscale transition height and reserved metadata, and the lane projection version.
    /// These are not node-configuration keys; paths identify their projected fields.
    Genesis,
}

/// One field of a Nexus/AMX context preimage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NexusAmxContextFieldV1 {
    /// Configuration path such as `nexus.fusion.exit_teu`. Group members carry their identity
    /// (`nexus.dataspace_catalog[id=9].alias`) or position (`nexus.routing_policy.rules[0].lane`).
    pub path: String,
    /// Where the value comes from.
    pub origin: NexusAmxContextFieldOriginV1,
    /// Canonical rendering of the value.
    pub value: String,
    group: Option<Group>,
    encoded: Vec<u8>,
}

/// One field whose value differs between two Nexus/AMX context preimages.
///
/// A lane, dataspace, lineage entry or routing rule present in only one preimage is reported
/// once, under its group path, with a summary of the entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NexusAmxContextFieldDiffV1 {
    /// Field or group path.
    pub path: String,
    /// Where the value comes from.
    pub origin: NexusAmxContextFieldOriginV1,
    /// Rendering in the expected preimage, `None` when absent there.
    pub expected: Option<String>,
    /// Rendering in the actual preimage, `None` when absent there.
    pub actual: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Group {
    path: String,
    summary: String,
    origin: NexusAmxContextFieldOriginV1,
}

/// List every field of one exact Nexus/AMX context preimage in grammar order.
///
/// # Errors
/// Any refusal of [`decode_nexus_amx_context_v1`], a field that is not the canonical encoding of
/// its projected type, or a field path that occurs twice.
pub fn nexus_amx_context_fields_v1(
    preimage: &[u8],
) -> Result<Vec<NexusAmxContextFieldV1>, NexusAmxContextError> {
    let catalog = decode_nexus_amx_context_v1(preimage)?;
    let mut walk = Walk {
        fields: Fields::tokenize(preimage)?,
        catalog: &catalog,
        out: Vec::new(),
    };
    walk.catalog_fields()?;
    walk.tail_fields()?;
    walk.fields.finish()?;
    let mut paths = BTreeSet::new();
    if !walk
        .out
        .iter()
        .all(|field| paths.insert(field.path.as_str()))
    {
        return Err(NexusAmxContextError::Structure("a field path occurs twice"));
    }
    Ok(walk.out)
}

/// Compare two exact Nexus/AMX context preimages field by field.
///
/// The result follows the order of `expected`, then the fields present only in `actual`.
/// Equal preimages yield no differences.
///
/// # Errors
/// Either preimage is refused by [`nexus_amx_context_fields_v1`].
pub fn diff_nexus_amx_context_v1(
    expected: &[u8],
    actual: &[u8],
) -> Result<Vec<NexusAmxContextFieldDiffV1>, NexusAmxContextError> {
    let expected = nexus_amx_context_fields_v1(expected)?;
    let actual = nexus_amx_context_fields_v1(actual)?;
    let mut diffs = Vec::new();
    let mut reported_groups = BTreeSet::new();
    for (side, other, side_is_expected) in [(&expected, &actual, true), (&actual, &expected, false)]
    {
        let other_paths = other
            .iter()
            .map(|field| (field.path.as_str(), field))
            .collect::<BTreeMap<_, _>>();
        let other_groups = other
            .iter()
            .filter_map(|field| field.group.as_ref().map(|group| group.path.as_str()))
            .collect::<BTreeSet<_>>();
        for field in side {
            let one_sided = |path: &str, value: &str| {
                let value = Some(value.to_owned());
                let (expected, actual) = if side_is_expected {
                    (value, None)
                } else {
                    (None, value)
                };
                NexusAmxContextFieldDiffV1 {
                    path: path.to_owned(),
                    origin: field.origin,
                    expected,
                    actual,
                }
            };
            match &field.group {
                Some(group) if !other_groups.contains(group.path.as_str()) => {
                    if reported_groups.insert(group.path.as_str()) {
                        let mut diff = one_sided(&group.path, &group.summary);
                        diff.origin = group.origin;
                        diffs.push(diff);
                    }
                }
                _ => match other_paths.get(field.path.as_str()) {
                    None => diffs.push(one_sided(&field.path, &field.value)),
                    Some(other) if side_is_expected && other.encoded != field.encoded => {
                        diffs.push(NexusAmxContextFieldDiffV1 {
                            path: field.path.clone(),
                            origin: field.origin,
                            expected: Some(field.value.clone()),
                            actual: Some(other.value.clone()),
                        });
                    }
                    Some(_) => {}
                },
            }
        }
    }
    Ok(diffs)
}

struct Walk<'a> {
    fields: Fields<'a>,
    catalog: &'a NexusAmxContextCatalogV1,
    out: Vec<NexusAmxContextFieldV1>,
}

impl Walk<'_> {
    fn push<T: Encode>(
        &mut self,
        path: String,
        origin: NexusAmxContextFieldOriginV1,
        group: Option<&Group>,
        value: &T,
        rendered: String,
    ) {
        self.out.push(NexusAmxContextFieldV1 {
            path,
            origin,
            value: rendered,
            group: group.cloned(),
            encoded: value.encode(),
        });
    }

    /// Decode, record and return one canonical field.
    fn scalar<T: Decode + Encode>(
        &mut self,
        tag: &'static str,
        path: String,
        origin: NexusAmxContextFieldOriginV1,
        group: Option<&Group>,
        render: impl FnOnce(&T, &NexusAmxContextCatalogV1) -> String,
    ) -> Result<T, NexusAmxContextError> {
        let value = self.fields.value::<T>(tag)?;
        let rendered = render(&value, self.catalog);
        self.push(path, origin, group, &value, rendered);
        Ok(value)
    }

    /// Record one genesis-fixed field by its length and hash.
    fn opaque(&mut self, path: &str, bytes: &[u8], prefix: &str) {
        self.out.push(NexusAmxContextFieldV1 {
            path: path.to_owned(),
            origin: NexusAmxContextFieldOriginV1::Genesis,
            value: format!("{prefix}{} bytes, hash {}", bytes.len(), Hash::new(bytes)),
            group: None,
            encoded: bytes.to_vec(),
        });
    }

    fn catalog_fields(&mut self) -> Result<(), NexusAmxContextError> {
        use NexusAmxContextFieldOriginV1::{Genesis, NodeConfig};
        let catalog = self.catalog;
        self.scalar::<u32>(
            tag::LANE_COUNT,
            "nexus.lane_count".to_owned(),
            NodeConfig,
            None,
            |count, _| count.to_string(),
        )?;
        for lane in &self
            .fields
            .value::<Vec<LaneConsensusProjectionV1>>(tag::LANES)?
        {
            self.lane(lane);
        }
        for _ in 0..self.fields.count(tag::LANE_LIFECYCLE_COUNT, 4)? {
            let lane = self.fields.value::<LaneId>(tag::LANE_LIFECYCLE_LANE_ID)?;
            let generation = self.fields.value::<u64>(tag::LANE_LIFECYCLE_GENERATION)?;
            let incarnation = self.fields.value::<Hash>(tag::LANE_LIFECYCLE_INCARNATION)?;
            let height = self
                .fields
                .value::<u64>(tag::LANE_LIFECYCLE_ACTIVATION_HEIGHT)?;
            let group = Group {
                path: format!("nexus.lane_lifecycle[lane={lane}]"),
                origin: Genesis,
                summary: format!(
                    "lane {}, generation {generation}, incarnation {incarnation}, activation_height {height}",
                    render_lane(lane, catalog)
                ),
            };
            let field = |name: &str| format!("{}.{name}", group.path);
            self.push(
                field("generation"),
                Genesis,
                Some(&group),
                &generation,
                generation.to_string(),
            );
            self.push(
                field("incarnation"),
                Genesis,
                Some(&group),
                &incarnation,
                incarnation.to_string(),
            );
            self.push(
                field("activation_height"),
                Genesis,
                Some(&group),
                &height,
                height.to_string(),
            );
        }
        for _ in 0..self.fields.count(tag::DATASPACE_COUNT, 3)? {
            let id = self.fields.value::<DataSpaceId>(tag::DATASPACE_ID)?;
            let alias = self.fields.value::<String>(tag::DATASPACE_ALIAS)?;
            let fault_tolerance = self.fields.value::<u32>(tag::DATASPACE_FAULT_TOLERANCE)?;
            let group = dataspace_group(id, catalog);
            self.push(
                format!("{}.alias", group.path),
                NodeConfig,
                Some(&group),
                &alias,
                format!("{alias:?}"),
            );
            self.push(
                format!("{}.fault_tolerance", group.path),
                NodeConfig,
                Some(&group),
                &fault_tolerance,
                fault_tolerance.to_string(),
            );
        }
        self.scalar::<LaneId>(
            tag::ROUTING_DEFAULT_LANE,
            "nexus.routing_policy.default_lane".to_owned(),
            NodeConfig,
            None,
            |lane, catalog| render_lane(*lane, catalog),
        )?;
        self.scalar::<DataSpaceId>(
            tag::ROUTING_DEFAULT_DATASPACE,
            "nexus.routing_policy.default_dataspace".to_owned(),
            NodeConfig,
            None,
            |id, catalog| render_dataspace(*id, catalog),
        )?;
        for (index, rule) in
            (0..self.fields.count(tag::ROUTING_RULE_COUNT, 4)?).zip(catalog.rules())
        {
            let group = Group {
                path: format!("nexus.routing_policy.rules[{index}]"),
                origin: NodeConfig,
                summary: format!(
                    "lane {}, dataspace {}, account {}, instruction {}",
                    render_lane(rule.lane(), catalog),
                    render_optional_dataspace(rule.dataspace(), catalog),
                    render_optional_string(rule.account()),
                    render_optional_string(rule.instruction()),
                ),
            };
            let field = |name: &str| format!("{}.{name}", group.path);
            self.scalar::<LaneId>(
                tag::ROUTING_RULE_LANE,
                field("lane"),
                NodeConfig,
                Some(&group),
                |lane, catalog| render_lane(*lane, catalog),
            )?;
            self.scalar::<Option<DataSpaceId>>(
                tag::ROUTING_RULE_DATASPACE,
                field("dataspace"),
                NodeConfig,
                Some(&group),
                |id, catalog| render_optional_dataspace(*id, catalog),
            )?;
            self.scalar::<Option<String>>(
                tag::ROUTING_RULE_ACCOUNT,
                field("matcher.account"),
                NodeConfig,
                Some(&group),
                |account, _| render_optional_string(account.as_deref()),
            )?;
            self.scalar::<Option<String>>(
                tag::ROUTING_RULE_INSTRUCTION,
                field("matcher.instruction"),
                NodeConfig,
                Some(&group),
                |instruction, _| render_optional_string(instruction.as_deref()),
            )?;
        }
        Ok(())
    }

    fn lane(&mut self, lane: &LaneConsensusProjectionV1) {
        use NexusAmxContextFieldOriginV1::{Genesis, NodeConfig};
        let group = Group {
            path: format!("nexus.lane_catalog[index={}]", lane.id),
            origin: NodeConfig,
            summary: format!(
                "alias {:?}, dataspace {}, visibility {}",
                lane.alias,
                render_dataspace(lane.dataspace_id, self.catalog),
                lane.visibility
            ),
        };
        let field = |name: &str| format!("{}.{name}", group.path);
        let dataspace = render_dataspace(lane.dataspace_id, self.catalog);
        let group = Some(&group);
        self.push(
            field("version"),
            Genesis,
            group,
            &lane.version,
            lane.version.to_string(),
        );
        self.push(
            field("alias"),
            NodeConfig,
            group,
            &lane.alias,
            format!("{:?}", lane.alias),
        );
        self.push(
            field("shard_id"),
            NodeConfig,
            group,
            &lane.shard_id,
            lane.shard_id.to_string(),
        );
        self.push(
            field("dataspace"),
            NodeConfig,
            group,
            &lane.dataspace_id,
            dataspace,
        );
        self.push(
            field("visibility"),
            NodeConfig,
            group,
            &lane.visibility,
            lane.visibility.to_string(),
        );
        self.push(
            field("lane_type"),
            NodeConfig,
            group,
            &lane.lane_type,
            render_optional_string(lane.lane_type.as_deref()),
        );
        self.push(
            field("governance"),
            NodeConfig,
            group,
            &lane.governance,
            render_optional_string(lane.governance.as_deref()),
        );
        self.push(
            field("settlement"),
            NodeConfig,
            group,
            &lane.settlement,
            render_optional_string(lane.settlement.as_deref()),
        );
        self.push(
            field("storage"),
            NodeConfig,
            group,
            &lane.storage,
            lane.storage.as_str().to_owned(),
        );
        self.push(
            field("proof_scheme"),
            NodeConfig,
            group,
            &lane.proof_scheme,
            lane.proof_scheme.as_str().to_owned(),
        );
        self.push(
            field("manifest_policy"),
            NodeConfig,
            group,
            &lane.manifest_policy,
            lane.manifest_policy.as_str().to_owned(),
        );
        self.push(
            field("confidential_compute"),
            NodeConfig,
            group,
            &lane.confidential_compute,
            format!("{:?}", lane.confidential_compute),
        );
        self.push(
            field("scheduler"),
            NodeConfig,
            group,
            &lane.scheduler,
            format!("{:?}", lane.scheduler),
        );
        self.push(
            field("settlement_buffer"),
            NodeConfig,
            group,
            &lane.settlement_buffer,
            format!("{:?}", lane.settlement_buffer),
        );
        self.push(
            field("metadata"),
            Genesis,
            group,
            &lane.consensus_metadata,
            format!("{:?}", lane.consensus_metadata),
        );
    }

    fn tail_fields(&mut self) -> Result<(), NexusAmxContextError> {
        use NexusAmxContextFieldOriginV1::{Genesis, NodeConfig};
        use tag::*;
        let integer = |value: &u64, _: &NexusAmxContextCatalogV1| value.to_string();
        for tag in TAIL {
            let path = tag.to_owned();
            match tag {
                STAKING_PUBLIC_VALIDATOR_MODE | STAKING_RESTRICTED_VALIDATOR_MODE => {
                    self.scalar::<u8>(tag, path, NodeConfig, None, |mode, _| match mode {
                        0 => "\"stake_elected\"".to_owned(),
                        1 => "\"admin_managed\"".to_owned(),
                        other => format!("mode {other}"),
                    })?;
                }
                ROOT_FEE_POLICY => {
                    let bytes = self.fields.expect(tag)?;
                    self.opaque(tag, bytes, "");
                }
                STAGED_ACTIVE_PUBLIC_LANE_VALIDATORS => {
                    let bytes = self.fields.expect(tag)?;
                    let records =
                        norito::codec::decode_adaptive::<Vec<GenesisActiveNexusLaneRecord>>(bytes)
                            .ok()
                            .filter(|records| records.encode() == bytes)
                            .ok_or(NexusAmxContextError::NonCanonical(tag))?;
                    self.opaque(tag, bytes, &format!("{} records, ", records.len()));
                }
                DATASPACE_FEE_SPONSOR_PROGRAM_IDS => {
                    let programs = self
                        .fields
                        .value::<BTreeMap<DataSpaceId, FeeSponsorProgramId>>(tag)?;
                    for (id, program) in &programs {
                        let group = dataspace_group(*id, self.catalog);
                        self.push(
                            format!("{}.fee_sponsor_program_id", group.path),
                            NodeConfig,
                            Some(&group),
                            program,
                            format!("\"{program}\""),
                        );
                    }
                }
                STAKING_MAX_VALIDATORS
                | FUSION_FLOOR_TEU
                | FUSION_EXIT_TEU
                | AUTOSCALE_MIN_LANE_ID
                | AUTOSCALE_MAX_LANE_ID_EXCLUSIVE
                | AUTOSCALE_PER_LANE_TARGET_TPS
                | DA_Q_IN_SLOT_TOTAL => {
                    self.scalar::<u32>(tag, path, NodeConfig, None, |value, _| value.to_string())?;
                }
                FUSION_OBSERVATION_SLOTS
                | FUSION_MAX_WINDOW_SLOTS
                | AUTOSCALE_SCALE_OUT_WINDOW_BLOCKS
                | AUTOSCALE_SCALE_IN_WINDOW_BLOCKS
                | AUTOSCALE_COOLDOWN_BLOCKS
                | COMMIT_WINDOW_SLOTS
                | DA_Q_IN_SLOT_PER_DS_MIN
                | DA_SAMPLE_SIZE_BASE
                | DA_SAMPLE_SIZE_MAX
                | DA_THRESHOLD_BASE
                | DA_PER_ATTESTER_SHARDS
                | DA_AUDIT_SAMPLE_SIZE
                | DA_AUDIT_WINDOW_COUNT
                | DA_ROTATION_MAX_HITS_PER_WINDOW
                | DA_ROTATION_WINDOW_SLOTS => {
                    self.scalar::<u16>(tag, path, NodeConfig, None, |value, _| value.to_string())?;
                }
                AXT_SLOT_LENGTH_MS
                | AXT_MAX_CLOCK_SKEW_MS
                | AXT_PROOF_CACHE_TTL_SLOTS
                | AXT_REPLAY_RETENTION_SLOTS
                | AUTOSCALE_TARGET_BLOCK_MS
                | DA_INGEST_QUOTA_WINDOW_BLOCKS
                | DA_INGEST_QUOTA_MAX_COUNT_PER_ACCOUNT
                | DA_INGEST_QUOTA_MAX_BYTES_PER_ACCOUNT => {
                    self.scalar::<u64>(tag, path, NodeConfig, None, integer)?;
                }
                AUTOSCALE_LAST_TRANSITION_HEIGHT
                | PIPELINE_AMX_PER_DATASPACE_BUDGET_MS
                | PIPELINE_AMX_GROUP_BUDGET_MS
                | PIPELINE_AMX_PER_INSTRUCTION_NS
                | PIPELINE_AMX_PER_MEMORY_ACCESS_NS
                | PIPELINE_AMX_PER_SYSCALL_NS => {
                    self.scalar::<u64>(tag, path, Genesis, None, integer)?;
                }
                AUTOSCALE_ENABLED => {
                    self.scalar::<bool>(tag, path, NodeConfig, None, |value, _| value.to_string())?;
                }
                AUTOSCALE_SCALE_OUT_LATENCY_RATIO_BITS
                | AUTOSCALE_SCALE_IN_LATENCY_RATIO_BITS
                | AUTOSCALE_SCALE_OUT_UTILIZATION_RATIO_BITS
                | AUTOSCALE_SCALE_IN_UTILIZATION_RATIO_BITS
                | DA_ROTATION_LATENCY_DECAY_BITS => {
                    let path = tag.strip_suffix("_bits").unwrap_or(tag).to_owned();
                    self.scalar::<u64>(tag, path, NodeConfig, None, |bits, _| {
                        format!("{:?}", f64::from_bits(*bits))
                    })?;
                }
                DA_AUDIT_INTERVAL_NS | DA_RECOVERY_REQUEST_TIMEOUT_NS => {
                    let path = format!("{}_ms", tag.strip_suffix("_ns").unwrap_or(tag));
                    self.scalar::<u128>(tag, path, NodeConfig, None, |nanos, _| {
                        if nanos % 1_000_000 == 0 {
                            (nanos / 1_000_000).to_string()
                        } else {
                            format!("{nanos} ns")
                        }
                    })?;
                }
                DA_ROTATION_SEED_TAG => {
                    self.scalar::<String>(tag, path, NodeConfig, None, |seed, _| {
                        format!("{seed:?}")
                    })?;
                }
                _ => {
                    return Err(NexusAmxContextError::Structure(
                        "a projection field has no field-level rendering",
                    ));
                }
            }
        }
        Ok(())
    }
}

fn dataspace_group(id: DataSpaceId, catalog: &NexusAmxContextCatalogV1) -> Group {
    let summary = catalog
        .dataspaces()
        .iter()
        .find(|dataspace| dataspace.id() == id)
        .map_or_else(
            || format!("dataspace {id}"),
            |dataspace| {
                format!(
                    "alias {:?}, fault_tolerance {}",
                    dataspace.alias(),
                    dataspace.fault_tolerance()
                )
            },
        );
    Group {
        path: format!("nexus.dataspace_catalog[id={id}]"),
        summary,
        origin: NexusAmxContextFieldOriginV1::NodeConfig,
    }
}

/// A lane id, annotated with its alias in the same preimage.
fn render_lane(id: LaneId, catalog: &NexusAmxContextCatalogV1) -> String {
    catalog.lane(id).map_or_else(
        || id.to_string(),
        |lane| format!("{id} ({:?})", lane.alias()),
    )
}

/// A dataspace id, annotated with its alias in the same preimage.
fn render_dataspace(id: DataSpaceId, catalog: &NexusAmxContextCatalogV1) -> String {
    catalog
        .dataspaces()
        .iter()
        .find(|dataspace| dataspace.id() == id)
        .map_or_else(
            || id.to_string(),
            |dataspace| format!("{id} ({:?})", dataspace.alias()),
        )
}

fn render_optional_dataspace(
    id: Option<DataSpaceId>,
    catalog: &NexusAmxContextCatalogV1,
) -> String {
    id.map_or_else(|| "unset".to_owned(), |id| render_dataspace(id, catalog))
}

fn render_optional_string(value: Option<&str>) -> String {
    value.map_or_else(|| "unset".to_owned(), |value| format!("{value:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nexus::{
        LaneCatalog, LaneConfig, NexusAmxContextPreimageV1, NexusAmxContextWriterV1,
    };
    use NexusAmxContextFieldOriginV1::{Genesis, NodeConfig};
    use std::num::NonZeroU32;

    type Rule = (u32, Option<u64>, Option<&'static str>, Option<&'static str>);

    #[derive(Clone)]
    struct Context {
        lanes: Vec<LaneConfig>,
        dataspaces: Vec<(u64, &'static str)>,
        rules: Vec<Rule>,
        exit_teu: u32,
    }

    fn lane(id: u32, alias: &str, dataspace: u64) -> LaneConfig {
        LaneConfig {
            id: LaneId::new(id),
            dataspace_id: DataSpaceId::new(dataspace),
            alias: alias.to_owned(),
            ..LaneConfig::default()
        }
    }

    fn sample() -> Context {
        Context {
            lanes: vec![lane(0, "core", 0), lane(5, "bpng", 9)],
            dataspaces: vec![(0, "universal"), (9, "bpng")],
            rules: vec![
                (0, Some(0), None, Some("governance")),
                (5, Some(9), Some("*@bpng"), None),
            ],
            exit_teu: 50_000_000,
        }
    }

    /// A complete preimage whose every field has the type the `iroha_config` writer emits.
    fn preimage(context: &Context) -> Vec<u8> {
        use tag::*;
        let catalog = LaneCatalog::new(NonZeroU32::new(8).unwrap(), context.lanes.clone()).unwrap();
        let (count, projected) = catalog.consensus_projection();
        let mut writer = NexusAmxContextWriterV1::new();
        writer.field(LANE_COUNT, &count);
        writer.field(LANES, &projected);
        writer.field(LANE_LIFECYCLE_COUNT, &(context.lanes.len() as u64));
        for lane in &context.lanes {
            writer.field(LANE_LIFECYCLE_LANE_ID, &lane.id);
            writer.field(LANE_LIFECYCLE_GENERATION, &0_u64);
            writer.field(
                LANE_LIFECYCLE_INCARNATION,
                &Hash::new(lane.alias.as_bytes()),
            );
            writer.field(LANE_LIFECYCLE_ACTIVATION_HEIGHT, &1_u64);
        }
        writer.field(DATASPACE_COUNT, &(context.dataspaces.len() as u64));
        for (id, alias) in &context.dataspaces {
            writer.field(DATASPACE_ID, &DataSpaceId::new(*id));
            writer.field(DATASPACE_ALIAS, &(*alias).to_owned());
            writer.field(DATASPACE_FAULT_TOLERANCE, &1_u32);
        }
        writer.field(ROUTING_DEFAULT_LANE, &LaneId::new(0));
        writer.field(ROUTING_DEFAULT_DATASPACE, &DataSpaceId::UNIVERSAL);
        writer.field(ROUTING_RULE_COUNT, &(context.rules.len() as u64));
        for (lane, dataspace, account, instruction) in &context.rules {
            writer.field(ROUTING_RULE_LANE, &LaneId::new(*lane));
            writer.field(ROUTING_RULE_DATASPACE, &dataspace.map(DataSpaceId::new));
            writer.field(ROUTING_RULE_ACCOUNT, &account.map(str::to_owned));
            writer.field(ROUTING_RULE_INSTRUCTION, &instruction.map(str::to_owned));
        }
        for tag in TAIL {
            match tag {
                STAKING_PUBLIC_VALIDATOR_MODE => writer.field(tag, &0_u8),
                STAKING_RESTRICTED_VALIDATOR_MODE => writer.field(tag, &1_u8),
                ROOT_FEE_POLICY => {
                    writer.field(tag, &Option::<crate::block::consensus::RootFeePolicy>::None)
                }
                DATASPACE_FEE_SPONSOR_PROGRAM_IDS => {
                    writer.field(tag, &BTreeMap::<DataSpaceId, FeeSponsorProgramId>::new())
                }
                FUSION_EXIT_TEU => writer.field(tag, &context.exit_teu),
                STAKING_MAX_VALIDATORS
                | FUSION_FLOOR_TEU
                | AUTOSCALE_MIN_LANE_ID
                | AUTOSCALE_MAX_LANE_ID_EXCLUSIVE
                | AUTOSCALE_PER_LANE_TARGET_TPS
                | DA_Q_IN_SLOT_TOTAL => writer.field(tag, &7_u32),
                FUSION_OBSERVATION_SLOTS
                | FUSION_MAX_WINDOW_SLOTS
                | AUTOSCALE_SCALE_OUT_WINDOW_BLOCKS
                | AUTOSCALE_SCALE_IN_WINDOW_BLOCKS
                | AUTOSCALE_COOLDOWN_BLOCKS
                | COMMIT_WINDOW_SLOTS
                | DA_Q_IN_SLOT_PER_DS_MIN
                | DA_SAMPLE_SIZE_BASE
                | DA_SAMPLE_SIZE_MAX
                | DA_THRESHOLD_BASE
                | DA_PER_ATTESTER_SHARDS
                | DA_AUDIT_SAMPLE_SIZE
                | DA_AUDIT_WINDOW_COUNT
                | DA_ROTATION_MAX_HITS_PER_WINDOW
                | DA_ROTATION_WINDOW_SLOTS => writer.field(tag, &7_u16),
                AUTOSCALE_ENABLED => writer.field(tag, &false),
                AUTOSCALE_SCALE_OUT_LATENCY_RATIO_BITS
                | AUTOSCALE_SCALE_IN_LATENCY_RATIO_BITS
                | AUTOSCALE_SCALE_OUT_UTILIZATION_RATIO_BITS
                | AUTOSCALE_SCALE_IN_UTILIZATION_RATIO_BITS
                | DA_ROTATION_LATENCY_DECAY_BITS => writer.field(tag, &0.75_f64.to_bits()),
                DA_AUDIT_INTERVAL_NS => writer.field(tag, &1_500_000_000_u128),
                DA_RECOVERY_REQUEST_TIMEOUT_NS => writer.field(tag, &1_500_u128),
                DA_ROTATION_SEED_TAG => writer.field(tag, &"seed".to_owned()),
                STAGED_ACTIVE_PUBLIC_LANE_VALIDATORS => {
                    writer.field(tag, &Vec::<GenesisActiveNexusLaneRecord>::new());
                }
                _ => writer.field(tag, &7_u64),
            }
        }
        writer.finish()
    }

    fn diff(expected: &Context, actual: &Context) -> Vec<NexusAmxContextFieldDiffV1> {
        diff_nexus_amx_context_v1(&preimage(expected), &preimage(actual)).unwrap()
    }

    #[test]
    fn fields_are_named_by_config_key_and_rendered_by_type() {
        let fields = nexus_amx_context_fields_v1(&preimage(&sample())).unwrap();
        let field = |path: &str| {
            fields.iter().find(|field| field.path == path).map_or_else(
                || panic!("missing {path}"),
                |field| (field.origin, field.value.as_str()),
            )
        };
        for (path, origin, value) in [
            ("nexus.lane_count", NodeConfig, "8"),
            ("nexus.lane_catalog[index=5].version", Genesis, "1"),
            ("nexus.lane_catalog[index=5].alias", NodeConfig, "\"bpng\""),
            (
                "nexus.lane_catalog[index=5].storage",
                NodeConfig,
                "full_replica",
            ),
            (
                "nexus.lane_catalog[index=5].proof_scheme",
                NodeConfig,
                "merkle_sha256",
            ),
            (
                "nexus.lane_catalog[index=5].manifest_policy",
                NodeConfig,
                "strict",
            ),
            ("nexus.lane_catalog[index=5].metadata", Genesis, "{}"),
            (
                "nexus.lane_catalog[index=5].dataspace",
                NodeConfig,
                "9 (\"bpng\")",
            ),
            (
                "nexus.dataspace_catalog[id=9].fault_tolerance",
                NodeConfig,
                "1",
            ),
            (
                "nexus.routing_policy.default_lane",
                NodeConfig,
                "0 (\"core\")",
            ),
            (
                "nexus.routing_policy.rules[1].matcher.account",
                NodeConfig,
                "\"*@bpng\"",
            ),
            (
                "nexus.routing_policy.rules[1].matcher.instruction",
                NodeConfig,
                "unset",
            ),
            (
                "nexus.staking.restricted_validator_mode",
                NodeConfig,
                "\"admin_managed\"",
            ),
            ("nexus.fusion.exit_teu", NodeConfig, "50000000"),
            ("nexus.fusion.observation_slots", NodeConfig, "7"),
            (
                "nexus.autoscale.scale_out_latency_ratio",
                NodeConfig,
                "0.75",
            ),
            ("nexus.da.audit.interval_ms", NodeConfig, "1500"),
            (
                "nexus.da.recovery.request_timeout_ms",
                NodeConfig,
                "1500 ns",
            ),
            ("nexus.da.rotation.seed_tag", NodeConfig, "\"seed\""),
            (
                "nexus.lane_lifecycle[lane=5].activation_height",
                Genesis,
                "1",
            ),
            ("nexus.autoscale.last_transition_height", Genesis, "7"),
            ("pipeline.amx_group_budget_ms", Genesis, "7"),
            ("pipeline.amx_per_instruction_ns", Genesis, "7"),
        ] {
            assert_eq!(field(path), (origin, value), "{path}");
        }
        let (origin, validators) = field(tag::STAGED_ACTIVE_PUBLIC_LANE_VALIDATORS);
        assert_eq!(origin, Genesis);
        assert!(
            validators.starts_with("0 records, ") && validators.contains(" bytes, hash "),
            "{validators}"
        );
        // Every tail tag has exactly one field, renamed only where the node key differs.
        for tail in tag::TAIL {
            if tail == tag::DATASPACE_FEE_SPONSOR_PROGRAM_IDS {
                continue;
            }
            let path = match tail {
                tag::DA_AUDIT_INTERVAL_NS => "nexus.da.audit.interval_ms",
                tag::DA_RECOVERY_REQUEST_TIMEOUT_NS => "nexus.da.recovery.request_timeout_ms",
                // Committed pipeline costs retain nanosecond units.
                _ => tail.strip_suffix("_bits").unwrap_or(tail),
            };
            assert_eq!(
                fields.iter().filter(|field| field.path == path).count(),
                1,
                "{tail}"
            );
        }
    }

    #[test]
    fn diff_names_a_changed_scalar_by_its_config_key() {
        let signed = sample();
        assert!(diff(&signed, &signed).is_empty());
        let mut node = signed.clone();
        node.exit_teu = 6_000;
        assert_eq!(
            diff(&signed, &node),
            [NexusAmxContextFieldDiffV1 {
                path: tag::FUSION_EXIT_TEU.to_owned(),
                origin: NodeConfig,
                expected: Some("50000000".to_owned()),
                actual: Some("6000".to_owned()),
            }]
        );
    }

    #[test]
    fn lane_policy_differences_use_config_labels_and_derived_metadata_origins() {
        use crate::nexus::{AUTOSCALE_META_CREATED_HEIGHT, DaManifestPolicy, LaneStorageProfile};

        let signed = sample();
        let mut node = signed.clone();
        node.lanes[1].storage = LaneStorageProfile::CommitmentOnly;
        node.lanes[1].manifest_policy = DaManifestPolicy::Audit;
        node.lanes[1]
            .metadata
            .insert(AUTOSCALE_META_CREATED_HEIGHT.into(), "42".into());
        node.lanes[1]
            .metadata
            .insert("operator_note".into(), "not committed".into());
        let diffs = diff(&signed, &node);
        assert_eq!(diffs.len(), 3, "{diffs:?}");
        let storage = &diffs[0];
        assert_eq!(storage.path, "nexus.lane_catalog[index=5].storage");
        assert_eq!(storage.origin, NodeConfig);
        assert_eq!(
            storage
                .expected
                .as_deref()
                .unwrap()
                .parse::<LaneStorageProfile>(),
            Ok(LaneStorageProfile::FullReplica)
        );
        assert_eq!(
            storage
                .actual
                .as_deref()
                .unwrap()
                .parse::<LaneStorageProfile>(),
            Ok(LaneStorageProfile::CommitmentOnly)
        );
        let manifest = &diffs[1];
        assert_eq!(manifest.path, "nexus.lane_catalog[index=5].manifest_policy");
        assert_eq!(manifest.origin, NodeConfig);
        assert_eq!(
            manifest
                .expected
                .as_deref()
                .unwrap()
                .parse::<DaManifestPolicy>(),
            Ok(DaManifestPolicy::Strict)
        );
        assert_eq!(
            manifest
                .actual
                .as_deref()
                .unwrap()
                .parse::<DaManifestPolicy>(),
            Ok(DaManifestPolicy::Audit)
        );
        let metadata = &diffs[2];
        assert_eq!(metadata.path, "nexus.lane_catalog[index=5].metadata");
        assert_eq!(metadata.origin, Genesis);
        let metadata = metadata.actual.as_deref().unwrap();
        assert!(
            metadata.contains(AUTOSCALE_META_CREATED_HEIGHT),
            "{metadata}"
        );
        assert!(!metadata.contains("operator_note"), "{metadata}");
    }

    #[test]
    fn diff_reports_a_one_sided_entry_once_without_misaligning_later_fields() {
        let signed = sample();
        let mut node = signed.clone();
        node.lanes.retain(|lane| lane.id != LaneId::new(5));
        node.dataspaces.retain(|(id, _)| *id != 9);
        node.rules.truncate(1);
        let diffs = diff(&signed, &node);
        let summary = |path: &str| {
            diffs
                .iter()
                .find(|diff| diff.path == path)
                .unwrap_or_else(|| panic!("missing {path}: {diffs:?}"))
        };
        assert_eq!(
            diffs
                .iter()
                .map(|diff| diff.path.as_str())
                .collect::<Vec<_>>(),
            [
                "nexus.lane_catalog[index=5]",
                "nexus.lane_lifecycle[lane=5]",
                "nexus.dataspace_catalog[id=9]",
                "nexus.routing_policy.rules[1]",
            ]
        );
        assert_eq!(
            summary("nexus.dataspace_catalog[id=9]").expected.as_deref(),
            Some("alias \"bpng\", fault_tolerance 1")
        );
        assert_eq!(summary("nexus.lane_catalog[index=5]").actual, None);
        assert_eq!(summary("nexus.lane_catalog[index=5]").origin, NodeConfig);
        assert_eq!(summary("nexus.lane_lifecycle[lane=5]").origin, Genesis);
        // The same entries reported from the other side swap expected and actual.
        let reverse = diff(&node, &signed);
        assert_eq!(reverse.len(), diffs.len());
        assert!(
            reverse
                .iter()
                .all(|diff| diff.expected.is_none() && diff.actual.is_some())
        );
    }

    #[test]
    fn alias_annotations_never_create_differences_of_their_own() {
        let signed = sample();
        let mut node = signed.clone();
        node.dataspaces[1].1 = "bank";
        // Lane 5 and the second rule render dataspace 9 with its alias, but their own
        // encoded values are unchanged: only the renamed alias differs.
        assert_eq!(
            diff(&signed, &node),
            [NexusAmxContextFieldDiffV1 {
                path: "nexus.dataspace_catalog[id=9].alias".to_owned(),
                origin: NodeConfig,
                expected: Some("\"bpng\"".to_owned()),
                actual: Some("\"bank\"".to_owned()),
            }]
        );
    }

    #[test]
    fn malformed_preimages_are_refused_instead_of_compared() {
        let good = preimage(&sample());
        assert_eq!(
            diff_nexus_amx_context_v1(&good, &good[..good.len() - 1]),
            Err(NexusAmxContextError::Truncated)
        );
        assert_eq!(
            diff_nexus_amx_context_v1(&[], &good),
            Err(NexusAmxContextError::Oversize)
        );
    }

    #[test]
    fn retained_preimage_debug_shows_length_and_hash_only() {
        let bytes = preimage(&sample());
        let retained = NexusAmxContextPreimageV1::new(bytes.clone());
        assert_eq!(retained.as_bytes(), bytes.as_slice());
        assert_eq!(retained.hash(), Hash::new(&bytes));
        assert_eq!(
            format!("{retained:?}"),
            format!(
                "NexusAmxContextPreimageV1 {{ len: {}, hash: {} }}",
                bytes.len(),
                Hash::new(&bytes)
            )
        );
    }
}
