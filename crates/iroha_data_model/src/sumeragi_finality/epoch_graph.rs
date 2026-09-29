//! Canonical epoch-bearing height slots and atomic boundary schedule results.

use super::{ChainParamsRecord, ScheduleError, consensus_key, global_committee};
use crate::{
    parameter::system::ConsensusMode,
    sumeragi::epoch::{ValidatorEpochBoundaryV1, ValidatorEpochContextV1},
};
use iroha_sumeragi::types::{
    AppliedConfig, ConfigSlot, EpochConfig, EpochId, Hash32, HeightConfig,
};
use norito::{
    NoritoDeserialize, NoritoSerialize,
    derive::{JsonDeserialize, JsonSerialize},
};

/// Complete authorized configuration of one height.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::ScheduledConfig")]
pub struct ScheduledConfig {
    /// Exact height this configuration governs.
    pub height: u64,
    /// Full authenticated scheduling epoch and original generation credentials.
    pub epoch: ValidatorEpochContextV1,
    /// Ordinary lag-two chain parameters, independent of authority-generation lifetime.
    pub params: ChainParamsRecord,
}
impl ScheduledConfig {
    /// Check complete context and exact containment before building a generic core config.
    pub fn height_config(&self) -> Result<HeightConfig, ScheduleError> {
        self.epoch.validate().map_err(ScheduleError::Epoch)?;
        if self.height < self.epoch.authorization.first_height
            || self.height > self.epoch.authorization.last_height
        {
            return Err(ScheduleError::Epoch(
                "scheduled height is outside its authenticated epoch".into(),
            ));
        }
        self.params.validate().map_err(ScheduleError::Params)?;
        let committee = global_committee(
            self.epoch
                .committee
                .iter()
                .map(|member| consensus_key(&member.validator))
                .collect::<Result<Vec<_>, _>>()?,
        )?;
        let config = HeightConfig {
            epoch: Box::new(core_epoch(&self.epoch)?),
            committee,
            params: self.params.to_core(),
        };
        iroha_sumeragi::pacemaker::validate_height(&config, super::CHAIN_TRANSPORT_FRAME_LIMIT)
            .map_err(ScheduleError::Params)?;
        Ok(config)
    }
}

/// Exact signed epoch identity passed to generic core signatures and safety records.
pub fn core_epoch(context: &ValidatorEpochContextV1) -> Result<EpochConfig, ScheduleError> {
    context.validate().map_err(ScheduleError::Epoch)?;
    Ok(EpochConfig {
        da_layout: context.da_layout,
        id: EpochId {
            epoch: context.authorization.epoch,
            context: Hash32(context.context_id().map_err(ScheduleError::Epoch)?),
        },
        authority_generation: Hash32(
            context
                .authority
                .authority_id()
                .map_err(|error| ScheduleError::Epoch(error.to_string()))?,
        ),
        first_height: context.authorization.first_height,
        last_height: context.authorization.last_height,
        leader_seed: Hash32(context.leader_seed),
    })
}

/// One scheduled height; a pending boundary has parameters but grants no signing authority.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::ScheduledSlot")]
#[norito(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ScheduledSlot {
    /// Configuration authorized by signed genesis or the incumbent-certified boundary.
    Ready(ScheduledConfig),
    /// Await publication of the exact predecessor boundary before signing this height.
    PendingBoundary {
        /// Height whose authority remains unavailable.
        height: u64,
        /// Incumbent boundary that must certify the successor.
        boundary_height: u64,
        /// Complete predecessor epoch identity, never a local QC signer subset.
        predecessor_context_id: [u8; 32],
        /// Lag-two parameters already fixed for this height.
        params: ChainParamsRecord,
    },
}
// Schema-only projection of the named variant; the canonical codec remains ScheduledSlot.
struct PendingBoundarySchema;
impl iroha_schema::TypeId for PendingBoundarySchema {
    fn id() -> iroha_schema::Ident {
        "SumeragiPendingBoundary".into()
    }
}
impl iroha_schema::IntoSchema for PendingBoundarySchema {
    fn type_name() -> iroha_schema::Ident {
        "SumeragiPendingBoundary".into()
    }
    fn update_schema_map(map: &mut iroha_schema::MetaMap) {
        use iroha_schema::{Declaration, Metadata, NamedFieldsMeta};
        if map.contains_key::<Self>() {
            return;
        }
        u64::update_schema_map(map);
        <[u8; 32]>::update_schema_map(map);
        ChainParamsRecord::update_schema_map(map);
        map.insert::<Self>(Metadata::Struct(NamedFieldsMeta {
            declarations: vec![
                Declaration {
                    name: "height".into(),
                    ty: core::any::TypeId::of::<u64>(),
                },
                Declaration {
                    name: "boundary_height".into(),
                    ty: core::any::TypeId::of::<u64>(),
                },
                Declaration {
                    name: "predecessor_context_id".into(),
                    ty: core::any::TypeId::of::<[u8; 32]>(),
                },
                Declaration {
                    name: "params".into(),
                    ty: core::any::TypeId::of::<ChainParamsRecord>(),
                },
            ],
        }));
    }
}
impl iroha_schema::TypeId for ScheduledSlot {
    fn id() -> iroha_schema::Ident {
        "ScheduledSlot".into()
    }
}
impl iroha_schema::IntoSchema for ScheduledSlot {
    fn type_name() -> iroha_schema::Ident {
        "ScheduledSlot".into()
    }
    fn update_schema_map(map: &mut iroha_schema::MetaMap) {
        use iroha_schema::{EnumMeta, EnumVariant, Metadata};
        if map.contains_key::<Self>() {
            return;
        }
        ScheduledConfig::update_schema_map(map);
        PendingBoundarySchema::update_schema_map(map);
        map.insert::<Self>(Metadata::Enum(EnumMeta {
            variants: vec![
                EnumVariant {
                    tag: "ready".into(),
                    discriminant: 0,
                    ty: Some(core::any::TypeId::of::<ScheduledConfig>()),
                },
                EnumVariant {
                    tag: "pending_boundary".into(),
                    discriminant: 1,
                    ty: Some(core::any::TypeId::of::<PendingBoundarySchema>()),
                },
            ],
        }));
    }
}
impl ScheduledSlot {
    /// Exact slot height.
    pub const fn height(&self) -> u64 {
        match self {
            Self::Ready(config) => config.height,
            Self::PendingBoundary { height, .. } => *height,
        }
    }
    /// Exact parameter projection, including an unauthenticated authority slot.
    pub const fn params(&self) -> &ChainParamsRecord {
        match self {
            Self::Ready(config) => &config.params,
            Self::PendingBoundary { params, .. } => params,
        }
    }
    /// Map only an authorized slot to signing authority; a pending slot remains explicit.
    pub fn to_core(&self, current: &ValidatorEpochContextV1) -> Result<ConfigSlot, ScheduleError> {
        self.validate_against(current)?;
        match self {
            Self::Ready(config) => Ok(ConfigSlot::Ready(config.height_config()?)),
            Self::PendingBoundary {
                boundary_height,
                predecessor_context_id,
                ..
            } => Ok(ConfigSlot::PendingBoundary {
                boundary_height: *boundary_height,
                predecessor: EpochId {
                    epoch: current.authorization.epoch,
                    context: Hash32(*predecessor_context_id),
                },
            }),
        }
    }
    fn validate_against(&self, current: &ValidatorEpochContextV1) -> Result<(), ScheduleError> {
        self.params().validate().map_err(ScheduleError::Params)?;
        match self {
            Self::Ready(config) => {
                config.epoch.validate().map_err(ScheduleError::Epoch)?;
                if config.epoch != *current
                    || config.height < current.authorization.first_height
                    || config.height > current.authorization.last_height
                {
                    return Err(ScheduleError::Epoch(
                        "ready slot changes its authorized epoch".into(),
                    ));
                }
            }
            Self::PendingBoundary {
                height,
                boundary_height,
                predecessor_context_id,
                ..
            } => {
                if current.mode != ConsensusMode::Npos
                    || *boundary_height != current.authorization.last_height
                    || *height <= *boundary_height
                    || *predecessor_context_id
                        != current.context_id().map_err(ScheduleError::Epoch)?
                {
                    return Err(ScheduleError::Epoch(
                        "pending slot differs from its exact incumbent boundary".into(),
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Complete authority/scheduling effect committed in the incumbent's exact execution result.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::ScheduleOutcome")]
pub struct ScheduleOutcome {
    /// Original execution height.
    pub height: u64,
    /// Exact incumbent context governing the block, including its complete signing generation.
    pub current: ValidatorEpochContextV1,
    /// Mandatory exactly at an NPoS boundary; carries next authorization and E+2 preparation.
    pub boundary: Option<ValidatorEpochBoundaryV1>,
    /// Height h+1, ready only when already authorized or installed by this exact boundary.
    pub next: ScheduledSlot,
    /// Height h+2, with original lag-two parameters and an explicit boundary barrier if needed.
    pub after_next: ScheduledSlot,
}
impl ScheduleOutcome {
    /// Validate the next certified result against this result's original successor slots.
    ///
    /// # Errors
    /// A height gap, changed incumbent, changed lag-two parameters or misplaced boundary.
    pub fn validate_successor(&self, next: &Self) -> Result<(), ScheduleError> {
        self.validate()?;
        next.validate()?;
        let ScheduledSlot::Ready(incumbent) = &self.next else {
            return Err(ScheduleError::Epoch(
                "next height lacks certified authority".into(),
            ));
        };
        if self.height.checked_add(1) != Some(next.height)
            || incumbent.epoch != next.current
            || self.after_next.params() != next.next.params()
            || (next.boundary.is_none() && self.after_next != next.next)
            || (next.boundary.is_some()
                && !matches!(self.after_next, ScheduledSlot::PendingBoundary { .. }))
        {
            return Err(ScheduleError::Epoch(
                "successor changes its authenticated epoch or parameter slots".into(),
            ));
        }
        Ok(())
    }
    /// Validate all canonical context, slot, boundary-presence and height relationships.
    pub fn validate(&self) -> Result<(), ScheduleError> {
        self.current.validate().map_err(ScheduleError::Epoch)?;
        let current = &self.current.authorization;
        if self.height < current.first_height
            || self.height > current.last_height
            || self.height.checked_add(1) != Some(self.next.height())
            || self.height.checked_add(2) != Some(self.after_next.height())
        {
            return Err(ScheduleError::Epoch(
                "schedule result has nonconsecutive or foreign heights".into(),
            ));
        }
        let is_boundary =
            self.current.mode == ConsensusMode::Npos && self.height == current.last_height;
        if is_boundary != self.boundary.is_some() {
            return Err(ScheduleError::Epoch(
                "schedule result omits or invents an epoch boundary".into(),
            ));
        }
        let authorized = if let Some(boundary) = &self.boundary {
            boundary
                .validate_against(&self.current)
                .map_err(ScheduleError::Epoch)?;
            if boundary.height != self.height {
                return Err(ScheduleError::Epoch(
                    "schedule result carries a future boundary".into(),
                ));
            }
            &boundary.next
        } else {
            &self.current
        };
        self.next.validate_against(authorized)?;
        self.after_next.validate_against(authorized)?;
        if is_boundary
            && (!matches!(&self.next, ScheduledSlot::Ready(_))
                || !matches!(&self.after_next, ScheduledSlot::Ready(_)))
        {
            return Err(ScheduleError::Epoch(
                "boundary must atomically authorize both successor slots".into(),
            ));
        }
        Ok(())
    }
    /// Generic core event released only after this original boundary's successful publication.
    pub fn applied_config(&self) -> Result<AppliedConfig, ScheduleError> {
        self.validate()?;
        if self.boundary.is_some() {
            let (ScheduledSlot::Ready(next), ScheduledSlot::Ready(after_next)) =
                (&self.next, &self.after_next)
            else {
                return Err(ScheduleError::Epoch(
                    "boundary successor authority is unavailable".into(),
                ));
            };
            Ok(AppliedConfig::Boundary {
                next: next.height_config()?,
                after_next: after_next.height_config()?,
            })
        } else {
            Ok(AppliedConfig::Continuation {
                after_next: self.after_next.to_core(&self.current)?,
            })
        }
    }
}

/// Canonical retained window after one applied execution. Pending slots carry no authority.
/// These pure decoded-value methods validate the graph; production construction separately
/// admits and retains each owned canonical allocation from the original execution pool.
#[derive(
    Clone,
    Debug,
    Default,
    PartialEq,
    Eq,
    NoritoSerialize,
    NoritoDeserialize,
    JsonSerialize,
    JsonDeserialize,
    norito::NoritoSchema,
    iroha_schema::IntoSchema,
)]
#[norito_schema(name = "iroha_data_model::sumeragi_finality::ConsensusSchedule")]
pub struct ConsensusSchedule {
    entries: Vec<ScheduledSlot>,
}
impl ConsensusSchedule {
    /// Construct the pre-genesis empty schedule.
    pub const fn empty() -> Self {
        Self {
            entries: Vec::new(),
        }
    }
    /// Adopt and validate a complete canonical slot window.
    pub fn from_owned_entries(entries: Vec<ScheduledSlot>) -> Result<Self, ScheduleError> {
        let result = Self { entries };
        result.validate()?;
        Ok(result)
    }
    /// Oldest to newest canonical slots.
    pub fn entries(&self) -> &[ScheduledSlot] {
        &self.entries
    }
    /// The applied cut represented by the window.
    pub fn tip(&self) -> Option<u64> {
        self.entries.first().map(ScheduledSlot::height)
    }
    /// Look up a slot without treating a boundary barrier as signing authority.
    pub fn get(&self, height: u64) -> Option<&ScheduledSlot> {
        self.entries.iter().find(|entry| entry.height() == height)
    }
    /// Obtain an already authenticated configuration, refusing a pending boundary.
    pub fn ready(&self, height: u64) -> Result<&ScheduledConfig, ScheduleError> {
        match self.get(height) {
            Some(ScheduledSlot::Ready(config)) => Ok(config),
            Some(ScheduledSlot::PendingBoundary { .. }) => Err(ScheduleError::Epoch(
                "height awaits its exact certified predecessor boundary".into(),
            )),
            None => Err(ScheduleError::Malformed),
        }
    }
    /// The window is empty before genesis or contains three consecutive valid slots.
    pub fn is_well_formed(&self) -> bool {
        self.validate().is_ok()
    }
    fn validate(&self) -> Result<(), ScheduleError> {
        if self.entries.is_empty() {
            return Ok(());
        }
        if self.entries.len() != 3
            || self
                .entries
                .windows(2)
                .any(|pair| pair[0].height().checked_add(1) != Some(pair[1].height()))
        {
            return Err(ScheduleError::Malformed);
        }
        let ScheduledSlot::Ready(first) = &self.entries[0] else {
            return Err(ScheduleError::Malformed);
        };
        first.height_config()?;
        let mut context = &first.epoch;
        for slot in &self.entries[1..] {
            if let ScheduledSlot::Ready(config) = slot {
                // A window straddles the applied boundary only at its first height.
                if config.epoch != *context {
                    if first.height != first.epoch.authorization.last_height
                        || first.epoch.mode != ConsensusMode::Npos
                        || first.height.checked_add(1) != Some(config.height)
                        || config.epoch.authorization.epoch
                            != first
                                .epoch
                                .authorization
                                .epoch
                                .checked_add(1)
                                .ok_or(ScheduleError::HeightOverflow)?
                        || config.epoch.authorization.previous_authorization_id
                            != first
                                .epoch
                                .authorization
                                .authorization_id()
                                .map_err(|error| ScheduleError::Epoch(error.to_string()))?
                        || config.epoch.authorization.first_height != config.height
                    {
                        return Err(ScheduleError::Epoch(
                            "stored window changes authority away from its applied boundary".into(),
                        ));
                    }
                    config
                        .epoch
                        .authorization
                        .validate_successor(&first.epoch.authorization)
                        .map_err(|error| ScheduleError::Epoch(error.to_string()))?;
                    if config.epoch.network_id != first.epoch.network_id
                        || config.epoch.mode != first.epoch.mode
                    {
                        return Err(ScheduleError::Epoch(
                            "stored successor changes its network or mode".into(),
                        ));
                    }
                    if matches!(config.epoch.authorization.decision,
                        crate::isi::kagemusha_v1::KagemushaMintFinalityEpochDecisionV1::Retain
                        | crate::isi::kagemusha_v1::KagemushaMintFinalityEpochDecisionV1::RetainAndCancel)
                        && (config.epoch.authority != first.epoch.authority || config.epoch.committee != first.epoch.committee) {
                        return Err(ScheduleError::Epoch("retained window replaces original authority credentials".into()));
                    }
                    context = &config.epoch;
                }
            }
            slot.validate_against(context)?;
        }
        Ok(())
    }
    /// Build the first window from independently authenticated signed genesis authority.
    pub fn from_genesis(
        epoch: ValidatorEpochContextV1,
        params: ChainParamsRecord,
    ) -> Result<Self, ScheduleError> {
        epoch.validate().map_err(ScheduleError::Epoch)?;
        if epoch.authorization.epoch != 0 || epoch.authorization.first_height != 1 {
            return Err(ScheduleError::Epoch(
                "genesis schedule does not begin at native epoch zero".into(),
            ));
        }
        params.validate().map_err(ScheduleError::Params)?;
        let entries = (1..=3)
            .map(|height| {
                ScheduledSlot::Ready(ScheduledConfig {
                    height,
                    epoch: epoch.clone(),
                    params,
                })
            })
            .collect();
        let result = Self { entries };
        result.validate()?;
        Ok(result)
    }
    /// Check a genesis result against a separately reconstructed signed genesis context.
    /// The caller must compare the resulting context with that independent root before trust.
    pub fn from_genesis_outcome(outcome: &ScheduleOutcome) -> Result<Self, ScheduleError> {
        outcome.validate()?;
        if outcome.height != 1
            || outcome.boundary.is_some()
            || outcome.current.authorization.epoch != 0
            || outcome.current.authorization.first_height != 1
        {
            return Err(ScheduleError::Epoch(
                "non-genesis result cannot seed native authority".into(),
            ));
        }
        if outcome.next.params() != outcome.after_next.params() {
            return Err(ScheduleError::Epoch(
                "genesis successor parameters differ".into(),
            ));
        }
        let result = Self::from_genesis(outcome.current.clone(), *outcome.next.params())?;
        if result.entries[1] != outcome.next || result.entries[2] != outcome.after_next {
            return Err(ScheduleError::Epoch(
                "genesis successor authority differs".into(),
            ));
        }
        Ok(result)
    }
    /// Advance one genuinely certified result. At B only, its boundary replaces the B+1
    /// authority barrier; already fixed B+1 parameters remain unchanged. Ordinary authority
    /// cannot change through candidacy or decoded result data outside this certified graph.
    pub fn advanced(&self, outcome: &ScheduleOutcome) -> Result<Self, ScheduleError> {
        self.validate()?;
        outcome.validate()?;
        if self.tip().and_then(|tip| tip.checked_add(1)) != Some(outcome.height) {
            return Err(ScheduleError::NotConsecutive {
                height: outcome.height,
                last_scheduled: self.entries.last().map(ScheduledSlot::height),
            });
        }
        let incumbent = self.ready(outcome.height)?;
        if incumbent.epoch != outcome.current {
            return Err(ScheduleError::Epoch(
                "certified result changes its signing incumbent".into(),
            ));
        }
        let retained_next = self
            .get(outcome.next.height())
            .ok_or(ScheduleError::Malformed)?;
        if retained_next.params() != outcome.next.params() {
            return Err(ScheduleError::Epoch(
                "boundary changes already scheduled parameters".into(),
            ));
        }
        if outcome.boundary.is_none() && retained_next != &outcome.next {
            return Err(ScheduleError::Epoch(
                "ordinary result changes already authorized next slot".into(),
            ));
        }
        if outcome.boundary.is_some()
            && !matches!(retained_next, ScheduledSlot::PendingBoundary { .. })
        {
            return Err(ScheduleError::Epoch(
                "boundary did not replace its exact pending successor".into(),
            ));
        }
        let result = Self {
            entries: vec![
                ScheduledSlot::Ready(incumbent.clone()),
                outcome.next.clone(),
                outcome.after_next.clone(),
            ],
        };
        result.validate()?;
        Ok(result)
    }
    /// Restore the actual ready/pending slots without guessing post-boundary authority.
    pub fn init_configs(
        &self,
        genesis_height: u64,
    ) -> Result<Vec<(u64, ConfigSlot)>, ScheduleError> {
        self.validate()?;
        let ScheduledSlot::Ready(first) = self.entries.first().ok_or(ScheduleError::Malformed)?
        else {
            return Err(ScheduleError::Malformed);
        };
        let mut current = &first.epoch;
        let mut result = Vec::with_capacity(3);
        for entry in &self.entries {
            if let ScheduledSlot::Ready(config) = entry {
                current = &config.epoch;
            }
            if entry.height() != genesis_height {
                result.push((entry.height(), entry.to_core(current)?));
            }
        }
        Ok(result)
    }
}
