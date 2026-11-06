//! Bounded compact runtime witnesses from certified archived native writes.

use super::*;
use crate::state::{StorageReadOnly, WorldReadOnly};
use iroha_data_model::{
    runtime::definition::DefinitionDescriptorV1,
    sumeragi_finality::lifecycle_runtime::LifecycleRuntimeProofV1,
};

/// Original resource/source failure while constructing one bounded lifecycle witness.
#[derive(Debug, thiserror::Error)]
pub enum LifecycleWitnessReadError {
    /// Preserve native finality's original State, deadline, stop and resource refusals.
    #[error(transparent)]
    Finality(#[from] crate::state::FinalityProofIntervalReadError),
    /// Preserve the original transient decode workspace refusal.
    #[error(transparent)]
    Workspace(
        #[from]
        crate::execution_attempt::ExecutionAttemptError<
            crate::sumeragi::certified_chain::ChainReadError,
        >,
    ),
    /// Original archive descriptor, namespace, source or allocation failure.
    #[error(transparent)]
    Archive(#[from] crate::query::native_context_archive::NativeContextArchiveError),
    /// Original paid complete-write projection refusal.
    #[error(transparent)]
    Projection(#[from] ordinary_writes::WriteDecodeError),
    /// Original checked certified execution frame refusal.
    #[error(transparent)]
    Certified(#[from] iroha_data_model::sumeragi_finality::FinalityReadError),
    /// Original canonical lane-state payload decode refusal.
    #[error(transparent)]
    Codec(#[from] norito::Error),
    /// Original lane-state branch refusal.
    #[error(transparent)]
    Lane(#[from] iroha_data_model::sumeragi_finality::NativeLaneStateProofError),
    /// Original runtime branch refusal.
    #[error(transparent)]
    Branch(
        #[from]
        iroha_data_model::sumeragi_finality::lifecycle_runtime::LifecycleRuntimeProofBuildErrorV1,
    ),
    /// The original paid response backing could not be supplied.
    #[error(transparent)]
    Buffer(#[from] iroha_allocation::ChargedBufferError),
    /// Original bounded JSON refusal, retaining any enclosing allocation scope.
    #[error(transparent)]
    Json(#[from] norito::json::BoundedJsonError),
    /// This exact request, source or selected definition is invalid.
    #[error("{0}")]
    Invalid(&'static str),
}
impl LifecycleWitnessReadError {
    /// Whether this error retains a local resource refusal instead of a source verdict.
    pub fn is_local_refusal(&self) -> bool {
        use crate::execution_attempt::ExecutionAttemptError;
        use iroha_allocation::{AllocationRefusal, ChargedBufferError, PrepaidBufferError};
        use iroha_data_model::sumeragi_finality::FinalityReadError;
        fn admission(error: &AllocationRefusal) -> bool {
            matches!(
                error,
                AllocationRefusal::Capacity { .. } | AllocationRefusal::ExceedsLimit { .. }
            )
        }
        fn buffer(error: &ChargedBufferError) -> bool {
            match error {
                ChargedBufferError::Admission(error) => admission(error),
                ChargedBufferError::Allocator { .. } => true,
            }
        }
        match self {
            Self::Workspace(ExecutionAttemptError::Deferred(_)) => true,
            Self::Buffer(error) => buffer(error),
            Self::Json(
                norito::json::BoundedJsonError::BodyTooLarge
                | norito::json::BoundedJsonError::AllocationFailed
                | norito::json::BoundedJsonError::DecodeResource(_)
                | norito::json::BoundedJsonError::ScopedDecodeResource(_),
            ) => true,
            Self::Archive(error) => error.is_local_refusal(),
            Self::Projection(ordinary_writes::WriteDecodeError::Admission(error)) => {
                admission(error)
            }
            Self::Projection(ordinary_writes::WriteDecodeError::Materialization(
                PrepaidBufferError::Allocation(error),
            )) => buffer(error),
            Self::Certified(
                FinalityReadError::DecodeResource(_) | FinalityReadError::Allocator(_),
            ) => true,
            Self::Codec(error)
            | Self::Projection(ordinary_writes::WriteDecodeError::Codec(error)) => {
                error.is_decode_resource_limit()
            }
            Self::Lane(error) => error.is_local_refusal(),
            Self::Branch(error) => error.is_local_refusal(),
            _ => false,
        }
    }
}

/// One paid JSON allocation; its original frame-pool charge remains until the final byte retires.
pub struct LifecycleWitnessBytes(iroha_allocation::ChargedBuffer<u8>);
impl AsRef<[u8]> for LifecycleWitnessBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}
impl norito::json::JsonWriteSink for LifecycleWitnessBytes {
    fn push(&mut self, value: char) -> Result<(), norito::json::BoundedJsonError> {
        let mut bytes = [0; 4];
        self.push_str(value.encode_utf8(&mut bytes))
    }
    fn push_str(&mut self, value: &str) -> Result<(), norito::json::BoundedJsonError> {
        self.0
            .append(value.as_bytes())
            .map_err(|_| norito::json::BoundedJsonError::BodyTooLarge)
    }
}
impl LifecycleWitnessBytes {
    fn text(&mut self, value: &str) -> Result<(), LifecycleWitnessReadError> {
        use norito::json::JsonWriteSink;
        self.push_str(value)
            .map_err(|error| LifecycleWitnessReadError::Json(error))
    }
    fn value(
        &mut self,
        value: &impl norito::json::JsonSerialize,
    ) -> Result<(), LifecycleWitnessReadError> {
        value
            .json_serialize_to(self)
            .map_err(|error| LifecycleWitnessReadError::Json(error))
    }
}

// Fixed transport has at most 256 siblings; 32 KiB covers every maximal field spelling.
// The descriptor header is counted exactly from the same borrowed State view.
const MAX_RUNTIME_BRANCH_JSON_BYTES: usize = 32 * 1024;
struct WitnessJsonCount {
    length: usize,
    limit: usize,
}
impl norito::json::JsonWriteSink for WitnessJsonCount {
    fn push(&mut self, value: char) -> Result<(), norito::json::BoundedJsonError> {
        self.length = self
            .length
            .checked_add(value.len_utf8())
            .filter(|length| *length <= self.limit)
            .ok_or(norito::json::BoundedJsonError::BodyTooLarge)?;
        Ok(())
    }
    fn push_str(&mut self, value: &str) -> Result<(), norito::json::BoundedJsonError> {
        self.length = self
            .length
            .checked_add(value.len())
            .filter(|length| *length <= self.limit)
            .ok_or(norito::json::BoundedJsonError::BodyTooLarge)?;
        Ok(())
    }
}
fn write_header(
    out: &mut dyn norito::json::JsonWriteSink,
    selection: &impl norito::json::JsonSerialize,
    services: &impl norito::json::JsonSerialize,
    implementation: &impl norito::json::JsonSerialize,
) -> Result<(), LifecycleWitnessReadError> {
    (|| -> Result<(), norito::json::BoundedJsonError> {
        // Match the original JSON object's lexical key order without constructing a DTO map.
        out.push_str("{\"implementation\":")?;
        implementation.json_serialize_to(out)?;
        out.push_str(",\"selection\":")?;
        selection.json_serialize_to(out)?;
        out.push_str(",\"services\":")?;
        services.json_serialize_to(out)
    })()
    .map_err(|error| LifecycleWitnessReadError::Json(error))
}

fn stop_check(
    deadline: std::time::Instant,
    stop: &std::sync::atomic::AtomicBool,
) -> Result<(), LifecycleWitnessReadError> {
    use crate::sumeragi::finality::NativeFinalityProofIntervalError;
    let error = if stop.load(std::sync::atomic::Ordering::Acquire) {
        Some(NativeFinalityProofIntervalError::Cancelled)
    } else if std::time::Instant::now() >= deadline {
        Some(NativeFinalityProofIntervalError::Deadline)
    } else {
        None
    };
    if let Some(error) = error {
        return Err(crate::state::FinalityProofIntervalReadError::Proof(error).into());
    }
    Ok(())
}
fn short_view<'a>(
    state: &'a crate::state::State,
    deadline: std::time::Instant,
    stop: &std::sync::atomic::AtomicBool,
    waiter: &mut crate::state::StateViewWaiter,
) -> Result<crate::state::StateView<'a>, LifecycleWitnessReadError> {
    loop {
        stop_check(deadline, stop)?;
        match state.try_view_once() {
            Ok(view) => return Ok(view),
            Err(crate::state::StateViewError::Busy(release)) => {
                if !waiter.wait(&release, deadline, stop) {
                    stop_check(deadline, stop)?;
                    return Err(crate::state::FinalityProofIntervalReadError::StateView(
                        crate::state::StateViewError::Busy(release),
                    )
                    .into());
                }
            }
            Err(error) => {
                return Err(crate::state::FinalityProofIntervalReadError::StateView(error).into());
            }
        }
    }
}

/// Produce the exact scheduled-witness JSON using this State's verified prefix ladder.
///
/// No World view spans native history work. Every cut borrows its paid native proof,
/// joins the same original archive namespace and full write root, and emits directly
/// into one prepaid response buffer. The original deadline, stop flag, frame pool and
/// transient graph corridor survive all chunks; no proof DTO or JSON branch is cloned.
/// # Errors
/// Refuses invalid ranges, changed selections/sources, corrupt archives and original
/// resource/deadline/cancellation failures. At most 4096 cuts and 32 MiB are admitted.
#[expect(
    clippy::too_many_arguments,
    reason = "All owners and bounds are borrowed explicitly; none may be reconstructed from a configuration cap"
)]
pub fn lifecycle_activation_witness_owned(
    state: &crate::state::State,
    owner: &crate::sumeragi::finality::NativeFinalityProofBudget,
    start_height: u64,
    end_height: u64,
    response_limit: usize,
    deadline: std::time::Instant,
    stop: &std::sync::atomic::AtomicBool,
    waiter: &mut crate::state::StateViewWaiter,
) -> Result<LifecycleWitnessBytes, LifecycleWitnessReadError> {
    use crate::sumeragi::finality::NativeFinalityProofIntervalLimits;
    use iroha_data_model::sumeragi_finality::MAX_FINALITY_BLOCK_BYTES;
    use std::num::{NonZeroU64, NonZeroUsize};
    const MAX_BYTES: usize = 32 * 1024 * 1024;
    if start_height < 2 || end_height < start_height || end_height - start_height >= 4096 {
        return Err(LifecycleWitnessReadError::Invalid(
            "lifecycle witness requires 1..=4096 certified consecutive cuts",
        ));
    }
    stop_check(deadline, stop)?;
    let network = *state.network_id_ref();
    let end_index = usize::try_from(end_height - 1)
        .map_err(|_| LifecycleWitnessReadError::Invalid("lifecycle height exceeds platform"))?;
    let (mut output, selected, scope, end_hash) = {
        let view = short_view(state, deadline, stop, waiter)?;
        let selection = view.world().runtime_selection();
        let selected = selection.id().map_err(|_| {
            LifecycleWitnessReadError::Invalid("invalid selected lifecycle definition")
        })?;
        let definitions = view.world().runtime_definitions();
        let services = definitions.get(&selection.services_profile).ok_or(
            LifecycleWitnessReadError::Invalid("missing lifecycle services"),
        )?;
        let DefinitionDescriptorV1::ServicesProfile(profile) = services else {
            return Err(LifecycleWitnessReadError::Invalid(
                "selected lifecycle services have wrong kind",
            ));
        };
        if services.id().ok() != Some(selection.services_profile)
            || !profile.services.iter().any(|service| {
                service.name == "trigger_lifecycle"
                    && service.reference_closure
                        == <[u8; 32]>::from(iroha_crypto::Hash::new(
                            iroha_data_model::runtime::TRIGGER_LIFECYCLE_SERVICE_CLOSURE,
                        ))
            })
        {
            return Err(LifecycleWitnessReadError::Invalid(
                "current selection has no supported lifecycle capture",
            ));
        }
        let implementation_id = selection
            .bindings
            .first()
            .ok_or(LifecycleWitnessReadError::Invalid(
                "lifecycle selection has no bindings",
            ))?
            .implementation;
        if selection
            .bindings
            .iter()
            .any(|binding| binding.implementation != implementation_id)
        {
            return Err(LifecycleWitnessReadError::Invalid(
                "lifecycle witness requires one complete native bundle",
            ));
        }
        let implementation =
            definitions
                .get(&implementation_id)
                .ok_or(LifecycleWitnessReadError::Invalid(
                    "missing lifecycle implementation",
                ))?;
        if implementation.id().ok() != Some(implementation_id) {
            return Err(LifecycleWitnessReadError::Invalid(
                "lifecycle implementation identity changed",
            ));
        }
        let end_hash = view.block_hashes().get(end_index).copied().ok_or(
            LifecycleWitnessReadError::Invalid("requested lifecycle cut is not committed"),
        )?;
        let scope = view
            .native_execution_tip()
            .ok_or(LifecycleWitnessReadError::Invalid(
                "native execution scope is unavailable",
            ))?
            .root_scope()
            .into();
        let mut count = WitnessJsonCount {
            length: 0,
            limit: response_limit.min(MAX_BYTES),
        };
        write_header(&mut count, selection, services, implementation)?;
        let capacity = usize::try_from(end_height - start_height + 1)
            .ok()
            .and_then(|cuts| cuts.checked_mul(MAX_RUNTIME_BRANCH_JSON_BYTES))
            .and_then(|branches| branches.checked_add(count.length))
            .and_then(|bytes| bytes.checked_add(128))
            .ok_or(LifecycleWitnessReadError::Invalid(
                "lifecycle response demand overflow",
            ))?
            .min(response_limit)
            .min(MAX_BYTES);
        let mut output = LifecycleWitnessBytes(iroha_allocation::ChargedBuffer::new(
            capacity,
            owner.frames(),
        )?);
        write_header(&mut output, selection, services, implementation)?;
        (output, selected, scope, end_hash)
    };
    let mut archive = NativeContextArchive::open_existing(
        state.kura(),
        owner.frames().clone(),
        state.kura().native_context_archive_max_bytes(),
    )?;
    let mut first = start_height;
    while first <= end_height {
        stop_check(deadline, stop)?;
        let last = first.saturating_add(15).min(end_height);
        let limits = NativeFinalityProofIntervalLimits::new(
            NonZeroU64::new(first).expect("positive checked first height"),
            NonZeroU64::new(last).expect("positive checked last height"),
            NonZeroUsize::new(MAX_FINALITY_BLOCK_BYTES).expect("positive canonical wire maximum"),
            NonZeroUsize::new(owner.frames().limit_bytes()).ok_or(
                LifecycleWitnessReadError::Invalid("empty original frame corridor"),
            )?,
            deadline,
        )
        .map_err(crate::state::FinalityProofIntervalReadError::Proof)?;
        let interval = state.read_finality_proof_interval_waiting(owner, &limits, stop, waiter)?;
        let workspace = owner.workspace()?;
        for index in 0..interval.len() {
            let height = first + index as u64;
            let proof = interval
                .proof(index)
                .expect("immutable paid interval member");
            if height == end_height && proof.block_header.hash() != end_hash {
                return Err(LifecycleWitnessReadError::Invalid(
                    "lifecycle final carrier changed during production",
                ));
            }
            stop_check(deadline, stop)?;
            let mut read = archive.read_job(height, proof.block_header.hash());
            let bytes = loop {
                stop_check(deadline, stop)?;
                match read.poll() {
                    Ok(Some(bytes)) => break bytes,
                    Ok(None) => {}
                    Err(crate::query::native_context_archive::NativeContextArchiveError::Io(
                        error,
                    )) if error.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(error) => return Err(error.into()),
                }
            };
            read.recheck_namespace()?;
            archive = read.into_archive();
            workspace.stage(|| -> Result<(), LifecycleWitnessReadError> {
                let projection = checked_original_projection(
                    &bytes,
                    owner.frames(),
                    network,
                    height,
                    scope,
                    proof,
                )?;
                let branch = LifecycleRuntimeProofV1::from_witness_owned(
                    projection.witness.get(),
                    network,
                    height,
                    owner.frames(),
                )?;
                if branch.proof().execution_selection != selected
                    || branch.proof().post_selection != selected
                {
                    return Err(LifecycleWitnessReadError::Invalid(
                        "lifecycle runtime selection differs within requested window",
                    ));
                }
                output.text(if height == start_height {
                    ",\"runtime_proof\":"
                } else if height == start_height + 1 {
                    ",\"continuation_proofs\":["
                } else {
                    ","
                })?;
                output.value(branch.proof())?;
                Ok(())
            })?;
            archive.recheck_namespace()?;
            stop_check(deadline, stop)?;
        }
        first = last
            .checked_add(1)
            .ok_or(LifecycleWitnessReadError::Invalid(
                "lifecycle height overflow",
            ))?;
    }
    output.text(if start_height == end_height {
        ",\"continuation_proofs\":[]}"
    } else {
        "]}"
    })?;
    archive.recheck_namespace()?;
    {
        let view = short_view(state, deadline, stop, waiter)?;
        if view.world().runtime_selection().id().ok() != Some(selected)
            || view.block_hashes().get(end_index).copied() != Some(end_hash)
            || view
                .native_execution_tip()
                .map(|tip| tip.root_scope().into())
                != Some(scope)
        {
            return Err(LifecycleWitnessReadError::Invalid(
                "lifecycle selection or original final carrier changed before return",
            ));
        }
    }
    stop_check(deadline, stop)?;
    Ok(output)
}

/// Assemble the Python scheduled verifier's exact JSON witness for a consecutive window.
///
/// Uses one committed view and original durable archives, never mutable trigger lookups.
/// Consecutive cuts reuse the native verified reader after its original-byte fences;
/// they do not rebuild a separate signed-genesis verifier for every height.
/// The window must still select the current immutable lifecycle closure; a changed
/// selection fails closed. At most 4096 cuts and 32 MiB of JSON are returned.
/// # Errors
/// Missing/corrupt archives, absent native anchors, selection changes or bounds.
#[cfg(test)]
fn lifecycle_activation_witness(
    view: &impl StateReadOnly,
    start_height: u64,
    end_height: u64,
) -> Result<String, String> {
    const MAX_BYTES: usize = 32 * 1024 * 1024;
    if start_height < 2 || end_height < start_height || end_height - start_height >= 4096 {
        return Err("lifecycle witness requires 1..=4096 certified consecutive cuts".into());
    }
    let selection = view.world().runtime_selection();
    let selected = selection.id().map_err(|e| e.to_string())?;
    let definitions = view.world().runtime_definitions();
    let services = definitions
        .get(&selection.services_profile)
        .ok_or("missing lifecycle services")?;
    let DefinitionDescriptorV1::ServicesProfile(profile) = services else {
        return Err("selected lifecycle services have wrong kind".into());
    };
    if !profile.services.iter().any(|service| {
        service.name == "trigger_lifecycle"
            && service.reference_closure
                == <[u8; 32]>::from(iroha_crypto::Hash::new(
                    iroha_data_model::runtime::TRIGGER_LIFECYCLE_SERVICE_CLOSURE,
                ))
    }) {
        return Err("current selection has no supported lifecycle capture".into());
    }
    let implementation_id = selection
        .bindings
        .first()
        .ok_or("lifecycle selection has no bindings")?
        .implementation;
    if selection
        .bindings
        .iter()
        .any(|binding| binding.implementation != implementation_id)
    {
        return Err("lifecycle witness requires one complete native bundle".into());
    }
    let implementation = definitions
        .get(&implementation_id)
        .ok_or("missing lifecycle implementation")?;
    let mut json = norito::json::to_json(&norito::json!({
        "selection": selection, "services": services, "implementation": implementation,
    }))
    .map_err(|e| e.to_string())?;
    json.pop(); // Replace the final object delimiter with the two bounded proof fields.
    let scope = view
        .native_execution_tip()
        .ok_or("native execution scope is unavailable")?
        .root_scope()
        .into();
    crate::sumeragi::finality::with_proof_reader(view, |reader| -> Result<(), String> {
        for height in start_height..=end_height {
            let finality = reader.proof(height).map_err(|error| error.to_string())?;
            let source = original_source_from_proof(view, height, scope, finality)?;
            let proof = LifecycleRuntimeProofV1::from_witness(
                source.witness.get(),
                *view.network_id(),
                height,
                &view.execution_budget(),
            )?;
            let original = proof.proof();
            if original.execution_selection != selected || original.post_selection != selected {
                return Err("lifecycle runtime selection differs within requested window".into());
            }
            let branch = norito::json::to_json(original).map_err(|e| e.to_string())?;
            let prefix = if height == start_height {
                ",\"runtime_proof\":"
            } else if height == start_height + 1 {
                ",\"continuation_proofs\":["
            } else {
                ","
            };
            let required = json
                .len()
                .checked_add(prefix.len())
                .and_then(|n| n.checked_add(branch.len()))
                .and_then(|n| n.checked_add(32))
                .ok_or("lifecycle witness size overflow")?;
            if required > MAX_BYTES {
                return Err("lifecycle witness exceeds SDK JSON bound".into());
            }
            json.try_reserve(prefix.len() + branch.len() + 32)
                .map_err(|e| e.to_string())?;
            json.push_str(prefix);
            json.push_str(&branch);
        }
        Ok(())
    })?;
    json.push_str(if start_height == end_height {
        ",\"continuation_proofs\":[]}"
    } else {
        "]}"
    });
    Ok(json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};

    fn paid_read(
        state: &crate::state::State,
        frames: &iroha_allocation::AllocationBudget,
        start: u64,
        end: u64,
        limit: usize,
        stopped: bool,
        expired: bool,
    ) -> Result<LifecycleWitnessBytes, LifecycleWitnessReadError> {
        let owner = crate::sumeragi::finality::NativeFinalityProofBudget::new(
            frames.clone(),
            iroha_allocation::AllocationBudget::new(33_312_848),
            norito::DecodeLimits::new(
                2_562_487,
                iroha_data_model::sumeragi_finality::MAX_FINALITY_BLOCK_BYTES,
                33_312_848,
                33_312_848,
                norito::core::MAX_VALUE_NESTING_DEPTH,
            ),
        );
        let controls = iroha_allocation::AllocationBudget::new(4096);
        let mut reservation = controls
            .try_reserve(crate::state::StateViewWaiter::allocation_layout())
            .unwrap();
        let mut waiter =
            crate::state::StateViewWaiter::for_current_thread(&mut reservation).unwrap();
        let now = std::time::Instant::now();
        lifecycle_activation_witness_owned(
            state,
            &owner,
            start,
            end,
            limit,
            if expired {
                now
            } else {
                now + std::time::Duration::from_secs(60)
            },
            &std::sync::atomic::AtomicBool::new(stopped),
            &mut waiter,
        )
    }

    #[test]
    fn lifecycle_transport_maximal_branch_fits_its_prepaid_cut_bound() {
        use iroha_data_model::sumeragi_finality::lifecycle_runtime::LifecycleRuntimeProofStepV1;
        let proof = LifecycleRuntimeProofV1 {
            height: u64::MAX,
            execution_selection: iroha_data_model::runtime::definition::SelectionId([255; 32]),
            post_selection: iroha_data_model::runtime::definition::SelectionId([255; 32]),
            siblings: (0..=255)
                .map(|level| LifecycleRuntimeProofStepV1 {
                    level,
                    hash: iroha_crypto::Hash::new([level]),
                })
                .collect(),
        };
        assert!(norito::json::to_json(&proof).unwrap().len() < MAX_RUNTIME_BRANCH_JSON_BYTES);
    }

    fn assert_same_witness(actual: &[u8], expected: &str) {
        let actual = std::str::from_utf8(actual).unwrap();
        let actual_value: norito::json::Value = norito::json::from_str(actual).unwrap();
        let expected_value: norito::json::Value = norito::json::from_str(expected).unwrap();
        // The paid writer emits typed descriptor fields directly; the old Value
        // intermediary sorted their object keys. JSON object order grants no proof
        // authority. Every field and the exact original branch bytes must agree.
        assert!(actual_value == expected_value, "witness fields changed");
        let proof_fields = |text: &str| text.find(",\"runtime_proof\":").unwrap();
        assert!(
            actual[proof_fields(actual)..] == expected[proof_fields(expected)..],
            "original runtime and continuation proof bytes changed"
        );
    }

    #[test]
    fn lifecycle_valid_projection_preserves_inherited_decode_refusal_and_original_bytes() {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(crate::state::World::new(), 1000))
                .unwrap();
        chain.commit(Vec::new());
        let frames = iroha_allocation::AllocationBudget::new(64 * 1024 * 1024);
        let archive = NativeContextArchive::open_existing(
            chain.kura(),
            frames.clone(),
            chain.kura().native_context_archive_max_bytes(),
        )
        .unwrap();
        let bytes = archive
            .read_exact(2, chain.committed(2).block_hash())
            .unwrap();
        let pointer = bytes.as_slice().as_ptr();
        let original = iroha_crypto::Hash::new(bytes.as_slice());
        let held = frames.reserved_bytes();
        drop(ordinary_writes::decode_projection(&bytes, &frames).unwrap());
        assert_eq!(frames.reserved_bytes(), held);
        for limits in [
            norito::DecodeLimits::new(usize::MAX, 1, usize::MAX, usize::MAX, usize::MAX),
            norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 1, usize::MAX),
        ] {
            // The original committed projection remains valid. Only the enclosing
            // caller's field/aggregate budget refuses this acquisition.
            let error = norito::core::with_decode_limits_scope(limits, || {
                ordinary_writes::decode_projection(&bytes, &frames)
            })
            .err()
            .expect("the inherited projection decode ceiling must refuse");
            assert!(
                matches!(&error, ordinary_writes::WriteDecodeError::Codec(error)
                if error.is_decode_resource_limit())
            );
            let error = LifecycleWitnessReadError::Projection(error);
            assert!(error.is_local_refusal(), "{error}");
            assert_eq!(bytes.as_slice().as_ptr(), pointer);
            assert_eq!(iroha_crypto::Hash::new(bytes.as_slice()), original);
            assert_eq!(frames.reserved_bytes(), held);
            drop(ordinary_writes::decode_projection(&bytes, &frames).unwrap());
            assert_eq!(frames.reserved_bytes(), held);
        }
        drop(bytes);
        assert_eq!(frames.reserved_bytes(), 0);
    }

    #[test]
    fn lifecycle_projection_semantic_and_ownership_errors_are_not_local_refusals() {
        use iroha_allocation::{
            AllocationRefusal, ChargedBufferError, InsufficientReservation, PrepaidBufferError,
        };
        use ordinary_writes::WriteDecodeError;
        for original in [
            WriteDecodeError::Codec(norito::Error::NonCanonicalEncoding),
            WriteDecodeError::Codec(norito::Error::ChecksumMismatch),
            WriteDecodeError::ForeignPool,
            WriteDecodeError::PlanChanged,
            WriteDecodeError::Admission(AllocationRefusal::DemandOverflow),
            WriteDecodeError::Materialization(PrepaidBufferError::Reservation(
                InsufficientReservation {
                    requested_bytes: 2,
                    remaining_bytes: 1,
                },
            )),
            WriteDecodeError::Materialization(PrepaidBufferError::Allocation(
                ChargedBufferError::Admission(AllocationRefusal::DemandOverflow),
            )),
        ] {
            assert!(!LifecycleWitnessReadError::Projection(original).is_local_refusal());
        }
        assert!(
            !LifecycleWitnessReadError::Buffer(ChargedBufferError::Admission(
                AllocationRefusal::DemandOverflow,
            ))
            .is_local_refusal()
        );
    }

    #[test]
    fn lifecycle_live_witness_requires_selected_consecutive_original_archives() {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(crate::state::World::new(), 1000))
                .unwrap();
        chain.commit(Vec::new());
        assert!(lifecycle_activation_witness(&chain.state().view(), 2, 2).is_err());
        chain.setup_world_at(1002, |tx| {
            let catalog = crate::runtime::release_catalog().unwrap();
            for (id, descriptor) in &catalog.definitions {
                if tx.world.runtime_definitions.get(id).is_none() {
                    tx.world.runtime_definitions.insert(*id, descriptor.clone());
                }
            }
            *tx.world.runtime_selection.get_mut() = catalog.lifecycle.clone();
        });
        chain.commit_at(1002, Vec::new());
        chain.commit_at(1003, Vec::new());
        let json = lifecycle_activation_witness(&chain.state().view(), 3, 4).unwrap();
        let frames = iroha_allocation::AllocationBudget::new(64 * 1024 * 1024);
        let owned =
            paid_read(chain.state(), &frames, 3, 4, 32 * 1024 * 1024, false, false).unwrap();
        assert_same_witness(owned.as_ref(), &json);
        assert!(frames.reserved_bytes() > 0);
        drop(owned);
        assert_eq!(
            frames.reserved_bytes(),
            0,
            "all request backing retires with the response"
        );
        for (stopped, expired) in [(true, false), (false, true)] {
            assert!(matches!(
                paid_read(
                    chain.state(),
                    &frames,
                    3,
                    4,
                    32 * 1024 * 1024,
                    stopped,
                    expired
                ),
                Err(LifecycleWitnessReadError::Finality(
                    crate::state::FinalityProofIntervalReadError::Proof(
                        crate::sumeragi::finality::NativeFinalityProofIntervalError::Cancelled
                            | crate::sumeragi::finality::NativeFinalityProofIntervalError::Deadline
                    )
                ))
            ));
            assert_eq!(frames.reserved_bytes(), 0);
        }
        let tiny = iroha_allocation::AllocationBudget::new(1);
        assert!(matches!(
            paid_read(chain.state(), &tiny, 3, 4, 32 * 1024 * 1024, false, false),
            Err(LifecycleWitnessReadError::Buffer(_))
        ));
        assert_eq!(tiny.reserved_bytes(), 0);
        assert!(paid_read(chain.state(), &frames, 2, 4, 32 * 1024 * 1024, false, false).is_err());
        assert!(
            paid_read(
                chain.state(),
                &frames,
                3,
                4099,
                32 * 1024 * 1024,
                false,
                false
            )
            .is_err()
        );
        assert!(paid_read(chain.state(), &frames, 4, 3, 32 * 1024 * 1024, false, false).is_err());
        assert!(paid_read(chain.state(), &frames, 3, 4, 1, false, false).is_err());

        let value: norito::json::Value = norito::json::from_json(&json).unwrap();
        assert_eq!(value["continuation_proofs"].as_array().unwrap().len(), 1);
        for (height, branch) in [
            (3, &value["runtime_proof"]),
            (4, &value["continuation_proofs"][0]),
        ] {
            let proof: LifecycleRuntimeProofV1 = norito::json::from_value(branch.clone()).unwrap();
            // Compare with the independent single-height archive reader. Sequential
            // finality reuse must not change even one projected branch byte.
            let view = chain.state().view();
            let single = original_source(&view, height).unwrap();
            let single_branch = LifecycleRuntimeProofV1::from_witness(
                single.witness.get(),
                chain.network_id(),
                height,
                &view.execution_budget(),
            )
            .unwrap();
            assert_eq!(
                norito::json::to_value(single_branch.proof()).unwrap(),
                *branch
            );
            let certified = build_proof(&chain.state().view(), height).unwrap();
            let execution = certified
                .decode_checked(
                    iroha_data_model::sumeragi_finality::CertifiedResultScope::TairaGlobal,
                )
                .unwrap();
            assert_eq!(
                proof.computed_root(chain.network_id()).unwrap(),
                execution.execution().ordinary_writes_root
            );
        }
        assert!(lifecycle_activation_witness(&chain.state().view(), 2, 4).is_err());
        assert!(lifecycle_activation_witness(&chain.state().view(), 3, 4099).is_err());
        assert!(lifecycle_activation_witness(&chain.state().view(), 4, 3).is_err());
        let path = std::fs::read_dir(chain.kura().store_root().join("native-contexts"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .find(|path| {
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("00000000000000000003-")
            })
            .unwrap();
        let original = std::fs::read(&path).unwrap();
        std::fs::write(&path, b"foreign archive bytes").unwrap();
        assert!(paid_read(chain.state(), &frames, 3, 4, 32 * 1024 * 1024, false, false).is_err());
        assert_eq!(frames.reserved_bytes(), 0);
        std::fs::write(&path, original).unwrap();
        assert_same_witness(
            paid_read(chain.state(), &frames, 3, 4, 32 * 1024 * 1024, false, false)
                .unwrap()
                .as_ref(),
            &json,
        );
        // Cross the producer's sixteen-cut chunk boundary, with a stable appended tip.
        // Each original branch must remain identical to the independent single-view oracle.
        while chain.height() < 19 {
            chain.commit(Vec::new());
        }
        let across_chunks = lifecycle_activation_witness(&chain.state().view(), 3, 19).unwrap();
        let owned = paid_read(
            chain.state(),
            &frames,
            3,
            19,
            32 * 1024 * 1024,
            false,
            false,
        )
        .unwrap();
        assert_same_witness(owned.as_ref(), &across_chunks);
        drop(owned);
        assert_eq!(frames.reserved_bytes(), 0);
    }
}
