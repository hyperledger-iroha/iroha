//! Borrowed live-State boundary for the sole transition planner; no ownership cache or replay.
use super::*;

/// Exact indexed reads from the same retained native cut as the compact summary.
///
/// Implementations must distinguish absent keys from unavailable/corrupt storage. An error is
/// preserved verbatim as `TopologyPreparationErrorV1::Lookup`; native callers must keep local
/// resource/refund refusal out of consensus-invalid execution results. These methods do not
/// request history enumeration, snapshots, map clones, or whole-state reconstruction.
pub trait TopologyIndexedReadV1 {
    /// Typed storage failure retained separately from deterministic transition rejection.
    type Error;
    /// Exact immutable latest record for the permanent original operation ID, or proven absence.
    fn operation(&self, id: &[u8; 32]) -> Result<Option<&TopologyOperationRecordV1>, Self::Error>;
    /// Whether the exact signer-key tombstone exists in this same retained cut.
    fn signer_key_seen(&self, key: &[u8; 32]) -> Result<bool, Self::Error>;
    /// Whether the exact independent-attester-key tombstone exists in the same retained cut.
    fn attester_key_seen(&self, key: &[u8; 32]) -> Result<bool, Self::Error>;
}
/// Deterministic rule failure and host storage/refund refusal occupy distinct variants.
#[derive(Debug, PartialEq, Eq)]
pub enum TopologyPreparationErrorV1<E> {
    /// A deterministic input/state invariant rejected the transition.
    Transition(TopologyTransitionErrorV1),
    /// Indexed storage was unavailable; this is not a consensus-invalid result.
    Lookup(E),
}
impl<E> From<Error> for TopologyPreparationErrorV1<E> {
    fn from(error: Error) -> Self {
        Self::Transition(error)
    }
}
impl<E: fmt::Display> fmt::Display for TopologyPreparationErrorV1<E> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transition(error) => fmt::Display::fmt(error, out),
            Self::Lookup(_) => out.write_str("topology indexed storage unavailable"),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for TopologyPreparationErrorV1<E> {}

/// Immutable claimed projection borrowed from one native State owner.
///
/// Public construction is deliberately non-authoritative: Core must derive scope, summaries and
/// all indexed rows from its own authenticated cut, then enforce actual permissions/execution.
/// Successful preparation does not authenticate caller-supplied snapshots or grant finality.
pub struct TopologyStateViewV1<'a, L: TopologyIndexedReadV1 + ?Sized> {
    /// Deployment scope pinned by the native caller.
    pub deployment: &'a str,
    /// Genesis-derived network pinned independently from submitted instructions.
    pub network: [u8; 32],
    /// Canonical chain scope.
    pub chain: &'a str,
    /// Configured canonical account discriminator.
    pub chain_discriminant: u16,
    /// Exact retained native summary, never reconstructed from the submitted action.
    pub root: &'a TopologyRetainedStateV1,
    /// Current immutable control row from this same cut.
    pub control: Option<&'a TopologyControlRecordV1>,
    /// Canonically decoded current control state from that exact row.
    pub state: Option<&'a SignerCustodyControlStateV1>,
    /// Exact native keyed reads; no whole-history or alternate cache interface.
    pub index: &'a L,
}
impl<L: TopologyIndexedReadV1 + ?Sized> TopologyStateViewV1<'_, L> {
    /// Plan one atomic transition without mutating, cloning or enumerating the retained store.
    ///
    /// # Errors
    /// Returns deterministic rule rejection separately from the original typed indexed-read error.
    pub fn prepare_claimed(
        &self,
        transition: &TopologyTransitionV1,
        context: &TopologyContextClaimV1,
    ) -> Result<TopologyPreparedTransitionV1, TopologyPreparationErrorV1<L::Error>> {
        bounded_input(transition)?;
        encode(transition, TOPOLOGY_RECORD_MAX_BYTES_V1)?;
        self.preflight(transition, context)?;
        self.validate_indexed_projection()?;
        if let TopologyActionV1::Check(request) = &transition.action {
            check::evaluate(self, request, context)?;
            return Ok(TopologyPreparedTransitionV1::unchanged());
        }
        let request_digest = digest(
            b"iroha.sorafs.topology.transition.v1\0",
            &(transition.clone(), context.execution.authority.clone()),
        )?;
        let prepared_control = control::prepare(self, transition, context, request_digest)?;
        let operation = if prepared_control.is_some() {
            operation::invalidate(self, context, request_digest)?
        } else {
            operation::prepare(self, &transition.action, context, request_digest)?
        };
        if prepared_control.is_none() && operation.is_none() {
            return Ok(TopologyPreparedTransitionV1::unchanged());
        }
        self.prepare_publication(transition, context, prepared_control, operation)
    }
    pub(super) fn operation(
        &self,
        id: &[u8; 32],
    ) -> Result<Option<&TopologyOperationRecordV1>, TopologyPreparationErrorV1<L::Error>> {
        let row = self
            .index
            .operation(id)
            .map_err(TopologyPreparationErrorV1::Lookup)?;
        if let Some(row) = row {
            if row.reviewed.request.operation_id != *id
                || row.deployment_id != self.deployment
                || row.revision == 0
                || row.revision > self.root.operation_head.revision
            {
                return Err(Error::History.into());
            }
        }
        Ok(row)
    }
    pub(super) fn preflight(
        &self,
        transition: &TopologyTransitionV1,
        context: &TopologyContextClaimV1,
    ) -> Result<(), Error> {
        self.validate_projection()?;
        if transition.deployment_id != self.deployment
            || transition.control != self.root.control_head
            || transition.operations != self.root.operation_head
        {
            return Err(Error::Conflict);
        }
        let execution = &context.execution;
        if execution.height == 0
            || execution.recorded_at_unix_ms == 0
            || execution.recorded_at_unix_ms == u64::MAX
        {
            return Err(Error::Invalid);
        }
        if let Some(previous) = &self.root.last_execution {
            if execution.height < previous.height
                || execution.recorded_at_unix_ms < previous.recorded_at_unix_ms
                || (execution.height == previous.height
                    && (previous.ordinal.checked_add(1) != Some(execution.ordinal)
                        || execution.recorded_at_unix_ms != previous.recorded_at_unix_ms))
                || (execution.height > previous.height && execution.ordinal != 0)
            {
                return Err(Error::Time);
            }
        } else if execution.ordinal != 0 {
            return Err(Error::Invalid);
        }
        if let Some(state) = self.state {
            self.validate_binding(&state.policy.binding)?;
        }
        Ok(())
    }
    pub(super) fn validate_binding(&self, binding: &SignerCustodyBindingV1) -> Result<(), Error> {
        binding.validate().map_err(|_| Error::Binding)?;
        if binding.role != SignerRoleV1::TopologyApproval
            || binding.network_id != self.network
            || binding.chain_id != self.chain
            || binding.purpose
                != (SignerPurposeBindingV1::TopologyApproval {
                    deployment_id: self.deployment.to_owned(),
                })
        {
            return Err(Error::Binding);
        }
        Ok(())
    }
    pub(super) fn anchor(
        &self,
        context: &TopologyContextClaimV1,
    ) -> Result<SignerCustodyAnchorV1, Error> {
        let anchor = context.custody_anchor.ok_or(Error::Custody)?;
        let control = self.control.ok_or(Error::Custody)?;
        if anchor.height < control.execution.height
            || anchor.height >= context.execution.height
            || anchor.block_hash == [0; 32]
            || anchor.state_digest != self.root.control_head.digest
        {
            return Err(Error::Custody);
        }
        Ok(anchor)
    }
    pub(super) fn use_current(
        &self,
        context: &TopologyContextClaimV1,
    ) -> Result<VerifiedSignerCustodyV1, Error> {
        let state = self.state.ok_or(Error::Custody)?;
        let record = self
            .control
            .and_then(|row| row.enrollment.as_deref())
            .ok_or(Error::Custody)?;
        verify_signer_custody_use_v1(
            record,
            &state.policy.binding,
            &state.policy.custody_trust(),
            &SignerCustodyUseContextV1 {
                now_unix_ms: context.execution.recorded_at_unix_ms,
                anchor_observed_at_unix_ms: context.execution.recorded_at_unix_ms,
                current_anchor: self.anchor(context)?,
                active_head: state.active_head.ok_or(Error::Custody)?,
                signer_revoked: state.signer_revoked,
                attester_revoked: state.attester_revoked,
            },
        )
        .map_err(|_| Error::Custody)
    }
    fn validate_projection(&self) -> Result<(), Error> {
        if !is_production_identity_v1(self.deployment, 128)
            || !is_production_identity_v1(self.chain, 128)
            || self.network == [0; 32]
        {
            return Err(Error::Binding);
        }
        let root = self.root;
        for (head, limit) in [
            (root.control_head, TOPOLOGY_CONTROL_LIMIT_V1),
            (root.operation_head, 2 * TOPOLOGY_OPERATION_LIMIT_V1),
            (root.history_head, TOPOLOGY_HISTORY_LIMIT_V1),
        ] {
            if (head.revision == 0) != (head.digest == [0; 32]) || head.revision > limit {
                return Err(Error::History);
            }
        }
        if root.operation_count > TOPOLOGY_OPERATION_LIMIT_V1
            || root.signer_key_count > TOPOLOGY_CONTROL_NORMAL_LIMIT_V1
            || root.signer_key_count > root.control_head.revision
            || root.attester_key_count > root.control_head.revision
            || root.attester_key_count > TOPOLOGY_CONTROL_NORMAL_LIMIT_V1
            || root.fence != root.operation_count
            || (root.active.is_some() && root.operation_count == 0)
            || root.operation_head.revision
                != 2 * root.operation_count - u64::from(root.active.is_some())
            || root.audit.sequence > root.operation_count
            || (root.audit.sequence == 0) != (root.audit.digest == [0; 32])
            || (root.active.is_some() && root.operation_count == 0)
            || (root.history_head.revision == 0) != root.last_execution.is_none()
            || root.history_head.revision < root.control_head.revision
            || root.history_head.revision < root.operation_head.revision
            || root.history_head.revision
                > root.control_head.revision + root.operation_head.revision
        {
            return Err(Error::History);
        }
        match (self.control, self.state) {
            (None, None) if root == &TopologyRetainedStateV1::empty() => Ok(()),
            (Some(row), Some(state)) => {
                if row.deployment_id != self.deployment || row.revision != root.control_head.revision
                    || digest(b"iroha.sorafs.topology.control.v1\0", row)? != root.control_head.digest
                    || encode(state, sorafs_manifest::signer::custody_control::SIGNER_CUSTODY_CONTROL_MAX_BYTES_V1)? != row.control_state
                    || root.signer_key_count == 0 || root.attester_key_count == 0 {
                    return Err(Error::History);
                }
                state.validate().map_err(|_| Error::Custody)?;
                self.validate_binding(&state.policy.binding)
            }
            _ => Err(Error::History),
        }
    }
    fn validate_indexed_projection(&self) -> Result<(), TopologyPreparationErrorV1<L::Error>> {
        if let Some(state) = self.state {
            let signer = digest(
                b"iroha.sorafs.topology.key-tombstone.v1\0",
                &state.policy.binding.public_key,
            )?;
            let attester = digest(
                b"iroha.sorafs.topology.key-tombstone.v1\0",
                &state.policy.attester_public_key,
            )?;
            let signer_seen = self
                .index
                .signer_key_seen(&signer)
                .map_err(TopologyPreparationErrorV1::Lookup)?;
            let attester_seen = self
                .index
                .attester_key_seen(&attester)
                .map_err(TopologyPreparationErrorV1::Lookup)?;
            if !signer_seen || !attester_seen {
                return Err(Error::History.into());
            }
        }
        if let Some(id) = self.root.active {
            let row = self.operation(&id)?.ok_or(Error::History)?;
            if row.outcome != TopologyOutcomeV1::Reserved
                || row.revision != self.root.operation_head.revision
                || digest(b"iroha.sorafs.topology.operation.v1\0", row)?
                    != self.root.operation_head.digest
                || row.reviewed.intent.previous_audit != self.root.audit
                || row.reservation.fence != self.root.fence
                || row.reviewed.request.original_custody.control_state_digest
                    != self.root.control_head.digest
                || self
                    .state
                    .and_then(|state| state.active_head)
                    .is_none_or(|head| {
                        head.record_digest != row.reviewed.request.original_custody.record_digest
                    })
            {
                return Err(Error::History.into());
            }
        }
        Ok(())
    }
}
