//! Test-only signed CAS over owned, simulated checkpoint storage.
//!
//! This volatile register models publication, retry and fresh selection. It is neither a native
//! WAL implementation nor an OEM durability/rollback guarantee. Core creates and verifies actual
//! bootstrap journals; the parent retains those locked stores throughout the diagnostic. Ordinary
//! register tests use separate simulated journal bytes that never supply Core journal prefixes.
//! Every actual Bootstrap and Guard proof remains verified before checkpoint staging.

use super::*;
#[cfg(unix)]
use crate::zk::kagemusha_v1_state::{
    KagemushaAuthenticatedHistoryStoreV1, KagemushaBootstrapCheckpointV1,
};
use crate::zk::kagemusha_v1_state::{
    KagemushaRecoveryCheckpointIdentityV1, KagemushaRecoveryCheckpointStatementV1,
    KagemushaRecoveryJournalPrefixV1, KagemushaRecoveryJournalsV1, KagemushaStateV1,
};

const CAS_DOMAIN: &[u8] = b"iroha:kagemusha:diagnostic:simulated-metadata-cas\0";
const CURRENT_DOMAIN: &[u8] = b"iroha:kagemusha:diagnostic:simulated-current-checkpoint\0";

#[derive(Clone)]
pub(super) struct DiagnosticCheckpointRegister {
    identity: DigestV1,
    initial_state: KagemushaStateV1,
    credential: KagemushaHardwareCredentialV1,
    enrollment: KagemushaRecoveryEnrollmentBindingV1,
    key: Rc<SigningKey>,
    journals: KagemushaRecoveryJournalsV1,
    register: Rc<RefCell<Register>>,
}

#[derive(Default)]
struct Register {
    current: Option<DurabilityAnchorStatementV1>,
    material: BTreeMap<DigestV1, Material>,
    terminals: BTreeMap<DigestV1, Terminal>,
    coordinator: Vec<u8>,
    responses: Vec<u8>,
    next_challenge: u64,
}

#[derive(Clone)]
struct Material {
    statement: KagemushaRecoveryCheckpointStatementV1,
    snapshot: Vec<u8>,
    snapshot_digest: DigestV1,
    journals: KagemushaRecoveryJournalsV1,
    source: JournalSource,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum JournalSource {
    Simulated,
    CoreBootstrap,
}

struct Terminal {
    material: Material,
    certificate: Vec<u8>,
}

fn prefix(bytes: &[u8], role: u8) -> KagemushaRecoveryJournalPrefixV1 {
    let mut preimage = vec![role];
    preimage.extend_from_slice(bytes);
    KagemushaRecoveryJournalPrefixV1 {
        sequence: 1,
        head: digest_bytes(b"diagnostic-simulated-journal-frame", &preimage),
        byte_len: bytes.len() as u64,
    }
}

fn checkpoint_identity(
    statement: &DurabilityAnchorStatementV1,
) -> KagemushaRecoveryCheckpointIdentityV1 {
    KagemushaRecoveryCheckpointIdentityV1 {
        revision: statement.metadata_revision,
        snapshot_commitment: statement.snapshot_commitment,
    }
}

impl DiagnosticCheckpointRegister {
    pub(super) fn new(
        material: &MintRecipientMaterial,
        initial_state: &KagemushaStateV1,
    ) -> Result<Self, String> {
        let credential = material.hardware_credential;
        credential
            .validate_against_profile(&material.hardware_profile)
            .map_err(|error| error.to_string())?;
        ensure(
            credential.network_id == initial_state.lane.network_id
                && credential.lane_commitment == initial_state.lane.device_lane_id
                && credential.hardware_epoch_id == initial_state.hardware_epoch.epoch_id
                && u128::from(credential.hardware_epoch_generation)
                    == initial_state.hardware_epoch.generation
                && credential.device_key_reference
                    == initial_state.device_policy_binding.device_key_reference
                && initial_state.device_policy_binding.hardware_policy_id
                    == material.provider_policy.root
                && credential.hardware_profile_id == initial_state.hardware_profile_id
                && credential.suite_id == initial_state.suite_id
                && credential.policy_epoch == initial_state.policy_epoch
                && initial_state.release_id
                    == material.authorization_relation.statement.context.release_id,
            "simulated checkpoint credential identity mismatch",
        )?;
        let enrollment = diagnostic_enrollment_binding(material, initial_state)?;
        let identity = digest_bytes(
            b"diagnostic-simulated-checkpoint-identity",
            &norito::encode_canonical(&(
                initial_state.context(),
                initial_state.lane.clone(),
                initial_state.hardware_epoch,
                initial_state.device_policy_binding,
                credential,
                enrollment.clone(),
            ))
            .map_err(|error| error.to_string())?,
        );
        // Both actual byte buffers contain one canonical initialization frame. This diagnostic
        // accepts no speculative suffix and does not impersonate the production WAL formats.
        let coordinator = norito::encode_canonical(&(1_u16, 0_u8, identity))
            .map_err(|error| error.to_string())?;
        let responses = norito::encode_canonical(&(1_u16, 1_u8, identity))
            .map_err(|error| error.to_string())?;
        let journals = KagemushaRecoveryJournalsV1 {
            coordinator: prefix(&coordinator, 0),
            responses: prefix(&responses, 1),
            // Explicit simulator roots, not production response-history SMT evidence.
            response_history_root: digest_bytes(b"diagnostic-simulated-response-root", &identity),
            retirement_transition_id: digest_bytes(
                b"diagnostic-simulated-retirement-init",
                &identity,
            ),
        };
        Ok(Self {
            identity,
            initial_state: initial_state.clone(),
            credential,
            enrollment,
            key: Rc::new(
                SigningKey::from_bytes((&material.provider_policy.provider_secret).into())
                    .map_err(|error| error.to_string())?,
            ),
            journals,
            register: Rc::new(RefCell::new(Register {
                coordinator,
                responses,
                ..Register::default()
            })),
        })
    }

    pub(super) fn journals(&self) -> KagemushaRecoveryJournalsV1 {
        self.journals
    }

    fn validate_identity(
        &self,
        statement: &KagemushaRecoveryCheckpointStatementV1,
    ) -> Result<(), String> {
        let successor = &statement.successor;
        ensure(
            statement.operation_id != [0; 32]
                && statement.previous.revision.checked_add(1) == Some(successor.metadata_revision)
                && (statement.previous.revision == 0)
                    == (statement.previous.snapshot_commitment == [0; 32])
                && successor.version == self.initial_state.version
                && successor.lane == self.initial_state.lane
                && successor.hardware_epoch == self.initial_state.hardware_epoch
                && successor.device_policy_binding == self.initial_state.device_policy_binding
                && successor.snapshot_commitment != [0; 32],
            "simulated checkpoint wallet or predecessor identity mismatch",
        )
    }

    fn check_material(
        &self,
        register: &Register,
        statement: &KagemushaRecoveryCheckpointStatementV1,
    ) -> Result<(), String> {
        let material = register
            .material
            .get(&statement.operation_id)
            .ok_or("simulated checkpoint bytes are unavailable")?;
        ensure(
            material.statement == *statement
                && !material.snapshot.is_empty()
                && digest_bytes(b"diagnostic-retained-snapshot", &material.snapshot)
                    == material.snapshot_digest,
            "simulated checkpoint persisted material conflict",
        )?;
        if material.source == JournalSource::Simulated {
            ensure(
                prefix(&register.coordinator, 0) == material.journals.coordinator
                    && prefix(&register.responses, 1) == material.journals.responses,
                "simulated checkpoint initialization journal bytes changed",
            )?;
        }
        if let Some(terminal) = register.terminals.get(&statement.operation_id) {
            ensure(
                terminal.material.statement == material.statement
                    && terminal.material.snapshot == material.snapshot
                    && terminal.material.snapshot_digest == material.snapshot_digest
                    && terminal.material.journals == material.journals
                    && terminal.material.source == material.source,
                "simulated checkpoint retained snapshot bytes changed",
            )?;
        }
        Ok(())
    }

    #[cfg(unix)]
    pub(super) fn retain_pending<R, G, H>(
        &self,
        pending: &KagemushaBootstrapCheckpointV1<R, G, H>,
    ) -> Result<(), String>
    where
        R: KagemushaRecursiveVerifierV1,
        G: KagemushaGuardBundleVerifierV1,
        H: KagemushaAuthenticatedHistoryStoreV1,
    {
        let snapshot = pending.snapshot();
        let statement = pending.statement();
        let expected = DurabilityAnchorStatementV1 {
            metadata_revision: snapshot.recovery_metadata.revision,
            version: snapshot.version,
            lane: snapshot.state.lane.clone(),
            state_commitment: snapshot.state.state_commitment,
            hardware_epoch: snapshot.state.hardware_epoch,
            device_policy_binding: snapshot.state.device_policy_binding,
            state_nonce_commitment: snapshot.state.state_nonce_commitment,
            logical_sequence: snapshot.state.logical_sequence,
            journal_revision: snapshot.journal_revision,
            inbox_revision: snapshot.inbox_revision,
            snapshot_commitment: snapshot.snapshot_commitment,
        };
        ensure(
            expected == statement.successor
                && snapshot.state == self.initial_state
                && snapshot.journal_revision == 0
                && snapshot.inbox_revision == 0
                && snapshot.recovery_metadata.revision == 1
                && snapshot.recovery_metadata.checkpoint_operation_id == statement.operation_id
                && snapshot.recovery_metadata.previous_checkpoint == statement.previous
                && snapshot.recovery_metadata.journals.coordinator.sequence == 1
                && snapshot.recovery_metadata.journals.responses.sequence == 1
                && snapshot.recovery_metadata.journals.retirement_transition_id
                    == statement.operation_id
                && snapshot.recovery_metadata.accepted_credential.credential == self.credential
                && snapshot.recovery_metadata.accepted_credential.release_id
                    == self.initial_state.release_id
                && snapshot.recovery_metadata.enrollment == self.enrollment,
            "simulated checkpoint does not bind the exact pending Core snapshot",
        )?;
        // Only the actual Core bootstrap stage supplies these prefixes. Its finish method
        // rechecks the held descriptor-owned journals; this simulated provider does not claim
        // that its in-memory test buffers are those files or authenticate later recovery.
        self.retain_material(
            statement,
            norito::encode_canonical(snapshot).map_err(|error| error.to_string())?,
            snapshot.recovery_metadata.journals,
            JournalSource::CoreBootstrap,
        )
    }

    // Separate storage/protocol primitive lets ordinary tests attack the register without a
    // fabricated proof or usable Core machine. Only retain_pending is exposed to the caller.
    fn retain_bytes(
        &self,
        statement: &KagemushaRecoveryCheckpointStatementV1,
        snapshot: Vec<u8>,
    ) -> Result<(), String> {
        self.retain_material(statement, snapshot, self.journals, JournalSource::Simulated)
    }

    fn retain_material(
        &self,
        statement: &KagemushaRecoveryCheckpointStatementV1,
        snapshot: Vec<u8>,
        journals: KagemushaRecoveryJournalsV1,
        source: JournalSource,
    ) -> Result<(), String> {
        self.validate_identity(statement)?;
        ensure(!snapshot.is_empty(), "empty simulated checkpoint snapshot")?;
        let mut register = self.register.borrow_mut();
        if let Some(existing) = register.material.get(&statement.operation_id) {
            return ensure(
                existing.statement == *statement
                    && existing.snapshot == snapshot
                    && existing.journals == journals
                    && existing.source == source,
                "conflicting simulated checkpoint persistence retry",
            );
        }
        register.material.insert(
            statement.operation_id,
            Material {
                statement: statement.clone(),
                snapshot_digest: digest_bytes(b"diagnostic-retained-snapshot", &snapshot),
                snapshot,
                journals,
                source,
            },
        );
        Ok(())
    }

    pub(super) fn commit(
        &self,
        statement: &KagemushaRecoveryCheckpointStatementV1,
    ) -> Result<Vec<u8>, String> {
        self.validate_identity(statement)?;
        let mut register = self.register.borrow_mut();
        self.check_material(&register, statement)?;
        if let Some(terminal) = register.terminals.get(&statement.operation_id) {
            ensure(
                terminal.material.statement == *statement,
                "conflicting simulated CAS retry",
            )?;
            return Ok(terminal.certificate.clone());
        }
        let current = register.current.as_ref().map_or(
            KagemushaRecoveryCheckpointIdentityV1 {
                revision: 0,
                snapshot_commitment: [0; 32],
            },
            checkpoint_identity,
        );
        ensure(
            current == statement.previous,
            "stale simulated CAS predecessor",
        )?;
        let certificate = sign_journal(&self.key, CAS_DOMAIN, &(self.identity, statement.clone()));
        let material = register
            .material
            .get(&statement.operation_id)
            .ok_or("simulated checkpoint bytes disappeared")?
            .clone();
        register.terminals.insert(
            statement.operation_id,
            Terminal {
                material,
                certificate: certificate.clone(),
            },
        );
        register.current = Some(statement.successor.clone());
        Ok(certificate)
    }

    pub(super) fn verify_cas(
        &self,
        statement: &KagemushaRecoveryCheckpointStatementV1,
        bytes: &[u8],
    ) -> Result<(), String> {
        self.validate_identity(statement)?;
        let register = self.register.borrow();
        let terminal = register
            .terminals
            .get(&statement.operation_id)
            .ok_or("simulated CAS never executed")?;
        ensure(
            terminal.material.statement == *statement && terminal.certificate == bytes,
            "simulated CAS original certificate mismatch",
        )?;
        self.check_material(&register, statement)?;
        verify_journal(
            &device_public_key(&self.key),
            CAS_DOMAIN,
            &(self.identity, statement.clone()),
            bytes,
        )
    }

    pub(super) fn verify_anchor(
        &self,
        statement: &DurabilityAnchorStatementV1,
        bytes: &[u8],
    ) -> Result<(), String> {
        let cas = self
            .register
            .borrow()
            .terminals
            .values()
            .find(|terminal| terminal.material.statement.successor == *statement)
            // Actual restoration lacks a diagnostic owner of native journal descriptors.
            // Bootstrap's finish checks must not become recovery authority.
            .filter(|terminal| terminal.material.source == JournalSource::Simulated)
            .map(|terminal| terminal.material.statement.clone())
            .ok_or("unissued simulated checkpoint anchor")?;
        self.verify_cas(&cas, bytes)
    }

    fn current_response(
        &self,
        register: &Register,
        challenge: u64,
    ) -> Result<(DurabilityAnchorStatementV1, Vec<u8>), String> {
        let current = register
            .current
            .clone()
            .ok_or("no simulated current checkpoint")?;
        let journals = register
            .terminals
            .values()
            .find(|terminal| terminal.material.statement.successor == current)
            .ok_or("no retained simulated current checkpoint")?
            .material
            .journals;
        let certificate = sign_journal(
            &self.key,
            CURRENT_DOMAIN,
            &(self.identity, challenge, current.clone(), journals),
        );
        Ok((current, certificate))
    }

    fn verify_current_response(
        &self,
        challenge: u64,
        statement: &DurabilityAnchorStatementV1,
        journals: &KagemushaRecoveryJournalsV1,
        response: &(DurabilityAnchorStatementV1, Vec<u8>),
    ) -> Result<(), String> {
        ensure(
            challenge != 0 && response.0 == *statement,
            "simulated current selection statement mismatch",
        )?;
        verify_journal(
            &device_public_key(&self.key),
            CURRENT_DOMAIN,
            &(self.identity, challenge, response.0.clone(), *journals),
            &response.1,
        )
    }

    pub(super) fn verify_current(
        &self,
        statement: &DurabilityAnchorStatementV1,
        journals: &KagemushaRecoveryJournalsV1,
    ) -> Result<(), String> {
        let mut register = self.register.borrow_mut();
        // The verifier owns the challenge and the device exchange. There is no caller freshness
        // flag; the monotonically increasing simulator counter is shared by every verifier clone.
        let challenge = register
            .next_challenge
            .checked_add(1)
            .ok_or("simulated current selection challenge exhausted")?;
        register.next_challenge = challenge;
        let response = self.current_response(&register, challenge)?;
        self.verify_current_response(challenge, statement, journals, &response)?;
        let cas = register
            .terminals
            .values()
            .find(|terminal| terminal.material.statement.successor == *statement)
            .ok_or("simulated current selection has no retained CAS")?;
        self.check_material(&register, &cas.material.statement)
    }
}

#[cfg(test)]
#[path = "recovery_checkpoint/tests.rs"]
mod tests;
