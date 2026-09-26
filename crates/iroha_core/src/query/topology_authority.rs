//! Read-only role-16 StatePath and bounded row decoding, without native admission.
//!
//! These same-World reads establish local row/index consistency only. They do not authenticate
//! execution, complete replay, Kura/QC finality, signer use, or a successful Check. The role-16
//! instruction remains closed until the native transaction owner and funded recovery are wired.

use crate::state::{StateReadOnly, WorldReadOnly};
use iroha_data_model::sorafs::topology_authority::{
    TOPOLOGY_AUTHORITY_NAMESPACE_V1, TOPOLOGY_CONTROL_LIMIT_V1, TOPOLOGY_HISTORY_LIMIT_V1,
    TOPOLOGY_HISTORY_MAX_BYTES_V1, TOPOLOGY_OPERATION_LIMIT_V1, TOPOLOGY_RECORD_MAX_BYTES_V1,
    TopologyControlRecordV1, TopologyHeadV1, TopologyHistoryEntryV1, TopologyOperationRecordV1,
    TopologyOutcomeV1, TopologyRetainedStateV1,
};
use iroha_model_base::state_path::StatePath;
use iroha_primitives::production_identity::is_production_identity_v1;
use mv::storage::StorageReadOnly;
use norito::core::{NoritoDeserialize, NoritoSerialize};
use sorafs_manifest::signer::{
    custody_control::{SIGNER_CUSTODY_CONTROL_MAX_BYTES_V1, SignerCustodyControlStateV1},
    protocol::{SignerPurposeBindingV1, SignerRoleV1},
};

/// Failure to parse a scope or to reconcile a bounded retained row with its index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopologyRowErrorV1 {
    /// The deployment identity cannot name a production role-16 namespace.
    InvalidScope,
    /// A native key, bounded canonical frame, digest, row, or index is inconsistent.
    CorruptHistory,
}

/// One borrowed World cut. No owned map or replay accumulator is created.
pub struct TopologyRowsV1<'a, W: WorldReadOnly + ?Sized> {
    world: &'a W,
    deployment: &'a str,
}

impl<'a, W: WorldReadOnly + ?Sized> TopologyRowsV1<'a, W> {
    /// Pin a production deployment to one borrowed World view.
    ///
    /// # Errors
    /// Rejects malformed, reserved, or oversized deployment labels.
    pub fn new(world: &'a W, deployment: &'a str) -> Result<Self, TopologyRowErrorV1> {
        if !is_production_identity_v1(deployment, 128) {
            return Err(TopologyRowErrorV1::InvalidScope);
        }
        Ok(Self { world, deployment })
    }

    fn path(&self, suffix: &str) -> StatePath {
        let scope = hex::encode(blake3::hash(self.deployment.as_bytes()).as_bytes());
        format!("{TOPOLOGY_AUTHORITY_NAMESPACE_V1}_{scope}_{suffix}")
            .parse()
            .expect("fixed bounded role-16 StatePath")
    }

    fn has_prefix(&self, suffix: &str) -> bool {
        let start = self.path(suffix);
        self.world
            .smart_contract_state()
            .range(start.clone()..)
            .next()
            .is_some_and(|(key, _)| key.as_ref().starts_with(start.as_ref()))
    }

    fn decode<T>(&self, bytes: &[u8], maximum: usize) -> Result<T, TopologyRowErrorV1>
    where
        T: NoritoSerialize + for<'de> NoritoDeserialize<'de>,
    {
        if bytes.is_empty() || bytes.len() > maximum {
            return Err(TopologyRowErrorV1::CorruptHistory);
        }
        norito::decode_canonical_with_limits(
            bytes,
            norito::DecodeLimits::new(16 * 1024, maximum, 8192, 512 * 1024, 24),
        )
        .map_err(|_| TopologyRowErrorV1::CorruptHistory)
    }

    fn digest<T>(&self, domain: &[u8], value: &T) -> Result<[u8; 32], TopologyRowErrorV1>
    where
        T: NoritoSerialize,
    {
        if norito::canonical_frame_len(value).map_err(|_| TopologyRowErrorV1::CorruptHistory)?
            > TOPOLOGY_HISTORY_MAX_BYTES_V1
        {
            return Err(TopologyRowErrorV1::CorruptHistory);
        }
        let bytes =
            norito::encode_canonical(value).map_err(|_| TopologyRowErrorV1::CorruptHistory)?;
        let mut hash = blake3::Hasher::new();
        hash.update(domain);
        hash.update(&(bytes.len() as u64).to_be_bytes());
        hash.update(&bytes);
        Ok(*hash.finalize().as_bytes())
    }

    fn read_head_row<T>(
        &self,
        prefix: &str,
        head: TopologyHeadV1,
        domain: &[u8],
        maximum: usize,
    ) -> Result<Option<T>, TopologyRowErrorV1>
    where
        T: NoritoSerialize + for<'de> NoritoDeserialize<'de>,
    {
        if head.revision == 0 {
            return (!self.has_prefix(prefix))
                .then_some(None)
                .ok_or(TopologyRowErrorV1::CorruptHistory);
        }
        let prefix_path = self.path(prefix);
        let first_path = self.path(&format!("{prefix}{:020}", 1));
        let path = self.path(&format!("{prefix}{:020}", head.revision));
        let rows = self.world.smart_contract_state();
        if rows.range(prefix_path.clone()..).next().map(|(key, _)| key) != Some(&first_path) {
            return Err(TopologyRowErrorV1::CorruptHistory);
        }
        let mut at_or_after_head = rows.range(path.clone()..);
        if at_or_after_head.next().map(|(key, _)| key) != Some(&path)
            || at_or_after_head
                .next()
                .is_some_and(|(key, _)| key.as_ref().starts_with(prefix_path.as_ref()))
        {
            return Err(TopologyRowErrorV1::CorruptHistory);
        }
        let bytes = rows.get(&path).ok_or(TopologyRowErrorV1::CorruptHistory)?;
        let row = self.decode(bytes, maximum)?;
        if self.digest(domain, &row)? != head.digest {
            return Err(TopologyRowErrorV1::CorruptHistory);
        }
        Ok(Some(row))
    }

    /// Check gap-free revision keys and raw frame lengths without owning replay state.
    ///
    /// This scan borrows one World view and keeps constant additional memory. It rejects a
    /// missing or malformed middle key that an indexed terminal read would not see. It does
    /// not decode intermediate rows, apply the reducer, authenticate execution, or grant signer
    /// authority. Full cold replay still requires funded ownership before its allocations.
    ///
    /// # Errors
    /// Rejects missing, extra or noncanonical revision keys and empty or oversized raw frames.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "funded cold replay is not connected to native admission"
        )
    )]
    pub(crate) fn validate_local_revision_prefixes(&self) -> Result<(), TopologyRowErrorV1> {
        // TODO: Fund and authenticate full reducer replay before this local check can feed authority.
        let root = self.retained()?;
        self.validate_revision_prefix(
            "history_revision_",
            root.history_head,
            TOPOLOGY_HISTORY_MAX_BYTES_V1,
        )?;
        self.validate_revision_prefix(
            "control_revision_",
            root.control_head,
            TOPOLOGY_RECORD_MAX_BYTES_V1,
        )?;
        self.validate_revision_prefix(
            "operation_revision_",
            root.operation_head,
            TOPOLOGY_RECORD_MAX_BYTES_V1,
        )
    }

    fn validate_revision_prefix(
        &self,
        suffix: &str,
        head: TopologyHeadV1,
        maximum: usize,
    ) -> Result<(), TopologyRowErrorV1> {
        let prefix = self.path(suffix);
        let mut expected = 1_u64;
        for (key, bytes) in self.world.smart_contract_state().range(prefix.clone()..) {
            let Some(revision) = key.as_ref().strip_prefix(prefix.as_ref()) else {
                break;
            };
            if expected > head.revision
                || revision.len() != 20
                || !revision.bytes().all(|byte| byte.is_ascii_digit())
                || revision.parse::<u64>() != Ok(expected)
                || bytes.is_empty()
                || bytes.len() > maximum
            {
                return Err(TopologyRowErrorV1::CorruptHistory);
            }
            expected = expected
                .checked_add(1)
                .ok_or(TopologyRowErrorV1::CorruptHistory)?;
        }
        if expected.checked_sub(1) != Some(head.revision) {
            return Err(TopologyRowErrorV1::CorruptHistory);
        }
        Ok(())
    }

    /// Read a retained summary and its exact current history row from this World.
    ///
    /// An absent root is an empty prefix only if no role-16 key exists for this deployment.
    /// This is a local read, not complete authenticated recovery or execution authority.
    ///
    /// # Errors
    /// Rejects orphan rows, malformed frames, impossible heads and a substituted terminal row.
    pub fn retained(&self) -> Result<TopologyRetainedStateV1, TopologyRowErrorV1> {
        let root_key = self.path("retained");
        let Some(bytes) = self.world.smart_contract_state().get(&root_key) else {
            if self.has_prefix("") {
                return Err(TopologyRowErrorV1::CorruptHistory);
            }
            return Ok(TopologyRetainedStateV1::empty());
        };
        let root: TopologyRetainedStateV1 = self.decode(bytes, TOPOLOGY_RECORD_MAX_BYTES_V1)?;
        for (head, maximum) in [
            (root.control_head, TOPOLOGY_CONTROL_LIMIT_V1),
            (root.operation_head, 2 * TOPOLOGY_OPERATION_LIMIT_V1),
            (root.history_head, TOPOLOGY_HISTORY_LIMIT_V1),
        ] {
            if (head.revision == 0) != (head.digest == [0; 32]) || head.revision > maximum {
                return Err(TopologyRowErrorV1::CorruptHistory);
            }
        }
        if root.history_head.revision == 0
            || root.last_execution.is_none()
            || root.control_head.revision > root.history_head.revision
            || root.operation_head.revision > root.history_head.revision
        {
            return Err(TopologyRowErrorV1::CorruptHistory);
        }
        let current: TopologyHistoryEntryV1 = self
            .read_head_row(
                "history_revision_",
                root.history_head,
                b"iroha.sorafs.topology.history.v1\0",
                TOPOLOGY_HISTORY_MAX_BYTES_V1,
            )?
            .ok_or(TopologyRowErrorV1::CorruptHistory)?;
        if current.revision != root.history_head.revision
            || current.transition.deployment_id != self.deployment
            || current.control != root.control_head
            || current.operations != root.operation_head
            || root.last_execution.as_ref() != Some(&current.context.execution)
        {
            return Err(TopologyRowErrorV1::CorruptHistory);
        }
        Ok(root)
    }

    /// Read the exact current control row by its independent retained head.
    ///
    /// # Errors
    /// Rejects missing, malformed, substituted or cross-deployment rows.
    pub fn current_control(&self) -> Result<Option<TopologyControlRecordV1>, TopologyRowErrorV1> {
        let root = self.retained()?;
        let row: Option<TopologyControlRecordV1> = self.read_head_row(
            "control_revision_",
            root.control_head,
            b"iroha.sorafs.topology.control.v1\0",
            TOPOLOGY_RECORD_MAX_BYTES_V1,
        )?;
        if row.as_ref().is_some_and(|value| {
            value.deployment_id != self.deployment || value.revision != root.control_head.revision
        }) {
            return Err(TopologyRowErrorV1::CorruptHistory);
        }
        Ok(row)
    }

    /// Read one permanent operation ID through its immutable admission and latest-slot indexes.
    ///
    /// # Errors
    /// Rejects index/row mismatch, missing original Reserve, a nonadjacent terminal revision,
    /// malformed frames and cross-deployment substitution.
    pub fn operation(
        &self,
        id: [u8; 32],
    ) -> Result<Option<TopologyOperationRecordV1>, TopologyRowErrorV1> {
        let root = self.retained()?;
        let slot = self
            .world
            .smart_contract_state()
            .get(&self.path(&format!("operation_id_{}", hex::encode(id))));
        let admission = self
            .world
            .smart_contract_state()
            .get(&self.path(&format!("operation_admission_{}", hex::encode(id))));
        let (Some(slot), Some(admission)) = (slot, admission) else {
            return if slot.is_none() && admission.is_none() {
                if root.active == Some(id) {
                    Err(TopologyRowErrorV1::CorruptHistory)
                } else {
                    Ok(None)
                }
            } else {
                Err(TopologyRowErrorV1::CorruptHistory)
            };
        };
        let revision: u64 = self.decode(slot, TOPOLOGY_RECORD_MAX_BYTES_V1)?;
        let admitted: u64 = self.decode(admission, TOPOLOGY_RECORD_MAX_BYTES_V1)?;
        if admitted == 0
            || revision < admitted
            || revision > root.operation_head.revision
            || admitted.checked_add(1).is_none_or(|next| revision > next)
        {
            return Err(TopologyRowErrorV1::CorruptHistory);
        }
        let original = self.operation_row(admitted)?;
        let current = self.operation_row(revision)?;
        if original.outcome != TopologyOutcomeV1::Reserved
            || original.reviewed.request.operation_id != id
            || current.reviewed.request.operation_id != id
            || original.deployment_id != self.deployment
            || current.deployment_id != self.deployment
            || current.reviewed != original.reviewed
            || current.reservation != original.reservation
            || current.reserved != original.reserved
            || (revision != admitted
                && (current.outcome == TopologyOutcomeV1::Reserved
                    || current.predecessor_digest
                        != self.digest(b"iroha.sorafs.topology.operation.v1\0", &original)?))
        {
            return Err(TopologyRowErrorV1::CorruptHistory);
        }
        let reserved = current.outcome == TopologyOutcomeV1::Reserved;
        if reserved != (root.active == Some(id))
            || (reserved && revision != root.operation_head.revision)
        {
            return Err(TopologyRowErrorV1::CorruptHistory);
        }
        if revision == root.operation_head.revision
            && self.digest(b"iroha.sorafs.topology.operation.v1\0", &current)?
                != root.operation_head.digest
        {
            return Err(TopologyRowErrorV1::CorruptHistory);
        }
        Ok(Some(current))
    }

    fn operation_row(
        &self,
        revision: u64,
    ) -> Result<TopologyOperationRecordV1, TopologyRowErrorV1> {
        let bytes = self
            .world
            .smart_contract_state()
            .get(&self.path(&format!("operation_revision_{revision:020}")))
            .ok_or(TopologyRowErrorV1::CorruptHistory)?;
        let row: TopologyOperationRecordV1 = self.decode(bytes, TOPOLOGY_RECORD_MAX_BYTES_V1)?;
        if row.revision != revision {
            return Err(TopologyRowErrorV1::CorruptHistory);
        }
        Ok(row)
    }
}

/// Decode the current topology control and bind its public custody state to one State view.
///
/// This local read does not authenticate history replay, execution, finality, enrollment use,
/// or a topology approval. It retains no authority token and leaves native admission closed.
///
/// # Errors
/// Rejects a malformed current row or control state, or a role, deployment, network, chain,
/// or enrollment-head mismatch in the same borrowed State view.
// TODO: Connect this reader to funded full replay and the native transaction owner before it
// may participate in role-16 admission or signed approval.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "role-16 control reader is staged before native admission"
    )
)]
pub(crate) fn current_control_state_v1(
    snapshot: &impl StateReadOnly,
    deployment: &str,
) -> Result<Option<(TopologyControlRecordV1, SignerCustodyControlStateV1)>, TopologyRowErrorV1> {
    let rows = TopologyRowsV1::new(snapshot.world(), deployment)?;
    let Some(record) = rows.current_control()? else {
        return Ok(None);
    };
    let control: SignerCustodyControlStateV1 =
        rows.decode(&record.control_state, SIGNER_CUSTODY_CONTROL_MAX_BYTES_V1)?;
    control
        .validate()
        .map_err(|_| TopologyRowErrorV1::CorruptHistory)?;
    let binding = &control.policy.binding;
    if binding.role != SignerRoleV1::TopologyApproval
        || !matches!(
            &binding.purpose,
            SignerPurposeBindingV1::TopologyApproval { deployment_id }
                if deployment_id.as_str() == deployment
        )
        || binding.network_id != *snapshot.network_id().as_bytes()
        || binding.chain_id.as_str() != snapshot.chain_id().as_ref()
        || record.enrollment.is_some() != control.active_head.is_some()
    {
        return Err(TopologyRowErrorV1::CorruptHistory);
    }
    Ok(Some((record, control)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_data_model::sorafs::topology_authority::{
        TopologyActionV1, TopologyContextClaimV1, TopologyExecutionClaimV1, TopologyExpireV1,
        TopologyReserveV1, TopologyTransitionV1,
    };
    use iroha_data_model::{NetworkId, block::BlockHeader};
    use sorafs_manifest::signer::{
        custody::SignerCustodyAuthorityV1,
        custody_control::SignerCustodyPolicyV1,
        protocol::{
            SignerKeyAlgorithmV1, SignerOperationActionV1, SignerOperationAuditHeadV1,
            SignerOperationCustodyV1, SignerOperationIntentV1, SignerOperationReservationV1,
            SignerPurposeBindingV1, SignerRoleV1,
        },
        topology::{SignerTopologyRequestV1, subject::TopologyApprovalSubjectV1},
    };

    fn execution() -> TopologyExecutionClaimV1 {
        TopologyExecutionClaimV1 {
            height: 1,
            ordinal: 0,
            recorded_at_unix_ms: 1,
            authority: iroha_data_model::account::AccountId::new(
                iroha_crypto::KeyPair::try_from_seed(vec![1; 32], iroha_crypto::Algorithm::Ed25519)
                    .unwrap()
                    .public_key()
                    .clone(),
            ),
        }
    }

    fn execution_at(height: u64) -> TopologyExecutionClaimV1 {
        let mut value = execution();
        value.height = height;
        value.recorded_at_unix_ms = 1_000 * height;
        value
    }

    fn reviewed(id: [u8; 32]) -> TopologyReserveV1 {
        let request = SignerTopologyRequestV1 {
            operation_id: id,
            binding_digest: [2; 32],
            original_custody: SignerOperationCustodyV1 {
                record_digest: [3; 32],
                control_state_digest: [4; 32],
            },
            subject_digest: [5; 32],
        };
        TopologyReserveV1 {
            subject: TopologyApprovalSubjectV1 {
                deployment_id: "production-primary".into(),
                network_id: [6; 32],
                chain_id: "topology-chain".into(),
                chain_discriminant: 369,
                release_manifest_sha256: [7; 32],
                qualification_summary_sha256: [8; 32],
                manifest_sha256: [9; 32],
                canonical_manifest_sha256: [10; 32],
                validator_ids_sha256: [11; 32],
                reviewed_at_unix_ms: 1_000,
                expires_at_unix_ms: 10_000,
            },
            request,
            intent: SignerOperationIntentV1 {
                action: SignerOperationActionV1::Sign,
                operation_id: id,
                request_digest: [12; 32],
                previous_audit: SignerOperationAuditHeadV1 {
                    sequence: 0,
                    digest: [0; 32],
                },
            },
        }
    }

    // These rows test local native encoding/index geometry only. The claimed control/custody
    // source is intentionally absent, so this fixture cannot authorize any topology operation.
    fn indexed_operation_world(terminal: bool) -> (World, TopologyOperationRecordV1) {
        let mut world = World::new();
        let id = [0xaa; 32];
        let reviewed = reviewed(id);
        let reservation = SignerOperationReservationV1 {
            reservation_id: [13; 32],
            fence: 1,
            expires_at_unix_ms: 2_500,
        };
        let reserved = execution_at(2);
        let original = TopologyOperationRecordV1 {
            deployment_id: "production-primary".into(),
            revision: 1,
            predecessor_digest: [0; 32],
            transition_digest: [14; 32],
            execution: reserved.clone(),
            reserved,
            reviewed: reviewed.clone(),
            reservation,
            outcome: TopologyOutcomeV1::Reserved,
        };
        let (
            original_digest,
            original_path,
            terminal_path,
            history_one_path,
            history_two_path,
            root_path,
            slot_path,
            admission_path,
        ) = {
            let view = world.view();
            let rows = TopologyRowsV1::new(&view, "production-primary").unwrap();
            (
                rows.digest(b"iroha.sorafs.topology.operation.v1\0", &original)
                    .unwrap(),
                rows.path("operation_revision_00000000000000000001"),
                rows.path("operation_revision_00000000000000000002"),
                rows.path("history_revision_00000000000000000001"),
                rows.path("history_revision_00000000000000000002"),
                rows.path("retained"),
                rows.path(&format!("operation_id_{}", hex::encode(id))),
                rows.path(&format!("operation_admission_{}", hex::encode(id))),
            )
        };
        let mut last = original.clone();
        if terminal {
            last.revision = 2;
            last.predecessor_digest = original_digest;
            last.transition_digest = [15; 32];
            last.execution = execution_at(3);
            last.outcome = TopologyOutcomeV1::Expired;
        }
        let (operation_digest, history_one, history_two, history_digest) = {
            let view = world.view();
            let rows = TopologyRowsV1::new(&view, "production-primary").unwrap();
            let operation_digest = rows
                .digest(b"iroha.sorafs.topology.operation.v1\0", &last)
                .unwrap();
            let first = TopologyHistoryEntryV1 {
                revision: 1,
                predecessor_digest: [0; 32],
                transition: TopologyTransitionV1 {
                    deployment_id: "production-primary".into(),
                    control: TopologyHeadV1::EMPTY,
                    operations: TopologyHeadV1::EMPTY,
                    action: TopologyActionV1::Reserve(Box::new(reviewed.clone())),
                },
                context: TopologyContextClaimV1 {
                    execution: execution_at(2),
                    custody_anchor: None,
                    floor: None,
                },
                control: TopologyHeadV1::EMPTY,
                operations: TopologyHeadV1 {
                    revision: 1,
                    digest: original_digest,
                },
            };
            let first_digest = rows
                .digest(b"iroha.sorafs.topology.history.v1\0", &first)
                .unwrap();
            let second = terminal.then(|| TopologyHistoryEntryV1 {
                revision: 2,
                predecessor_digest: first_digest,
                transition: TopologyTransitionV1 {
                    deployment_id: "production-primary".into(),
                    control: TopologyHeadV1::EMPTY,
                    operations: first.operations,
                    action: TopologyActionV1::Expire(TopologyExpireV1 {
                        operation_id: id,
                        reservation,
                    }),
                },
                context: TopologyContextClaimV1 {
                    execution: execution_at(3),
                    custody_anchor: None,
                    floor: None,
                },
                control: TopologyHeadV1::EMPTY,
                operations: TopologyHeadV1 {
                    revision: 2,
                    digest: operation_digest,
                },
            });
            let history_digest = second.as_ref().map_or(first_digest, |value| {
                rows.digest(b"iroha.sorafs.topology.history.v1\0", value)
                    .unwrap()
            });
            (operation_digest, first, second, history_digest)
        };
        let mut root = TopologyRetainedStateV1::empty();
        root.operation_head = TopologyHeadV1 {
            revision: last.revision,
            digest: operation_digest,
        };
        root.history_head = TopologyHeadV1 {
            revision: if terminal { 2 } else { 1 },
            digest: history_digest,
        };
        root.fence = 1;
        root.operation_count = 1;
        root.active = (!terminal).then_some(id);
        root.last_execution = Some(execution_at(if terminal { 3 } else { 2 }));
        world
            .smart_contract_state
            .insert(original_path, norito::encode_canonical(&original).unwrap());
        if terminal {
            world
                .smart_contract_state
                .insert(terminal_path, norito::encode_canonical(&last).unwrap());
        }
        world.smart_contract_state.insert(
            history_one_path,
            norito::encode_canonical(&history_one).unwrap(),
        );
        if let Some(second) = history_two {
            world
                .smart_contract_state
                .insert(history_two_path, norito::encode_canonical(&second).unwrap());
        }
        world
            .smart_contract_state
            .insert(root_path, norito::encode_canonical(&root).unwrap());
        world
            .smart_contract_state
            .insert(slot_path, norito::encode_canonical(&last.revision).unwrap());
        world
            .smart_contract_state
            .insert(admission_path, norito::encode_canonical(&1_u64).unwrap());
        (world, last)
    }

    fn locally_consistent_history(
        control_head: TopologyHeadV1,
        operation_head: TopologyHeadV1,
    ) -> World {
        let mut world = World::new();
        let execution = execution();
        let history = TopologyHistoryEntryV1 {
            revision: 1,
            predecessor_digest: [0; 32],
            transition: TopologyTransitionV1 {
                deployment_id: "production-primary".into(),
                control: TopologyHeadV1::EMPTY,
                operations: TopologyHeadV1::EMPTY,
                action: TopologyActionV1::Configure(vec![1]),
            },
            context: TopologyContextClaimV1 {
                execution: execution.clone(),
                custody_anchor: None,
                floor: None,
            },
            control: control_head,
            operations: operation_head,
        };
        let (root_key, history_key, history_digest) = {
            let view = world.view();
            let rows = TopologyRowsV1::new(&view, "production-primary").unwrap();
            (
                rows.path("retained"),
                rows.path("history_revision_00000000000000000001"),
                rows.digest(b"iroha.sorafs.topology.history.v1\0", &history)
                    .unwrap(),
            )
        };
        let mut root = TopologyRetainedStateV1::empty();
        root.control_head = control_head;
        root.operation_head = operation_head;
        root.history_head = TopologyHeadV1 {
            revision: 1,
            digest: history_digest,
        };
        root.last_execution = Some(execution);
        world
            .smart_contract_state
            .insert(history_key, norito::encode_canonical(&history).unwrap());
        world
            .smart_contract_state
            .insert(root_key, norito::encode_canonical(&root).unwrap());
        world
    }

    #[test]
    fn namespace_is_exact_and_production_scoped() {
        let world = World::new();
        let view = world.view();
        let a = TopologyRowsV1::new(&view, "production-primary").unwrap();
        let b = TopologyRowsV1::new(&view, "production-secondary").unwrap();
        assert_ne!(a.path("retained"), b.path("retained"));
        assert_eq!(a.retained().unwrap(), TopologyRetainedStateV1::empty());
        assert_eq!(a.current_control().unwrap(), None);
        assert_eq!(a.operation([0xaa; 32]).unwrap(), None);
        assert_eq!(
            TopologyRowsV1::new(&view, "demo-primary").err(),
            Some(TopologyRowErrorV1::InvalidScope)
        );
    }

    #[test]
    fn orphan_index_cannot_be_mistaken_for_empty_prefix() {
        let mut world = World::new();
        let path = {
            let view = world.view();
            TopologyRowsV1::new(&view, "production-primary")
                .unwrap()
                .path("operation_id_aaaaaaaa")
        };
        world.smart_contract_state.insert(path, vec![1]);
        let view = world.view();
        let rows = TopologyRowsV1::new(&view, "production-primary").unwrap();
        assert_eq!(rows.retained(), Err(TopologyRowErrorV1::CorruptHistory));
    }

    #[test]
    fn malformed_and_empty_persisted_roots_fail_closed() {
        let mut world = World::new();
        let path = {
            let view = world.view();
            TopologyRowsV1::new(&view, "production-primary")
                .unwrap()
                .path("retained")
        };
        world.smart_contract_state.insert(path.clone(), vec![0xff]);
        let view = world.view();
        assert_eq!(
            TopologyRowsV1::new(&view, "production-primary")
                .unwrap()
                .retained(),
            Err(TopologyRowErrorV1::CorruptHistory)
        );
        drop(view);
        let mut noncanonical = norito::encode_canonical(&TopologyRetainedStateV1::empty()).unwrap();
        noncanonical.push(0);
        world
            .smart_contract_state
            .insert(path.clone(), noncanonical);
        let view = world.view();
        assert_eq!(
            TopologyRowsV1::new(&view, "production-primary")
                .unwrap()
                .retained(),
            Err(TopologyRowErrorV1::CorruptHistory)
        );
        drop(view);
        world.smart_contract_state.insert(
            path,
            norito::encode_canonical(&TopologyRetainedStateV1::empty()).unwrap(),
        );
        let view = world.view();
        assert_eq!(
            TopologyRowsV1::new(&view, "production-primary")
                .unwrap()
                .retained(),
            Err(TopologyRowErrorV1::CorruptHistory)
        );
    }

    #[test]
    fn missing_terminal_history_is_corruption() {
        let mut world = World::new();
        let path = {
            let view = world.view();
            TopologyRowsV1::new(&view, "production-primary")
                .unwrap()
                .path("retained")
        };
        let mut root = TopologyRetainedStateV1::empty();
        root.history_head = TopologyHeadV1 {
            revision: 1,
            digest: [3; 32],
        };
        root.last_execution = Some(execution());
        world
            .smart_contract_state
            .insert(path, norito::encode_canonical(&root).unwrap());
        let view = world.view();
        let rows = TopologyRowsV1::new(&view, "production-primary").unwrap();
        assert_eq!(rows.retained(), Err(TopologyRowErrorV1::CorruptHistory));
    }

    #[test]
    fn indexed_operation_requires_both_indices_and_a_bounded_original_row() {
        let mut world = locally_consistent_history(
            TopologyHeadV1::EMPTY,
            TopologyHeadV1 {
                revision: 1,
                digest: [4; 32],
            },
        );
        let id = [0xaa; 32];
        let (slot, admission, row) = {
            let view = world.view();
            let rows = TopologyRowsV1::new(&view, "production-primary").unwrap();
            (
                rows.path(&format!("operation_id_{}", hex::encode(id))),
                rows.path(&format!("operation_admission_{}", hex::encode(id))),
                rows.path("operation_revision_00000000000000000001"),
            )
        };
        world
            .smart_contract_state
            .insert(slot.clone(), norito::encode_canonical(&1_u64).unwrap());
        let view = world.view();
        assert_eq!(
            TopologyRowsV1::new(&view, "production-primary")
                .unwrap()
                .operation(id),
            Err(TopologyRowErrorV1::CorruptHistory)
        );
        drop(view);
        world
            .smart_contract_state
            .insert(admission, norito::encode_canonical(&1_u64).unwrap());
        world.smart_contract_state.insert(row, vec![0xff]);
        let view = world.view();
        assert_eq!(
            TopologyRowsV1::new(&view, "production-primary")
                .unwrap()
                .operation(id),
            Err(TopologyRowErrorV1::CorruptHistory)
        );
    }

    #[test]
    fn missing_current_control_row_is_corruption() {
        let world = locally_consistent_history(
            TopologyHeadV1 {
                revision: 1,
                digest: [5; 32],
            },
            TopologyHeadV1::EMPTY,
        );
        let view = world.view();
        let rows = TopologyRowsV1::new(&view, "production-primary").unwrap();
        assert!(rows.retained().is_ok());
        assert_eq!(
            rows.current_control(),
            Err(TopologyRowErrorV1::CorruptHistory)
        );
    }

    #[test]
    fn current_control_row_roundtrips_only_at_its_exact_head() {
        let record = TopologyControlRecordV1 {
            deployment_id: "production-primary".into(),
            revision: 1,
            predecessor_digest: [0; 32],
            request_digest: [6; 32],
            execution: execution(),
            control_state: vec![7, 8, 9],
            enrollment: None,
        };
        let digest = {
            let world = World::new();
            let view = world.view();
            TopologyRowsV1::new(&view, "production-primary")
                .unwrap()
                .digest(b"iroha.sorafs.topology.control.v1\0", &record)
                .unwrap()
        };
        let mut world = locally_consistent_history(
            TopologyHeadV1 {
                revision: 1,
                digest,
            },
            TopologyHeadV1::EMPTY,
        );
        let key = {
            let view = world.view();
            TopologyRowsV1::new(&view, "production-primary")
                .unwrap()
                .path("control_revision_00000000000000000001")
        };
        world
            .smart_contract_state
            .insert(key, norito::encode_canonical(&record).unwrap());
        let view = world.view();
        assert_eq!(
            TopologyRowsV1::new(&view, "production-primary")
                .unwrap()
                .current_control()
                .unwrap(),
            Some(record)
        );
    }

    fn control_state(network_id: [u8; 32]) -> SignerCustodyControlStateV1 {
        let public_key = |seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .unwrap()
                .public_key()
                .clone()
        };
        SignerCustodyControlStateV1 {
            policy: SignerCustodyPolicyV1 {
                binding: sorafs_manifest::signer::custody::SignerCustodyBindingV1 {
                    chain_id: "topology-chain".into(),
                    network_id,
                    runtime_handle: "software://sorafs/topology-approval/primary".into(),
                    key_handle: "software://sorafs/topology-approval/key-1".into(),
                    service_id: "topology-primary".into(),
                    administrator_id: "topology-security".into(),
                    role: SignerRoleV1::TopologyApproval,
                    purpose: SignerPurposeBindingV1::TopologyApproval {
                        deployment_id: "production-primary".into(),
                    },
                    algorithm: SignerKeyAlgorithmV1::Ed25519,
                    public_key: public_key(21),
                    key_revision: 1,
                    policy_revision: 1,
                    policy_digest: [2; 32],
                },
                attester_authority: SignerCustodyAuthorityV1 {
                    service_id: "custody-primary".into(),
                    administrator_id: "custody-security".into(),
                    key_revision: 1,
                    policy_revision: 1,
                    policy_digest: [3; 32],
                },
                attester_public_key: public_key(22),
                active_from_unix_ms: 90_000,
                active_until_unix_ms: 400_000,
                max_validity_ms: 300_000,
                max_anchor_age_ms: 10_000,
            },
            next_sequence: 1,
            predecessor_digest: [0; 32],
            active_head: None,
            signer_revoked: false,
            attester_revoked: false,
        }
    }

    fn control_world(control_frame: Vec<u8>, enrollment: Option<Vec<u8>>) -> World {
        let record = TopologyControlRecordV1 {
            deployment_id: "production-primary".into(),
            revision: 1,
            predecessor_digest: [0; 32],
            request_digest: [6; 32],
            execution: execution(),
            control_state: control_frame,
            enrollment,
        };
        let digest = {
            let world = World::new();
            let view = world.view();
            TopologyRowsV1::new(&view, "production-primary")
                .unwrap()
                .digest(b"iroha.sorafs.topology.control.v1\0", &record)
                .unwrap()
        };
        let mut world = locally_consistent_history(
            TopologyHeadV1 {
                revision: 1,
                digest,
            },
            TopologyHeadV1::EMPTY,
        );
        let path = {
            let view = world.view();
            TopologyRowsV1::new(&view, "production-primary")
                .unwrap()
                .path("control_revision_00000000000000000001")
        };
        world
            .smart_contract_state
            .insert(path, norito::encode_canonical(&record).unwrap());
        world
    }

    fn state_with_world(world: World) -> State {
        let network_id = NetworkId::from_genesis_hash(
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x42; Hash::LENGTH])),
        );
        State::new_with_chain_and_network_id_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
            "topology-chain".parse().unwrap(),
            network_id,
        )
    }

    #[test]
    fn current_control_state_uses_one_state_network_chain_and_canonical_frame() {
        let empty = state_with_world(World::new());
        assert!(
            current_control_state_v1(&empty.view(), "production-primary")
                .unwrap()
                .is_none()
        );
        let network_id = *empty.view().network_id().as_bytes();
        let control = control_state(network_id);
        assert!(control.validate().is_ok());
        let world = control_world(norito::encode_canonical(&control).unwrap(), None);
        let state = state_with_world(world);
        let (row, decoded) = current_control_state_v1(&state.view(), "production-primary")
            .unwrap()
            .expect("one current role-16 control");
        assert_eq!(row.revision, 1);
        assert_eq!(decoded, control);
        assert_eq!(
            decoded.policy.binding.network_id,
            *state.view().network_id().as_bytes()
        );
        assert_eq!(
            decoded.policy.binding.chain_id,
            state.view().chain_id().to_string()
        );
    }

    #[test]
    fn current_control_state_rejects_malformed_noncanonical_and_invalid_frames() {
        let empty = state_with_world(World::new());
        let mut control = control_state(*empty.view().network_id().as_bytes());
        let mut noncanonical = norito::encode_canonical(&control).unwrap();
        noncanonical.push(0);
        control.next_sequence = 0;
        for frame in [
            vec![0xff],
            vec![0; SIGNER_CUSTODY_CONTROL_MAX_BYTES_V1 + 1],
            noncanonical,
            norito::encode_canonical(&control).unwrap(),
        ] {
            let state = state_with_world(control_world(frame, None));
            assert!(matches!(
                current_control_state_v1(&state.view(), "production-primary"),
                Err(TopologyRowErrorV1::CorruptHistory)
            ));
        }
    }

    #[test]
    fn current_control_state_rejects_foreign_role_scope_network_chain_and_head() {
        let empty = state_with_world(World::new());
        let control = control_state(*empty.view().network_id().as_bytes());
        let mut wrong_role = control.clone();
        wrong_role.policy.binding.role = SignerRoleV1::FinalPromotionProvenance;
        wrong_role.policy.binding.purpose = SignerPurposeBindingV1::FinalPromotionProvenance {
            deployment_id: "production-primary".into(),
        };
        let mut wrong_deployment = control.clone();
        wrong_deployment.policy.binding.purpose = SignerPurposeBindingV1::TopologyApproval {
            deployment_id: "production-secondary".into(),
        };
        let mut wrong_network = control.clone();
        wrong_network.policy.binding.network_id = [0x99; 32];
        let mut wrong_chain = control.clone();
        wrong_chain.policy.binding.chain_id = "foreign-chain".into();
        for candidate in [wrong_role, wrong_deployment, wrong_network, wrong_chain] {
            assert!(candidate.validate().is_ok(), "policy grammar stays valid");
            let state = state_with_world(control_world(
                norito::encode_canonical(&candidate).unwrap(),
                None,
            ));
            assert!(matches!(
                current_control_state_v1(&state.view(), "production-primary"),
                Err(TopologyRowErrorV1::CorruptHistory)
            ));
        }
        let state = state_with_world(control_world(
            norito::encode_canonical(&control).unwrap(),
            Some(vec![1]),
        ));
        assert!(matches!(
            current_control_state_v1(&state.view(), "production-primary"),
            Err(TopologyRowErrorV1::CorruptHistory)
        ));
    }

    #[test]
    fn original_reserve_and_adjacent_terminal_rows_roundtrip_locally() {
        for terminal in [false, true] {
            let (world, expected) = indexed_operation_world(terminal);
            let view = world.view();
            let rows = TopologyRowsV1::new(&view, "production-primary").unwrap();
            assert!(rows.retained().is_ok());
            assert_eq!(
                rows.operation(expected.reviewed.request.operation_id)
                    .unwrap(),
                Some(expected)
            );
        }
    }

    #[test]
    fn rolled_back_terminal_slot_and_missing_active_slot_are_corruption() {
        let id = [0xaa; 32];
        let (mut terminal_world, _) = indexed_operation_world(true);
        let (slot, admission) = {
            let view = terminal_world.view();
            let rows = TopologyRowsV1::new(&view, "production-primary").unwrap();
            (
                rows.path(&format!("operation_id_{}", hex::encode(id))),
                rows.path(&format!("operation_admission_{}", hex::encode(id))),
            )
        };
        terminal_world
            .smart_contract_state
            .insert(slot.clone(), norito::encode_canonical(&1_u64).unwrap());
        let view = terminal_world.view();
        assert_eq!(
            TopologyRowsV1::new(&view, "production-primary")
                .unwrap()
                .operation(id),
            Err(TopologyRowErrorV1::CorruptHistory)
        );

        let (active_world, _) = indexed_operation_world(false);
        {
            let mut block = active_world.block();
            block.smart_contract_state.remove(slot);
            block.smart_contract_state.remove(admission);
            block.commit();
        }
        let view = active_world.view();
        assert_eq!(
            TopologyRowsV1::new(&view, "production-primary")
                .unwrap()
                .operation(id),
            Err(TopologyRowErrorV1::CorruptHistory)
        );
    }

    #[test]
    fn local_prefix_scan_rejects_missing_or_oversized_middle_history_row() {
        let (mut world, _) = indexed_operation_world(true);
        let (root_path, middle_path, third_path, middle_bytes, root, third) = {
            let view = world.view();
            let rows = TopologyRowsV1::new(&view, "production-primary").unwrap();
            assert_eq!(rows.validate_local_revision_prefixes(), Ok(()));
            let root_path = rows.path("retained");
            let middle_path = rows.path("history_revision_00000000000000000002");
            let third_path = rows.path("history_revision_00000000000000000003");
            let middle_bytes = view
                .smart_contract_state()
                .get(&middle_path)
                .unwrap()
                .clone();
            let mut root: TopologyRetainedStateV1 =
                norito::decode_canonical(view.smart_contract_state().get(&root_path).unwrap())
                    .unwrap();
            let mut third: TopologyHistoryEntryV1 =
                norito::decode_canonical(&middle_bytes).unwrap();
            third.revision = 3;
            third.predecessor_digest = root.history_head.digest;
            third.context.execution = execution_at(4);
            root.history_head = TopologyHeadV1 {
                revision: 3,
                digest: rows
                    .digest(b"iroha.sorafs.topology.history.v1\0", &third)
                    .unwrap(),
            };
            root.last_execution = Some(third.context.execution.clone());
            (
                root_path,
                middle_path,
                third_path,
                middle_bytes,
                root,
                third,
            )
        };
        world
            .smart_contract_state
            .insert(third_path, norito::encode_canonical(&third).unwrap());
        world
            .smart_contract_state
            .insert(root_path, norito::encode_canonical(&root).unwrap());
        {
            let view = world.view();
            let rows = TopologyRowsV1::new(&view, "production-primary").unwrap();
            assert_eq!(rows.validate_local_revision_prefixes(), Ok(()));
        }
        let extra_path = {
            let view = world.view();
            TopologyRowsV1::new(&view, "production-primary")
                .unwrap()
                .path("history_revision_00000000000000000004")
        };
        {
            let mut block = world.block();
            block
                .smart_contract_state
                .insert(extra_path.clone(), vec![1]);
            block.commit();
        }
        {
            let view = world.view();
            let rows = TopologyRowsV1::new(&view, "production-primary").unwrap();
            assert_eq!(
                rows.validate_local_revision_prefixes(),
                Err(TopologyRowErrorV1::CorruptHistory)
            );
        }
        {
            let mut block = world.block();
            block.smart_contract_state.remove(extra_path);
            block.commit();
        }
        {
            let mut block = world.block();
            block.smart_contract_state.insert(
                middle_path.clone(),
                vec![0xff; TOPOLOGY_HISTORY_MAX_BYTES_V1 + 1],
            );
            block.commit();
        }
        {
            let view = world.view();
            let rows = TopologyRowsV1::new(&view, "production-primary").unwrap();
            assert!(
                rows.retained().is_ok(),
                "the terminal indexed read misses this middle row"
            );
            assert_eq!(
                rows.validate_local_revision_prefixes(),
                Err(TopologyRowErrorV1::CorruptHistory)
            );
        }
        {
            let mut block = world.block();
            block
                .smart_contract_state
                .insert(middle_path.clone(), middle_bytes);
            block.commit();
        }
        {
            let mut block = world.block();
            block.smart_contract_state.remove(middle_path);
            block.commit();
        }
        let view = world.view();
        let rows = TopologyRowsV1::new(&view, "production-primary").unwrap();
        assert!(
            rows.retained().is_ok(),
            "the terminal indexed read misses this gap"
        );
        assert_eq!(
            rows.validate_local_revision_prefixes(),
            Err(TopologyRowErrorV1::CorruptHistory)
        );
    }

    #[test]
    fn terminal_history_cannot_be_transplanted_to_another_deployment() {
        let mut world = locally_consistent_history(TopologyHeadV1::EMPTY, TopologyHeadV1::EMPTY);
        let (root_bytes, history_bytes, other_root, other_history) = {
            let view = world.view();
            let primary = TopologyRowsV1::new(&view, "production-primary").unwrap();
            let secondary = TopologyRowsV1::new(&view, "production-secondary").unwrap();
            (
                view.smart_contract_state()
                    .get(&primary.path("retained"))
                    .unwrap()
                    .clone(),
                view.smart_contract_state()
                    .get(&primary.path("history_revision_00000000000000000001"))
                    .unwrap()
                    .clone(),
                secondary.path("retained"),
                secondary.path("history_revision_00000000000000000001"),
            )
        };
        world.smart_contract_state.insert(other_root, root_bytes);
        world
            .smart_contract_state
            .insert(other_history, history_bytes);
        let view = world.view();
        let primary = TopologyRowsV1::new(&view, "production-primary").unwrap();
        let secondary = TopologyRowsV1::new(&view, "production-secondary").unwrap();
        assert!(primary.retained().is_ok());
        assert_eq!(
            secondary.retained(),
            Err(TopologyRowErrorV1::CorruptHistory)
        );
    }

    #[test]
    fn retained_head_cannot_hide_newer_indexed_history() {
        for suffix in [
            "history_revision_00000000000000000002",
            "history_revision_18446744073709551616",
            "history_revision_z",
            "history_revision_+",
        ] {
            let mut world =
                locally_consistent_history(TopologyHeadV1::EMPTY, TopologyHeadV1::EMPTY);
            let extra = {
                let view = world.view();
                TopologyRowsV1::new(&view, "production-primary")
                    .unwrap()
                    .path(suffix)
            };
            world.smart_contract_state.insert(extra, vec![0xff]);
            let view = world.view();
            assert_eq!(
                TopologyRowsV1::new(&view, "production-primary")
                    .unwrap()
                    .retained(),
                Err(TopologyRowErrorV1::CorruptHistory),
                "{suffix}"
            );
        }
    }
}
