//! Creation-time stake ownership and retained global lifetime of native lane obligations.
//!
//! Monetary identity is pinned once. Retirement removes routing/storage ownership, not the
//! obligation. Native lane heights never enter the global evidence or withdrawal clocks.

use iroha_config::parameters::actual::Nexus;
use iroha_data_model::{
    nexus::PublicLaneValidatorRecord,
    sumeragi_lanes::{
        MAX_LANE_CUSTODY_OBLIGATIONS, MAX_LANE_CUSTODY_SIGNERS, SumeragiLaneCustody,
        SumeragiLaneCustodySigners, SumeragiLanePolicy, SumeragiLaneRecord,
        SumeragiLaneSignerCustody, SumeragiLaneStakeBinding, SumeragiLaneState,
    },
};
use iroha_model_base::topology::LaneId;
use mv::storage::StorageReadOnly;

use crate::state::{WorldReadOnly, nexus_staking_authority_lane_at_height};

/// Deterministic defects in original lane custody; fixed-size errors need no allocation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
// Keep the public lane-step error payload inline with its lane-id-sized alternatives.
#[repr(u32)]
pub enum CustodyViolation {
    /// The committee exceeds native geometry.
    #[error("invalid lane custody committee")]
    Committee,
    /// One peer occupies more than one committee position.
    #[error("duplicate original lane committee member")]
    DuplicateMember,
    /// A native position cannot fit its canonical index.
    #[error("lane signer index overflow")]
    SignerIndex,
    /// More than one original stake registration claims a native signer.
    #[error("lane signer has ambiguous original stake custody")]
    AmbiguousStake,
    /// Original registration or escrow cannot produce a valid tenure identity.
    #[error("invalid original lane stake custody")]
    StakeBinding,
    /// Sparse signer bindings violate their canonical order or bounds.
    #[error("invalid original lane custody signer order or bound")]
    Signers,
    /// Retained fences or original incarnation identity are inconsistent.
    #[error("invalid original lane custody obligation")]
    Obligation,
    /// Monetary custody has lost its signed admission and penalty policy.
    #[error("lane custody lost signed evidence policy")]
    MissingPolicy,
    /// A live original incarnation disappeared before authenticated retirement.
    #[error("live lane custody lost its original incarnation")]
    MissingIncarnation,
    /// A creation attempts to replace already pinned original custody.
    #[error("duplicate lane custody creation")]
    DuplicateCreation,
    /// Deterministic creation failed to respect the available ledger capacity.
    #[error("lane custody capacity exhausted")]
    Capacity,
}

/// Separate deterministic custody defects from local allocation refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CustodyError {
    /// An original binding or its canonical geometry is inconsistent.
    Invalid(CustodyViolation),
    /// The local allocator could not retain the bounded original binding table.
    Allocation,
}
impl From<CustodyViolation> for CustodyError {
    fn from(reason: CustodyViolation) -> Self {
        Self::Invalid(reason)
    }
}

/// Pin only genuine positive custody belonging to the original selected peer and tenure.
/// Sparse fixed-size entries contain no nested heap owners; identity hashing streams bytes.
/// TODO(S8): retain this new backing and every World clone in the original allocation pool.
fn pin_signers(
    world: &impl WorldReadOnly,
    nexus: &Nexus,
    record: &SumeragiLaneRecord,
    elastic: bool,
) -> Result<SumeragiLaneCustodySigners, CustodyError> {
    let mut signers = Vec::<SumeragiLaneSignerCustody>::new();
    if !(1..=MAX_LANE_CUSTODY_SIGNERS).contains(&record.committee.len()) {
        return Err(CustodyViolation::Committee.into());
    }
    // Elastic committees are sampled from global validators. Fixed committees may bind only
    // the canonical staking owner of their physical dataspace; an unbound signer is forensic.
    let owner = if elastic {
        Some(LaneId::SINGLE)
    } else {
        nexus_staking_authority_lane_at_height(record.lane, nexus, record.created_at)
    };
    let Some(owner) = owner else {
        return Ok(SumeragiLaneCustodySigners::default());
    };
    for (key, validator) in world.public_lane_validators().iter() {
        let (lane, account) = key;
        if *lane != owner
            || validator.lane_id != *lane
            || validator.validator != *account
            || validator.stake_account != *account
            || validator.activation_height > record.created_at
            || validator
                .deactivation_height
                .is_some_and(|end| record.created_at >= end)
        {
            continue;
        }
        let matching = record
            .committee
            .iter()
            .filter(|member| member.peer == validator.peer_id)
            .count();
        if matching == 0 {
            continue;
        }
        if matching != 1 {
            return Err(CustodyViolation::DuplicateMember.into());
        }
        // The native core sorts consensus keys, while a fixed policy need not store its
        // members in that order. BLS-normal PeerIds use the same canonical key ordering.
        let index = record
            .committee
            .iter()
            .filter(|member| member.peer < validator.peer_id)
            .count();
        // Borrow the complete composite key from this original cursor; no account clone is
        // needed just to find the original retained asset and positive custody amount.
        let Some((asset, amount)) = world.public_lane_stake_custody().get(key) else {
            continue;
        };
        if amount.is_zero() {
            continue;
        }
        let signer = u32::try_from(index).map_err(|_| CustodyViolation::SignerIndex)?;
        if signers.iter().any(|entry| entry.signer == signer) {
            return Err(CustodyViolation::AmbiguousStake.into());
        }
        signers
            .try_reserve_exact(1)
            .map_err(|_| CustodyError::Allocation)?;
        signers.push(SumeragiLaneSignerCustody {
            signer,
            binding: SumeragiLaneStakeBinding::from_record(validator, asset)
                .map_err(|_| CustodyViolation::StakeBinding)?,
        });
    }
    signers.sort_unstable_by_key(|entry| entry.signer);
    signers
        .try_into()
        .map_err(|_| CustodyViolation::Signers.into())
}

/// Carry forward monotone signed policy and mark exact retirement before records are removed.
pub(super) fn prepare_retirement(
    state: &mut SumeragiLaneState,
    world: &impl WorldReadOnly,
    height: u64,
) -> Result<(), CustodyViolation> {
    let parameters = world.sumeragi_npos_parameters();
    for obligation in &mut state.custody {
        obligation
            .validate()
            .map_err(|_| CustodyViolation::Obligation)?;
        let parameters = parameters.as_ref().ok_or(CustodyViolation::MissingPolicy)?;
        obligation.evidence_horizon = obligation
            .evidence_horizon
            .max(parameters.evidence_horizon_blocks());
        obligation.slashing_delay = obligation
            .slashing_delay
            .max(parameters.slashing_delay_blocks());
        if obligation.retired_at.is_none() {
            let record = state
                .lanes
                .iter()
                .find(|record| record.incarnation == obligation.incarnation)
                .ok_or(CustodyViolation::MissingIncarnation)?;
            if record
                .retirement_height()
                .is_some_and(|retirement| retirement <= height)
            {
                obligation.retired_at = record.retirement_height();
            }
        }
        obligation
            .validate()
            .map_err(|_| CustodyViolation::Obligation)?;
    }
    state.custody.retain(|obligation| {
        obligation.retains_at(height).unwrap_or(true) || has_pending_evidence(world, obligation)
    });
    Ok(())
}

/// Creation can wait for custody capacity; existing lane close/retirement remains executable.
pub(super) fn creation_capacity(state: &SumeragiLaneState, world: &impl WorldReadOnly) -> usize {
    if world.sumeragi_npos_parameters().is_some() {
        MAX_LANE_CUSTODY_OBLIGATIONS.saturating_sub(state.custody.len())
    } else {
        usize::MAX
    }
}

/// Install each creation's original binding exactly once after deterministic lane selection.
pub(super) fn pin_created(
    state: &mut SumeragiLaneState,
    world: &impl WorldReadOnly,
    nexus: &Nexus,
    network: &iroha_data_model::NetworkId,
    chain_id: &str,
    policy: Option<&SumeragiLanePolicy>,
    height: u64,
) -> Result<(), CustodyError> {
    let Some(parameters) = world.sumeragi_npos_parameters() else {
        // Permissioned chains have no signed monetary evidence horizon; existing native
        // forensic verification remains possible but creation cannot manufacture stake policy.
        return Ok(());
    };
    for record in state
        .lanes
        .iter()
        .filter(|record| record.created_at == height)
    {
        if state
            .custody
            .iter()
            .any(|row| row.incarnation == record.incarnation)
        {
            return Err(CustodyViolation::DuplicateCreation.into());
        }
        if state.custody.len() >= MAX_LANE_CUSTODY_OBLIGATIONS {
            return Err(CustodyViolation::Capacity.into());
        }
        let obligation = SumeragiLaneCustody {
            lane: record.lane,
            incarnation: record.incarnation,
            instance: super::lane_instance(
                &crate::sumeragi::crypto::BlsCrypto::new(),
                network,
                chain_id,
                record,
            )
            .0,
            created_at: record.created_at,
            signer_count: u32::try_from(record.committee.len())
                .map_err(|_| CustodyViolation::Committee)?,
            signers: pin_signers(
                world,
                nexus,
                record,
                policy.is_some_and(|policy| policy.is_elastic(record.lane)),
            )?,
            evidence_horizon: parameters.evidence_horizon_blocks(),
            slashing_delay: parameters.slashing_delay_blocks(),
            retired_at: None,
        };
        obligation
            .validate()
            .map_err(|_| CustodyViolation::Obligation)?;
        state
            .custody
            .try_reserve_exact(1)
            .map_err(|_| CustodyError::Allocation)?;
        state.custody.push(obligation);
    }
    state.custody.sort_unstable_by_key(|row| row.incarnation);
    Ok(())
}

/// Retained obligations prohibit reuse of the exact registration, even after all stake is gone.
/// Malformed authenticated custody fails closed. Ordinary global committee rules remain separate.
pub(crate) fn retains_registration(
    world: &impl WorldReadOnly,
    record: &PublicLaneValidatorRecord,
    height: u64,
) -> bool {
    let parameters = world.sumeragi_npos_parameters();
    world.sumeragi_lanes().custody.iter().any(|obligation| {
        (effective_retains_at(obligation, parameters.as_ref(), height)
            || has_pending_evidence(world, obligation))
            && obligation.signers.as_slice().iter().any(|entry| {
                let binding = &entry.binding;
                binding.activation_height == record.activation_height
                    && binding
                        .names_account(record.lane_id, &record.validator)
                        .unwrap_or(true)
            })
    })
}

// A same-block policy extension must apply before a transaction can release custody; the
// final lane step persists those monotone fences only after transaction execution finishes.
fn effective_retains_at(
    obligation: &SumeragiLaneCustody,
    parameters: Option<&iroha_data_model::parameter::system::SumeragiNposParameters>,
    height: u64,
) -> bool {
    if obligation.validate().is_err() {
        return true;
    }
    let Some(retired) = obligation.retired_at else {
        return true;
    };
    let horizon = parameters.map_or(obligation.evidence_horizon, |p| {
        obligation.evidence_horizon.max(p.evidence_horizon_blocks())
    });
    let delay = parameters.map_or(obligation.slashing_delay, |p| {
        obligation.slashing_delay.max(p.slashing_delay_blocks())
    });
    retired
        .checked_add(horizon)
        .and_then(|end| end.checked_add(delay))
        .is_none_or(|end| height < end)
}

fn has_pending_evidence(world: &impl WorldReadOnly, obligation: &SumeragiLaneCustody) -> bool {
    world.consensus_evidence().iter().any(|(_, record)| {
        record.attribution.instance == obligation.instance && !record.penalty_status.is_terminal()
    })
}

#[cfg(test)]
mod tests;
