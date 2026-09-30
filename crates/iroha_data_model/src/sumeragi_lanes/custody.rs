//! Exact original stake bindings and global lifetime fences of pinned lane incarnations.

use iroha_crypto::Hash;
use iroha_model_base::topology::LaneId;
use iroha_schema::IntoSchema;
use norito::{Decode, Encode};

use crate::{
    DeriveJsonDeserialize, DeriveJsonSerialize, account::AccountId, asset::AssetId,
    nexus::PublicLaneValidatorRecord,
};

mod signers;
pub use signers::{SumeragiLaneCustodySigners, SumeragiLaneSignerCustody};

/// Largest native-core committee representation; authenticated execution separately checks
/// lane authority geometry. This bound does not grant committee admission.
pub const MAX_LANE_CUSTODY_SIGNERS: usize = iroha_sumeragi::types::MAX_COMMITTEE_SIZE;
/// Bounded live and not-yet-released incarnation obligations in global state.
pub const MAX_LANE_CUSTODY_OBLIGATIONS: usize = 4_096;

/// Fixed-size identity of one original staking tenure, never a current-key lookup.
///
/// The tenure commitment includes the canonical account, peer, activation height and exact
/// escrow asset. It excludes mutable balances and lifecycle labels. A later registration or
/// a substituted custody asset cannot become a monetary target by reusing the peer key.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    norito::NoritoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::sumeragi_lanes::SumeragiLaneStakeBinding")]
pub struct SumeragiLaneStakeBinding {
    /// Original canonical staking owner, independent of the physical consensus lane.
    pub owner_lane: LaneId,
    /// Domain-separated identity of the original universal validator account.
    pub validator: Hash,
    /// Inclusive activation boundary of the original registration, never a lane height.
    pub activation_height: u64,
    /// Exact immutable registration and custody identity.
    pub tenure: Hash,
}

impl SumeragiLaneStakeBinding {
    /// Bind a genuine original staking record and its independently retained custody asset.
    ///
    /// The caller authenticates the record and asset from one original global state cut.
    /// This pure projection does not establish that a validator was selected for a lane.
    /// # Errors
    /// A canonical field cannot be encoded or the retained tenure has inconsistent bounds.
    pub fn from_record(
        record: &PublicLaneValidatorRecord,
        custody: &AssetId,
    ) -> Result<Self, String> {
        if record.activation_height == 0
            || record
                .deactivation_height
                .is_some_and(|end| end < record.activation_height)
        {
            return Err("invalid original lane stake tenure".into());
        }
        let validator = account_commitment(&record.validator)?;
        let tenure = Hash::new_from_writer(|writer| {
            writer.write_all(b"iroha:sumeragi:lane:stake-tenure:v1\0")?;
            writer.write_all(&record.lane_id.as_u32().to_le_bytes())?;
            writer.write_all(&record.activation_height.to_le_bytes())?;
            norito::core::write_canonical_to_writer(&record.validator, writer)
                .map_err(std::io::Error::other)?;
            norito::core::write_canonical_to_writer(&record.peer_id, writer)
                .map_err(std::io::Error::other)?;
            norito::core::write_canonical_to_writer(custody, writer).map_err(std::io::Error::other)
        })
        .map_err(|error| error.to_string())?;
        Ok(Self {
            owner_lane: record.lane_id,
            validator,
            activation_height: record.activation_height,
            tenure,
        })
    }

    /// Cheap account selection before comparing the exact retained tenure and custody.
    /// # Errors
    /// The canonical account cannot be encoded.
    pub fn names_account(&self, lane: LaneId, account: &AccountId) -> Result<bool, String> {
        Ok(self.owner_lane == lane && self.validator == account_commitment(account)?)
    }
}

fn account_commitment(account: &AccountId) -> Result<Hash, String> {
    Hash::new_from_writer(|writer| {
        writer.write_all(b"iroha:sumeragi:lane:stake-account:v1\0")?;
        norito::core::write_canonical_to_writer(account, writer).map_err(std::io::Error::other)
    })
    .map_err(|error| error.to_string())
}

/// Original committee obligations retained independently of routing and physical lane storage.
///
/// Each stored signer index is exactly its native committee index. An absent index is a forensic-only member whose
/// creation did not bind genuine stake; future registration never changes that decision.
/// Native lane heights are deliberately absent from these global-clock lifetime fences.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    Encode,
    Decode,
    IntoSchema,
    norito::NoritoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(deny_unknown_fields)]
#[norito_schema(name = "iroha_data_model::sumeragi_lanes::SumeragiLaneCustody")]
pub struct SumeragiLaneCustody {
    /// Physical lane whose instance has this obligation.
    pub lane: LaneId,
    /// Never-reused native lane incarnation.
    pub incarnation: [u8; 32],
    /// Exact network- and chain-bound native instance of the original incarnation.
    pub instance: [u8; 32],
    /// Authenticated global creation height.
    pub created_at: u64,
    /// Pinned exact native committee size.
    pub signer_count: u32,
    /// Sparse original stake identities, strictly ordered by native signer index.
    pub signers: SumeragiLaneCustodySigners,
    /// Signed global evidence policy pinned at creation; later policy can only extend it.
    pub evidence_horizon: u64,
    /// Signed mandatory-penalty delay pinned at creation; later policy can only extend it.
    pub slashing_delay: u64,
    /// First global height at which the original lane is retired, authenticated by execution.
    #[norito(required)]
    pub retired_at: Option<u64>,
}

impl SumeragiLaneCustody {
    /// Check bounded geometry and overflow-safe global lifetime fences.
    /// # Errors
    /// Invalid incarnation, committee size, unused slot, policy or retirement boundary.
    pub fn validate(&self) -> Result<(), &'static str> {
        let count = usize::try_from(self.signer_count).map_err(|_| "invalid signer count")?;
        if self.lane.as_u32() == 0
            || self.incarnation == [0; 32]
            || self.instance == [0; 32]
            || self.created_at == 0
            || !(1..=MAX_LANE_CUSTODY_SIGNERS).contains(&count)
            || self.signers.validate().is_err()
            || self.signers.as_slice().iter().any(|entry| {
                entry.signer >= self.signer_count
                    || entry.binding.activation_height == 0
                    || entry.binding.activation_height > self.created_at
            })
            || self.evidence_horizon == 0
            || self.slashing_delay == 0
            || self
                .retired_at
                .is_some_and(|height| height <= self.created_at)
        {
            return Err("invalid lane custody obligation");
        }
        self.release_height()?;
        Ok(())
    }

    /// Last global carrier allowed to admit a report; `None` while the incarnation is live.
    /// # Errors
    /// The authenticated retirement and horizon overflow the global height domain.
    pub fn admission_deadline(&self) -> Result<Option<u64>, &'static str> {
        self.retired_at
            .map(|retired| {
                retired
                    .checked_add(self.evidence_horizon)
                    .ok_or("lane evidence deadline overflow")
            })
            .transpose()
    }

    /// First global block whose transaction phase may release the original custody.
    /// Its mandatory effects phase has already applied the last permitted delayed penalty.
    /// # Errors
    /// Retirement, horizon or delay overflow the global height domain.
    pub fn release_height(&self) -> Result<Option<u64>, &'static str> {
        self.admission_deadline()?
            .map(|deadline| {
                deadline
                    .checked_add(self.slashing_delay)
                    .ok_or("lane custody release overflow")
            })
            .transpose()
    }

    /// Whether this original obligation permits admission at a global carrier height.
    /// # Errors
    /// The obligation is structurally invalid or its lifetime fence overflows.
    pub fn admits_at(&self, carrier: u64) -> Result<bool, &'static str> {
        self.validate()?;
        Ok(
            carrier > self.created_at
                && self.admission_deadline()?.is_none_or(|end| carrier <= end),
        )
    }

    /// Whether original custody still has a live or delayed-penalty obligation at this height.
    /// # Errors
    /// The obligation is structurally invalid or its lifetime fence overflows.
    pub fn retains_at(&self, height: u64) -> Result<bool, &'static str> {
        self.validate()?;
        Ok(self
            .release_height()?
            .is_none_or(|release| height < release))
    }
}

#[cfg(test)]
mod tests;
