//! Borrowed original signer custody; no decoded signer vector or World clone is constructed.

use iroha_crypto::Hash;
use iroha_data_model::sumeragi_lanes::{
    MAX_LANE_CUSTODY_SIGNERS, SumeragiLaneCustody, SumeragiLaneCustodySigners,
    SumeragiLaneFrontier, SumeragiLaneSignerCustody, SumeragiLaneStakeBinding,
};
use iroha_model_base::topology::LaneId;

use super::{
    LanePayloadError,
    select::{field, fields, identity},
};

// These fixed current-layout readers never invoke a decoder realignment allocation. The
// enclosing payload commitment and codec-parity tests bind their layout to the model.
fn number<const N: usize>(bytes: &[u8]) -> Result<[u8; N], norito::Error> {
    bytes.try_into().map_err(|_| norito::Error::LengthMismatch)
}
fn lane_id(bytes: &[u8]) -> Result<LaneId, norito::Error> {
    let [value] = fields::<1>(bytes)?;
    Ok(LaneId::new(u32::from_le_bytes(number(value)?)))
}
fn hash(bytes: &[u8]) -> Result<Hash, norito::Error> {
    Hash::from_marked_bytes(number(bytes)?).ok_or(norito::Error::LengthMismatch)
}
fn frontier(bytes: &[u8]) -> Result<SumeragiLaneFrontier, norito::Error> {
    let [height, block_hash, result] = fields::<3>(bytes)?;
    Ok(SumeragiLaneFrontier {
        height: u64::from_le_bytes(number(height)?),
        block_hash: identity(block_hash)?,
        result: identity(result)?,
    })
}
fn optional_height(bytes: &[u8]) -> Result<Option<u64>, norito::Error> {
    match bytes {
        [0] => Ok(None),
        [1, rest @ ..] => {
            let [value] = fields::<1>(rest)?;
            Ok(Some(u64::from_le_bytes(number(value)?)))
        }
        _ => Err(norito::Error::LengthMismatch),
    }
}
fn signer(bytes: &[u8]) -> Result<SumeragiLaneSignerCustody, norito::Error> {
    let [signer, binding] = fields::<2>(bytes)?;
    let [lane, validator, activation, tenure] = fields::<4>(binding)?;
    Ok(SumeragiLaneSignerCustody {
        signer: u32::from_le_bytes(number(signer)?),
        binding: SumeragiLaneStakeBinding {
            owner_lane: lane_id(lane)?,
            validator: hash(validator)?,
            activation_height: u64::from_le_bytes(number(activation)?),
            tenure: hash(tenure)?,
        },
    })
}

/// Exact original row with fixed-size lifecycle metadata and borrowed sparse signer bytes.
/// Only `LanePayload` constructs this view after authenticating its complete original payload.
pub(crate) struct LaneCustodyView<'a> {
    // The model's fence methods are reused without copying its signer backing. All original
    // signer values are separately checked below; this private empty vector is never exposed.
    fences: SumeragiLaneCustody,
    signers: &'a [u8],
    count: usize,
}
impl<'a> LaneCustodyView<'a> {
    pub(super) fn parse(bytes: &'a [u8]) -> Result<Self, norito::Error> {
        let [
            lane,
            incarnation,
            instance,
            created,
            merged,
            count,
            signers,
            horizon,
            delay,
            retired,
        ] = fields::<10>(bytes)?;
        let fences = SumeragiLaneCustody {
            lane: lane_id(lane)?,
            incarnation: identity(incarnation)?,
            instance: identity(instance)?,
            created_at: u64::from_le_bytes(number(created)?),
            merged: frontier(merged)?,
            signer_count: u32::from_le_bytes(number(count)?),
            signers: SumeragiLaneCustodySigners::default(),
            evidence_horizon: u64::from_le_bytes(number(horizon)?),
            slashing_delay: u64::from_le_bytes(number(delay)?),
            retired_at: optional_height(retired)?,
        };
        fences
            .validate()
            .map_err(|_| norito::Error::LengthMismatch)?;
        // The bounded signer newtype has exactly one length-delimited canonical Vec field.
        let [sequence] = fields::<1>(signers)?;
        let prefix = sequence.get(..8).ok_or(norito::Error::LengthMismatch)?;
        let count = usize::try_from(u64::from_le_bytes(number(prefix)?))
            .map_err(|_| norito::Error::LengthMismatch)?;
        let signers = &sequence[8..];
        if count > MAX_LANE_CUSTODY_SIGNERS || count > signers.len() {
            return Err(norito::Error::LengthMismatch);
        }
        let result = Self {
            fences,
            signers,
            count,
        };
        let mut previous = None;
        result.visit(|entry| {
            if entry.signer >= result.fences.signer_count
                || previous.is_some_and(|previous| entry.signer <= previous)
                || entry.binding.activation_height == 0
                || entry.binding.activation_height > result.fences.created_at
            {
                return Err(norito::Error::LengthMismatch);
            }
            previous = Some(entry.signer);
            Ok(())
        })?;
        Ok(result)
    }

    fn visit(
        &self,
        mut action: impl FnMut(SumeragiLaneSignerCustody) -> Result<(), norito::Error>,
    ) -> Result<(), norito::Error> {
        let mut bytes = self.signers;
        for _ in 0..self.count {
            action(signer(field(&mut bytes)?)?)?;
        }
        if !bytes.is_empty() {
            return Err(norito::Error::LengthMismatch);
        }
        Ok(())
    }

    /// Borrowed original identity and lifetime fences. The signer vector stays private.
    pub(crate) fn identity(&self) -> (iroha_model_base::topology::LaneId, [u8; 32], [u8; 32], u64) {
        (
            self.fences.lane,
            self.fences.incarnation,
            self.fences.instance,
            self.fences.created_at,
        )
    }

    /// Last globally authenticated native frontier, including the final retirement merge.
    pub(crate) fn frontier(&self) -> iroha_data_model::sumeragi_lanes::SumeragiLaneFrontier {
        self.fences.merged
    }

    /// Pinned geometry and immutable installed global policy; these are distinct from native
    /// heights and must match the original creation committee and signed policy.
    pub(crate) fn policy(&self) -> (u32, u64, u64, Option<u64>) {
        (
            self.fences.signer_count,
            self.fences.evidence_horizon,
            self.fences.slashing_delay,
            self.fences.retired_at,
        )
    }

    /// Admission uses the immutable global lifetime fence, never a native subject height.
    pub(crate) fn admits_at(&self, height: u64) -> Result<bool, LanePayloadError> {
        self.fences
            .admits_at(height)
            .map_err(|_| LanePayloadError::Source)
    }

    /// Exact original signer tenure; absent positions remain forensic-only forever.
    pub(crate) fn binding(
        &self,
        signer: u32,
    ) -> Result<Option<SumeragiLaneStakeBinding>, norito::Error> {
        let mut selected = None;
        self.visit(|entry| {
            if entry.signer == signer {
                selected = Some(entry.binding);
            }
            Ok(())
        })?;
        Ok(selected)
    }
}

#[cfg(test)]
mod tests;
