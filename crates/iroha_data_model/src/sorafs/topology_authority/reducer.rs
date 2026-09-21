//! One pure reducer for transition planning and complete bounded replay; no native authority token.
use super::*;
use iroha_primitives::production_identity::is_production_identity_v1;
use sorafs_manifest::signer::{
    custody::{
        SignerCustodyBindingV1, SignerCustodyUseContextV1, VerifiedSignerCustodyV1,
        verify_signer_custody_use_v1,
    },
    custody_control::SignerCustodyControlStateV1,
    protocol::{SignerPurposeBindingV1, SignerRoleV1},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};
mod check;
mod control;
mod operation;
mod publish;
mod view;
pub use view::{TopologyIndexedReadV1, TopologyPreparationErrorV1, TopologyStateViewV1};

/// Payload-free failure of pure topology transition validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopologyTransitionErrorV1 {
    /// Invalid bounds, canonical frame, identity or execution coordinates.
    Invalid,
    /// Exact deployment, role, network, candidate or whole binding differs.
    Binding,
    /// CAS, original operation, audit or immutable outcome differs.
    Conflict,
    /// Bounded history/key/id capacity or monotonic counter exhausted.
    Capacity,
    /// Independent custody attestation or claimed active state is ineligible.
    Custody,
    /// Review, custody, reservation or execution time is invalid.
    Time,
    /// Key history or governed policy generations are inconsistent.
    Generation,
    /// Claimed replay has missing, substituted or extra rows.
    History,
}
impl fmt::Display for TopologyTransitionErrorV1 {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Invalid => "invalid topology transition",
            Self::Binding => "topology transition binding mismatch",
            Self::Conflict => "topology transition conflict",
            Self::Capacity => "topology transition capacity exhausted",
            Self::Custody => "topology custody is ineligible",
            Self::Time => "topology transition time mismatch",
            Self::Generation => "topology key or policy generation mismatch",
            Self::History => "incomplete or substituted topology history",
        })
    }
}
impl std::error::Error for TopologyTransitionErrorV1 {}
type Error = TopologyTransitionErrorV1;

/// Planned exact writes. These records are claims until Core validates and atomically persists them.
#[derive(Debug, PartialEq, Eq)]
pub struct TopologyTransitionDeltaV1 {
    /// Exact next retained summary; absent only when no native write is required.
    pub next: Option<TopologyRetainedStateV1>,
    /// Complete replay entry, absent for a no-write Check or exact idempotent observation.
    pub history: Option<TopologyHistoryEntryV1>,
    /// New immutable control row; operations never rewrite an old control row.
    pub control: Option<TopologyControlRecordV1>,
    /// New immutable operation row; a control change can invalidate the original active slot.
    pub operation: Option<TopologyOperationRecordV1>,
    /// New immutable signer-key tombstone; Core must persist it with this same delta.
    pub signer_key: Option<[u8; 32]>,
    /// New immutable independent-attester-key tombstone; never inferred from an unrelated row.
    pub attester_key: Option<[u8; 32]>,
}
impl TopologyTransitionDeltaV1 {
    fn unchanged() -> Self {
        Self {
            next: None,
            history: None,
            control: None,
            operation: None,
            signer_key: None,
            attester_key: None,
        }
    }
}

/// Owning cold-replay accumulator for the sole borrowed transition rules.
///
/// This owns uncharged maps and is not a production live-State cache. Core must retain native
/// indexed rows under its original transaction owners and call `TopologyStateViewV1` directly.
/// Complete authenticated recovery may use this accumulator only under separate admission.
#[derive(Debug)]
pub struct TopologyTransitionModelV1 {
    deployment: String,
    network: [u8; 32],
    chain: String,
    chain_discriminant: u16,
    root: TopologyRetainedStateV1,
    control: Option<TopologyControlRecordV1>,
    state: Option<SignerCustodyControlStateV1>,
    operations: BTreeMap<[u8; 32], TopologyOperationRecordV1>,
    signer_keys: BTreeSet<[u8; 32]>,
    attester_keys: BTreeSet<[u8; 32]>,
}
impl TopologyTransitionModelV1 {
    /// Begin a genuinely empty claimed prefix, never a replacement for missing persisted rows.
    ///
    /// # Errors
    /// Rejects malformed independently expected deployment/chain labels or a zero network.
    pub fn new(
        deployment: String,
        network: [u8; 32],
        chain: String,
        chain_discriminant: u16,
    ) -> Result<Self, Error> {
        if !is_production_identity_v1(&deployment, 128)
            || !is_production_identity_v1(&chain, 128)
            || network == [0; 32]
        {
            return Err(Error::Binding);
        }
        Ok(Self {
            deployment,
            network,
            chain,
            chain_discriminant,
            root: TopologyRetainedStateV1::empty(),
            control: None,
            state: None,
            operations: BTreeMap::new(),
            signer_keys: BTreeSet::new(),
            attester_keys: BTreeSet::new(),
        })
    }
    /// Borrow the exact retained summary; no decoded summary grants native authority.
    #[must_use]
    pub const fn retained(&self) -> &TopologyRetainedStateV1 {
        &self.root
    }
    /// Exact claimed custody prefix, independent of operation progress.
    #[must_use]
    pub const fn control_head(&self) -> TopologyHeadV1 {
        self.root.control_head
    }
    /// Exact claimed operation prefix.
    #[must_use]
    pub const fn operation_head(&self) -> TopologyHeadV1 {
        self.root.operation_head
    }
    /// Exact complete claimed replay prefix.
    #[must_use]
    pub const fn history_head(&self) -> TopologyHeadV1 {
        self.root.history_head
    }
    /// Current claimed audit, independent of pending/expired/invalidated operations.
    #[must_use]
    pub const fn audit(&self) -> SignerOperationAuditHeadV1 {
        self.root.audit
    }
    /// Retained immutable control row.
    #[must_use]
    pub const fn control(&self) -> Option<&TopologyControlRecordV1> {
        self.control.as_ref()
    }
    /// Retained permanent operation row by original id.
    #[must_use]
    pub fn operation(&self, id: &[u8; 32]) -> Option<&TopologyOperationRecordV1> {
        self.operations.get(id)
    }
    /// Complete operation/ID inventory for comparison with persisted recovery indices.
    pub fn operation_inventory(
        &self,
    ) -> impl Iterator<Item = (&[u8; 32], &TopologyOperationRecordV1)> {
        self.operations.iter()
    }
    /// Complete signer-key tombstones for strict recovery comparison.
    pub fn signer_key_inventory(&self) -> impl Iterator<Item = &[u8; 32]> {
        self.signer_keys.iter()
    }
    /// Complete independent-attester-key tombstones for strict recovery comparison.
    pub fn attester_key_inventory(&self) -> impl Iterator<Item = &[u8; 32]> {
        self.attester_keys.iter()
    }
    /// Borrow the same immutable inputs consumed by native indexed storage; never clone maps.
    #[must_use]
    pub fn view(&self) -> TopologyStateViewV1<'_, Self> {
        TopologyStateViewV1 {
            deployment: &self.deployment,
            network: self.network,
            chain: &self.chain,
            chain_discriminant: self.chain_discriminant,
            root: &self.root,
            control: self.control.as_ref(),
            state: self.state.as_ref(),
            index: self,
        }
    }
    /// Prepare through the sole borrowed reducer, then append only its already prepared rows.
    ///
    /// # Errors
    /// Rejects invalid semantics or claimed storage. No partial model mutation is published.
    pub fn apply_claimed(
        &mut self,
        transition: &TopologyTransitionV1,
        context: &TopologyContextClaimV1,
    ) -> Result<TopologyTransitionDeltaV1, Error> {
        let prepared =
            self.view()
                .prepare_claimed(transition, context)
                .map_err(|error| match error {
                    TopologyPreparationErrorV1::Transition(error) => error,
                    TopologyPreparationErrorV1::Lookup(never) => match never {},
                })?;
        let TopologyPreparedTransitionV1 {
            delta,
            control_state,
        } = prepared;
        if let Some(root) = &delta.next {
            // Owning replay is deliberately separate from native MV publication/admission.
            if let Some(key) = delta.signer_key {
                self.signer_keys.insert(key);
            }
            if let Some(key) = delta.attester_key {
                self.attester_keys.insert(key);
            }
            if let Some(state) = control_state {
                self.state = Some(state);
            }
            if let Some(row) = &delta.control {
                self.control = Some(row.clone());
            }
            if let Some(row) = &delta.operation {
                self.operations
                    .insert(row.reviewed.request.operation_id, row.clone());
            }
            self.root = root.clone();
        }
        Ok(delta)
    }
    /// Reconstruct all fences, audit heads, ID/key tombstones and current custody from exact history.
    ///
    /// The complete retained summary is required even for an empty prefix and must be pinned by native storage.
    /// No snapshot decoder or missing-row fallback bypasses the same transition reducer.
    ///
    /// # Errors
    /// Rejects malformed/oversized entries, missing/interchanged/duplicate/extra rows, state/result
    /// substitution, history exhaustion, and a terminal head that differs from the supplied prefix.
    pub fn restore_claimed(
        deployment: String,
        network: [u8; 32],
        chain: String,
        chain_discriminant: u16,
        expected: &TopologyRetainedStateV1,
        frames: impl IntoIterator<Item = Vec<u8>>,
    ) -> Result<Self, Error> {
        if (expected.history_head.revision == 0) != (expected.history_head.digest == [0; 32])
            || expected.history_head.revision > TOPOLOGY_HISTORY_LIMIT_V1
        {
            return Err(Error::History);
        }
        let mut model = Self::new(deployment, network, chain, chain_discriminant)?;
        for bytes in frames {
            if model.root.history_head.revision >= expected.history_head.revision {
                return Err(Error::History);
            }
            let entry: TopologyHistoryEntryV1 = decode(&bytes, TOPOLOGY_HISTORY_MAX_BYTES_V1)?;
            if entry.revision
                != model
                    .root
                    .history_head
                    .revision
                    .checked_add(1)
                    .ok_or(Error::History)?
                || entry.predecessor_digest != model.root.history_head.digest
            {
                return Err(Error::History);
            }
            let delta = model.apply_claimed(&entry.transition, &entry.context)?;
            if delta.history.as_ref() != Some(&entry) {
                return Err(Error::History);
            }
        }
        if &model.root != expected {
            return Err(Error::History);
        }
        Ok(model)
    }
}
impl TopologyIndexedReadV1 for TopologyTransitionModelV1 {
    type Error = std::convert::Infallible;
    fn operation(&self, id: &[u8; 32]) -> Result<Option<&TopologyOperationRecordV1>, Self::Error> {
        Ok(self.operations.get(id))
    }
    fn signer_key_seen(&self, key: &[u8; 32]) -> Result<bool, Self::Error> {
        Ok(self.signer_keys.contains(key))
    }
    fn attester_key_seen(&self, key: &[u8; 32]) -> Result<bool, Self::Error> {
        Ok(self.attester_keys.contains(key))
    }
}
/// Immutable prepared rows from the sole reducer; no signing or finality capability.
///
/// Native execution must consume these rows under the same State transaction after actual
/// permissions, execution coordinates and storage admission have been established.
#[derive(Debug, PartialEq, Eq)]
pub struct TopologyPreparedTransitionV1 {
    delta: TopologyTransitionDeltaV1,
    control_state: Option<SignerCustodyControlStateV1>,
}
impl TopologyPreparedTransitionV1 {
    /// Inspect the exact rows without allowing substitution inside this owner.
    #[must_use]
    pub const fn delta(&self) -> &TopologyTransitionDeltaV1 {
        &self.delta
    }
    /// Consume prepared rows; their public wire values remain claims outside native execution.
    #[must_use]
    pub fn into_delta(self) -> TopologyTransitionDeltaV1 {
        self.delta
    }
    fn unchanged() -> Self {
        Self {
            delta: TopologyTransitionDeltaV1::unchanged(),
            control_state: None,
        }
    }
}

fn bounded_input(transition: &TopologyTransitionV1) -> Result<(), Error> {
    fn reviewed(value: &TopologyReserveV1) -> bool {
        value.subject.deployment_id.len() <= 128 && value.subject.chain_id.len() <= 128
    }
    let bounded = match &transition.action {
        TopologyActionV1::Configure(bytes) | TopologyActionV1::Enroll(bytes) => {
            bytes.len()
                <= sorafs_manifest::signer::custody_control::SIGNER_CUSTODY_CONTROL_MAX_BYTES_V1
        }
        TopologyActionV1::Reserve(value) => reviewed(value),
        TopologyActionV1::Check(value) => {
            reviewed(&value.reviewed)
                && match &value.phase {
                    TopologyCheckPhaseV1::Current(_) => true,
                    TopologyCheckPhaseV1::BeforeProvider(row)
                    | TopologyCheckPhaseV1::AfterProvider(row)
                    | TopologyCheckPhaseV1::BeforeCommit(row)
                    | TopologyCheckPhaseV1::AfterCommit(row)
                    | TopologyCheckPhaseV1::BeforeRelease(row) => {
                        row.deployment_id.len() <= 128 && reviewed(&row.reviewed)
                    }
                }
        }
        _ => true,
    };
    if transition.deployment_id.len() > 128 || !bounded {
        return Err(Error::Invalid);
    }
    Ok(())
}

fn encode<T: norito::core::NoritoSerialize>(value: &T, maximum: usize) -> Result<Vec<u8>, Error> {
    if norito::canonical_frame_len(value).map_err(|_| Error::Invalid)? > maximum {
        return Err(Error::Invalid);
    }
    let bytes = norito::encode_canonical(value).map_err(|_| Error::Invalid)?;
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(Error::Invalid);
    }
    Ok(bytes)
}
fn decode<T>(bytes: &[u8], maximum: usize) -> Result<T, Error>
where
    T: norito::core::NoritoSerialize + for<'de> norito::core::NoritoDeserialize<'de>,
{
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(Error::Invalid);
    }
    norito::decode_canonical_with_limits(
        bytes,
        norito::DecodeLimits::new(16 * 1024, maximum, 8192, 512 * 1024, 24),
    )
    .map_err(|_| Error::Invalid)
}
fn digest<T: norito::core::NoritoSerialize>(domain: &[u8], value: &T) -> Result<[u8; 32], Error> {
    let bytes = encode(value, TOPOLOGY_HISTORY_MAX_BYTES_V1)?;
    let mut hash = blake3::Hasher::new();
    hash.update(domain);
    hash.update(&(bytes.len() as u64).to_be_bytes());
    hash.update(&bytes);
    Ok(*hash.finalize().as_bytes())
}

#[cfg(test)]
mod tests;
