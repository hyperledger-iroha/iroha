//! Concrete recovered account/device session over the retained authenticated production Core.
//! Open selects this original owner; only dual-signature completion admits observation dispatch.

use super::{
    KagemushaCoreCoordinatorBackendErrorV1 as Error, KagemushaCoreCoordinatorBackendV1,
    KagemushaCoreCoordinatorMethodV1 as Method,
    enrolled_session::{
        AuthenticatedRecoveredOwnerV1, BegunRecoveredSessionV1, RecoveredEnrolledSessionRegistryV1,
    },
    kagemusha_core_coordinator_decode_request_v1, kagemusha_core_coordinator_encode_response_v1,
    kagemusha_core_coordinator_validate_method_request_v1,
    kagemusha_core_coordinator_validate_method_response_v1,
    kagemusha_core_coordinator_validate_storage_path_v1,
    session_registry::RegistryError,
};
use iroha_core_zk::kagemusha_v1_state::KagemushaAuthenticatedCoreOwnerV1;
use iroha_data_model::kagemusha::KagemushaDevicePublicKeyV1;
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

type Result<T> = std::result::Result<T, Error>;
enum Phase {
    Unbegun,
    Preparing,
    Pending(BegunRecoveredSessionV1),
    Completing,
    Complete {
        attempt: u64,
        lease: u64,
        original_request: Vec<u8>,
        acknowledgment: Vec<u8>,
        deadline: super::native_deadline::NativeDeadlineV1,
    },
    Frozen,
}
struct Open {
    active: Arc<AtomicBool>,
    phase: Phase,
}
struct Opens {
    next: u64,
    values: BTreeMap<u64, Open>,
}

/// Concrete possession-only recovery adapter retaining the original production Core owner.
/// Applications cannot construct it from a checkpoint, wallet projection or arbitrary verifier.
pub struct KagemushaAuthenticatedRecoveredCoordinatorV1 {
    path: String,
    owner: Arc<Mutex<AuthenticatedRecoveredOwnerV1>>,
    registry: RecoveredEnrolledSessionRegistryV1<AuthenticatedRecoveredOwnerV1>,
    opens: Mutex<Opens>,
}

impl KagemushaAuthenticatedRecoveredCoordinatorV1 {
    /// Consume the independently selected, descriptor-held production owner and native key.
    /// The path is the native provisioner's original selection, not a mobile authority input.
    /// # Errors
    /// Refuses unavailable hardware, changed journals, invalid paths or unadmitted production Core.
    ///
    /// Decoded durable projections cannot create a production owner:
    /// ```compile_fail
    /// use connect_norito_bridge::KagemushaAuthenticatedRecoveredCoordinatorV1;
    /// use iroha_core_zk::kagemusha_v1_state::KagemushaStateSnapshotV1;
    /// use iroha_data_model::kagemusha::KagemushaDevicePublicKeyV1;
    /// fn cannot_open_projection(snapshot: KagemushaStateSnapshotV1, key: KagemushaDevicePublicKeyV1,
    ///     signer: std::sync::Arc<dyn connect_norito_bridge::KagemushaNativeCoreAuthorizationSignerV1>) {
    ///     let _ = KagemushaAuthenticatedRecoveredCoordinatorV1::from_native_owner(
    ///         "/private/wallet".to_owned(), snapshot, key, signer);
    /// }
    /// ```
    pub fn from_native_owner(
        path: String,
        core: KagemushaAuthenticatedCoreOwnerV1,
        native_key: KagemushaDevicePublicKeyV1,
        signer: Arc<dyn super::KagemushaNativeCoreAuthorizationSignerV1>,
    ) -> Result<Self> {
        kagemusha_core_coordinator_validate_storage_path_v1(path.as_bytes())
            .map_err(|_| Error::Rejected)?;
        let owner = AuthenticatedRecoveredOwnerV1::new(path.clone(), core, native_key, signer)
            .map_err(|_| Error::Rejected)?;
        Ok(Self {
            path,
            owner: Arc::new(Mutex::new(owner)),
            registry: RecoveredEnrolledSessionRegistryV1::new(),
            opens: Mutex::new(Opens {
                next: 1,
                values: BTreeMap::new(),
            }),
        })
    }

    /// Recover the actual original prepared commit after restart, preserving its already
    /// signed command. Decoded candidates or app journals cannot enter this constructor.
    pub fn from_native_pending_commit(
        path: String,
        commit: iroha_core_zk::kagemusha_v1_state::KagemushaAuthenticatedOutgoingCommitV1,
        native_key: KagemushaDevicePublicKeyV1,
        signer: Arc<dyn super::KagemushaNativeCoreAuthorizationSignerV1>,
    ) -> Result<Self> {
        kagemusha_core_coordinator_validate_storage_path_v1(path.as_bytes())
            .map_err(|_| Error::Rejected)?;
        let owner = AuthenticatedRecoveredOwnerV1::from_pending_commit(
            path.clone(),
            commit,
            native_key,
            signer,
        )
        .map_err(|_| Error::Rejected)?;
        Ok(Self {
            path,
            owner: Arc::new(Mutex::new(owner)),
            registry: RecoveredEnrolledSessionRegistryV1::new(),
            opens: Mutex::new(Opens {
                next: 1,
                values: BTreeMap::new(),
            }),
        })
    }

    /// Recover only an actual original prepared native fold. Public app/proof archives cannot
    /// enter this constructor or select another history transaction, nonce, clock or owner.
    pub fn from_native_pending_incoming(
        path: String,
        cap: iroha_core_zk::kagemusha_v1_state::KagemushaAuthenticatedIncomingFoldV1,
        native_key: KagemushaDevicePublicKeyV1,
        signer: Arc<dyn super::KagemushaNativeCoreAuthorizationSignerV1>,
    ) -> Result<Self> {
        kagemusha_core_coordinator_validate_storage_path_v1(path.as_bytes())
            .map_err(|_| Error::Rejected)?;
        let owner = AuthenticatedRecoveredOwnerV1::from_pending_incoming(
            path.clone(),
            cap,
            native_key,
            signer,
        )
        .map_err(|_| Error::Rejected)?;
        Ok(Self {
            path,
            owner: Arc::new(Mutex::new(owner)),
            registry: RecoveredEnrolledSessionRegistryV1::new(),
            opens: Mutex::new(Opens {
                next: 1,
                values: BTreeMap::new(),
            }),
        })
    }
    fn active(active: &AtomicBool) -> std::result::Result<(), RegistryError> {
        if active.load(Ordering::Acquire) {
            Ok(())
        } else {
            Err(RegistryError::Rejected)
        }
    }
    fn response(fields: &[Vec<u8>]) -> Result<Vec<u8>> {
        kagemusha_core_coordinator_encode_response_v1(fields).map_err(|_| Error::Rejected)
    }
    fn challenge(pending: &BegunRecoveredSessionV1) -> Result<Vec<u8>> {
        Self::response(&[
            pending.attempt_id.to_le_bytes().to_vec(),
            pending.canonical_challenge.clone(),
            pending.account_signing_message.to_vec(),
            pending.device_command.clone(),
            pending.device_request_id.to_vec(),
        ])
    }

    fn recovered_phase(&self, handle: u64, frame: &[u8]) -> Result<Vec<u8>> {
        let fields =
            kagemusha_core_coordinator_decode_request_v1(frame).map_err(|_| Error::Rejected)?;
        let phase = u32::from_le_bytes(
            fields[0]
                .as_slice()
                .try_into()
                .map_err(|_| Error::Rejected)?,
        );
        match phase {
            9 => {
                let (active, saved) = {
                    let mut opens = self.opens.lock().map_err(|_| Error::Rejected)?;
                    let open = opens.values.get_mut(&handle).ok_or(Error::Rejected)?;
                    Self::active(&open.active).map_err(|_| Error::Rejected)?;
                    match &open.phase {
                        Phase::Pending(pending) => (open.active.clone(), Some(pending.clone())),
                        Phase::Unbegun => {
                            open.phase = Phase::Preparing;
                            (open.active.clone(), None)
                        }
                        _ => return Err(Error::Rejected),
                    }
                };
                if let Some(saved) = saved {
                    self.owner
                        .lock()
                        .map_err(|_| Error::Rejected)?
                        .require_pending_original(&saved)
                        .map_err(|_| Error::Rejected)?;
                    Self::active(&active).map_err(|_| Error::Rejected)?;
                    return Self::challenge(&saved);
                }
                let pending = self
                    .registry
                    .begin_checked(self.owner.clone(), || Self::active(&active))
                    .map_err(|_| Error::Rejected)?;
                let response = Self::challenge(&pending)?;
                let mut opens = self.opens.lock().map_err(|_| Error::Rejected)?;
                let open = opens.values.get_mut(&handle).ok_or(Error::Rejected)?;
                if !open.active.load(Ordering::Acquire) || !matches!(open.phase, Phase::Preparing) {
                    drop(opens);
                    let _ = self.registry.cancel(pending.attempt_id);
                    return Err(Error::Rejected);
                }
                open.phase = Phase::Pending(pending);
                Ok(response)
            }
            10 => {
                let attempt = u64::from_le_bytes(
                    fields[1]
                        .as_slice()
                        .try_into()
                        .map_err(|_| Error::Rejected)?,
                );
                let (active, pending, replay) = {
                    let mut opens = self.opens.lock().map_err(|_| Error::Rejected)?;
                    let open = opens.values.get_mut(&handle).ok_or(Error::Rejected)?;
                    Self::active(&open.active).map_err(|_| Error::Rejected)?;
                    match &open.phase {
                        Phase::Complete {
                            attempt: original,
                            lease,
                            original_request,
                            acknowledgment,
                            deadline,
                        } if *original == attempt && original_request == frame => {
                            deadline.check().map_err(|_| Error::Rejected)?;
                            (
                                open.active.clone(),
                                None,
                                Some((*lease, acknowledgment.clone())),
                            )
                        }
                        Phase::Pending(pending) if pending.attempt_id == attempt => {
                            let pending = pending.clone();
                            open.phase = Phase::Completing;
                            (open.active.clone(), Some(pending), None)
                        }
                        _ => return Err(Error::Rejected),
                    }
                };
                if let Some((lease, response)) = replay {
                    let checked = self
                        .registry
                        .dispatch_observation(lease, |_| ())
                        .map_err(|_| Error::Rejected)?;
                    if !checked.session_is_current {
                        return Err(Error::Rejected);
                    }
                    Self::active(&active).map_err(|_| Error::Rejected)?;
                    return Ok(response);
                }
                let pending = pending.ok_or(Error::Rejected)?;
                let lease = self
                    .registry
                    .complete_checked(attempt, &fields[2], &fields[3], || Self::active(&active))
                    .map_err(|_| Error::Rejected)?;
                let response = Self::response(&[attempt.to_le_bytes().to_vec()])?;
                let mut opens = self.opens.lock().map_err(|_| Error::Rejected)?;
                let open = opens.values.get_mut(&handle).ok_or(Error::Rejected)?;
                if !open.active.load(Ordering::Acquire) || !matches!(open.phase, Phase::Completing)
                {
                    drop(opens);
                    let _ = self.registry.close(lease);
                    return Err(Error::Rejected);
                }
                open.phase = Phase::Complete {
                    attempt,
                    lease,
                    original_request: frame.to_vec(),
                    acknowledgment: response.clone(),
                    deadline: pending.deadline,
                };
                Ok(response)
            }
            11 => {
                let attempt = u64::from_le_bytes(
                    fields[1]
                        .as_slice()
                        .try_into()
                        .map_err(|_| Error::Rejected)?,
                );
                let mut opens = self.opens.lock().map_err(|_| Error::Rejected)?;
                let open = opens.values.get_mut(&handle).ok_or(Error::Rejected)?;
                if !matches!(&open.phase, Phase::Pending(pending) if pending.attempt_id == attempt)
                {
                    return Err(Error::Rejected);
                }
                open.active.store(false, Ordering::Release);
                open.phase = Phase::Frozen;
                drop(opens);
                self.registry.cancel(attempt).map_err(|_| Error::Rejected)?;
                Self::response(&[])
            }
            _ => Err(Error::Rejected),
        }
    }

    fn observe(&self, handle: u64, method: Method, frame: &[u8]) -> Result<Vec<u8>> {
        let (active, lease) = {
            let opens = self.opens.lock().map_err(|_| Error::Rejected)?;
            let open = opens.values.get(&handle).ok_or(Error::Rejected)?;
            Self::active(&open.active).map_err(|_| Error::Rejected)?;
            let Phase::Complete { lease, .. } = open.phase else {
                return Err(Error::Rejected);
            };
            (open.active.clone(), lease)
        };
        let fields =
            kagemusha_core_coordinator_decode_request_v1(frame).map_err(|_| Error::Rejected)?;
        let completed = self
            .registry
            .dispatch_observation(lease, |observer| {
                Self::active(&active).map_err(|_| Error::Rejected)?;
                match method {
                    Method::BeginObservation => {
                        let operation = u32::from_le_bytes(
                            fields[0]
                                .as_slice()
                                .try_into()
                                .map_err(|_| Error::Rejected)?,
                        );
                        let operation = u8::try_from(operation).map_err(|_| Error::Rejected)?;
                        let nonce = observer
                            .begin(operation, &fields[1])
                            .map_err(|_| Error::Rejected)?;
                        Self::response(&[nonce.to_vec()])
                    }
                    Method::AcceptQualification => {
                        observer
                            .stage_qualification(&fields)
                            .map_err(|_| Error::Rejected)?;
                        Self::response(&[])
                    }
                    Method::AcceptAuthenticatedReply => {
                        let operation = u8::try_from(u32::from_le_bytes(
                            fields[0]
                                .as_slice()
                                .try_into()
                                .map_err(|_| Error::Rejected)?,
                        ))
                        .map_err(|_| Error::Rejected)?;
                        let nonce = fields[1]
                            .as_slice()
                            .try_into()
                            .map_err(|_| Error::Rejected)?;
                        observer
                            .accept(
                                operation,
                                nonce,
                                &fields[2],
                                &fields[3],
                                &fields[4],
                                &fields[5..],
                            )
                            .map_err(|_| Error::Rejected)?;
                        Self::response(&[])
                    }
                    _ => Err(Error::Rejected),
                }
            })
            .map_err(|_| Error::Rejected)?;
        if !completed.session_is_current {
            return Err(Error::Rejected);
        }
        Self::active(&active).map_err(|_| Error::Rejected)?;
        completed.value
    }

    fn original_proof(
        &self,
        handle: u64,
        operation: [u8; 32],
    ) -> Result<iroha_core_zk::kagemusha_v1_state::KagemushaOutgoingStateProofArchivePairV1> {
        let (active, lease) = {
            let opens = self.opens.lock().map_err(|_| Error::Rejected)?;
            let open = opens.values.get(&handle).ok_or(Error::Rejected)?;
            Self::active(&open.active).map_err(|_| Error::Rejected)?;
            let Phase::Complete { lease, .. } = open.phase else {
                return Err(Error::Rejected);
            };
            (open.active.clone(), lease)
        };
        let completed = self
            .registry
            .dispatch_owner(lease, |owner| {
                Self::active(&active)?;
                let proof = owner.export_original_state_proof(operation)?;
                Self::active(&active)?;
                Ok::<_, RegistryError>(proof)
            })
            .map_err(|_| Error::Rejected)?;
        if !completed.session_is_current {
            return Err(Error::Rejected);
        }
        Self::active(&active).map_err(|_| Error::Rejected)?;
        completed.value.map_err(|_| Error::Rejected)
    }

    fn accept_sender_reply(&self, handle: u64, fields: &[Vec<u8>]) -> Result<Vec<u8>> {
        let (active, lease) = {
            let opens = self.opens.lock().map_err(|_| Error::Rejected)?;
            let open = opens.values.get(&handle).ok_or(Error::Rejected)?;
            Self::active(&open.active).map_err(|_| Error::Rejected)?;
            let Phase::Complete { lease, .. } = open.phase else {
                return Err(Error::Rejected);
            };
            (open.active.clone(), lease)
        };
        let completed = self
            .registry
            .dispatch_owner(lease, |owner| {
                Self::active(&active)?;
                owner.accept_sender_reply(fields)?;
                Self::active(&active)?;
                Ok::<_, RegistryError>(())
            })
            .map_err(|_| Error::Rejected)?;
        if !completed.session_is_current {
            return Err(Error::Rejected);
        }
        completed.value.map_err(|_| Error::Rejected)?;
        Self::active(&active).map_err(|_| Error::Rejected)?;
        Self::response(&[])
    }

    fn sender_intent(&self, handle: u64, method: Method, fields: &[Vec<u8>]) -> Result<Vec<u8>> {
        let (active, lease) = {
            let opens = self.opens.lock().map_err(|_| Error::Rejected)?;
            let open = opens.values.get(&handle).ok_or(Error::Rejected)?;
            Self::active(&open.active).map_err(|_| Error::Rejected)?;
            let Phase::Complete { lease, .. } = open.phase else {
                return Err(Error::Rejected);
            };
            (open.active.clone(), lease)
        };
        let completed = self
            .registry
            .dispatch_owner(lease, |owner| {
                Self::active(&active)?;
                let response = match method {
                    Method::ReserveOperationId => {
                        let operation = owner.reserve_sender_operation(fields)?;
                        Self::response(&[operation.to_vec()])
                            .map_err(|_| RegistryError::Rejected)?
                    }
                    Method::PrepareIncomingFold
                    | Method::CompleteIncomingFold
                    | Method::StageIncomingOriginal => {
                        let fields = owner.incoming_work(method, fields)?;
                        Self::response(&fields).map_err(|_| RegistryError::Rejected)?
                    }
                    Method::BuildTerminalEnvelope => {
                        let envelope = owner.build_terminal(fields)?;
                        Self::response(&[envelope]).map_err(|_| RegistryError::Rejected)?
                    }
                    Method::ProvePreparedSenderTransition => {
                        let candidate = owner.prove_sender(fields)?;
                        Self::response(&[candidate
                            .encode_canonical()
                            .map_err(|_| RegistryError::Rejected)?])
                        .map_err(|_| RegistryError::Rejected)?
                    }
                    Method::BeginSenderTransition => {
                        let intent = owner.begin_sender_intent(fields)?;
                        let original = intent
                            .encode_canonical()
                            .map_err(|_| RegistryError::Rejected)?;
                        Self::response(&[intent.operation_id.to_vec(), original])
                            .map_err(|_| RegistryError::Rejected)?
                    }
                    _ => return Err(RegistryError::Rejected),
                };
                Self::active(&active)?;
                Ok::<_, RegistryError>(response)
            })
            .map_err(|_| Error::Rejected)?;
        if !completed.session_is_current {
            return Err(Error::Rejected);
        }
        Self::active(&active).map_err(|_| Error::Rejected)?;
        completed.value.map_err(|_| Error::Rejected)
    }
}

impl KagemushaCoreCoordinatorBackendV1 for KagemushaAuthenticatedRecoveredCoordinatorV1 {
    fn open(&self, path: &str) -> Result<u64> {
        if path != self.path {
            return Err(Error::Rejected);
        }
        // Complete only the exact already authorized pending publication before a new
        // account/device challenge. No old usable predecessor or generic fallback exists.
        self.owner
            .lock()
            .map_err(|_| Error::Rejected)?
            .resume_original_publication()
            .map_err(|_| Error::Rejected)?;
        let mut opens = self.opens.lock().map_err(|_| Error::Rejected)?;
        for open in opens.values.values() {
            open.active.store(false, Ordering::Release);
        }
        self.registry
            .revoke_selection()
            .map_err(|_| Error::Rejected)?;
        opens.values.clear();
        let handle = opens.next;
        opens.next = opens.next.checked_add(1).ok_or(Error::Rejected)?;
        opens.values.insert(
            handle,
            Open {
                active: Arc::new(AtomicBool::new(true)),
                phase: Phase::Unbegun,
            },
        );
        Ok(handle)
    }
    fn invoke(&self, handle: u64, method: Method, frame: &[u8]) -> Result<Vec<u8>> {
        let result = (|| {
            kagemusha_core_coordinator_validate_method_request_v1(method, frame)
                .map_err(|_| Error::Rejected)?;
            let fields =
                kagemusha_core_coordinator_decode_request_v1(frame).map_err(|_| Error::Rejected)?;
            let sender_reply = method == Method::AcceptAuthenticatedReply
                && matches!(
                    u32::from_le_bytes(
                        fields[0]
                            .as_slice()
                            .try_into()
                            .map_err(|_| Error::Rejected)?
                    ),
                    5 | 6 | 7 | 8 | 9 | 10 | 12
                );
            let response = if method == Method::InitialEnrollment {
                self.recovered_phase(handle, frame)?
            } else if sender_reply {
                self.accept_sender_reply(handle, &fields)?
            } else if matches!(
                method,
                Method::ReserveOperationId
                    | Method::BeginSenderTransition
                    | Method::ProvePreparedSenderTransition
                    | Method::BuildTerminalEnvelope
                    | Method::PrepareIncomingFold
                    | Method::CompleteIncomingFold
                    | Method::StageIncomingOriginal
            ) {
                self.sender_intent(handle, method, &fields)?
            } else {
                self.observe(handle, method, frame)?
            };
            kagemusha_core_coordinator_validate_method_response_v1(method, frame, &response)
                .map_err(|_| Error::Rejected)?;
            Ok(response)
        })();
        if result.is_err() {
            let _ = self.close(handle);
        }
        result
    }
    fn invoke_initial_enrollment(&self, handle: u64, frame: &[u8]) -> Result<Vec<u8>> {
        self.invoke(handle, Method::InitialEnrollment, frame)
    }
    fn export_outgoing_state_proof(
        &self,
        handle: u64,
        operation: [u8; 32],
    ) -> Result<iroha_core_zk::kagemusha_v1_state::KagemushaOutgoingStateProofArchivePairV1> {
        let result = self.original_proof(handle, operation);
        if result.is_err() {
            let _ = self.close(handle);
        }
        result
    }
    fn close(&self, handle: u64) -> Result<()> {
        let open = self
            .opens
            .lock()
            .ok()
            .and_then(|mut opens| opens.values.remove(&handle));
        if let Some(open) = open {
            open.active.store(false, Ordering::Release);
            match open.phase {
                Phase::Pending(pending) => {
                    let _ = self.registry.cancel(pending.attempt_id);
                }
                Phase::Complete { lease, .. } => {
                    let _ = self.registry.close(lease);
                }
                Phase::Preparing | Phase::Completing => {
                    let _ = self.registry.revoke_selection();
                }
                _ => {}
            }
            Ok(())
        } else {
            Err(Error::Rejected)
        }
    }
}
