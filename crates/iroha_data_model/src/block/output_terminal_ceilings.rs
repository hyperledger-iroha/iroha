//! Canonical ceilings for the exact pre-reserved output-limit terminal rows.

use iroha_crypto::Hash;
use iroha_model_base::name::MAX_NAME_BYTES;

use super::{ExecutionOutputEnvelope, ExecutionOutputPhase, output_bytes};
use crate::{
    block::execution_output::{
        ExecutionOutputV1, PipelineEventPositionV1, PipelineInvocationV1, TimeInvocationV1,
        TriggerUseV1,
    },
    events::time::{TimeEvent, TimeInterval},
};

/// Internally derived canonical frame ceilings for bounded terminal rows.
///
/// Private fields and the absence of a scalar/codec constructor prevent treating
/// caller-asserted byte ceilings as this derived value. The value is reusable
/// arithmetic, not policy, State/source authority or a reservation of host memory.
/// It bounds the exact output-limit terminal constructors and the shorter
/// internal-rejection diagnostic reused within those reservations, not successful
/// outputs, arbitrary rejection diagnostics or the complete executed block wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionOutputTerminalCeilings {
    bytes: [u64; 3],
}

impl ExecutionOutputTerminalCeilings {
    /// Derive each phase's ceiling from its maximum bounded descriptor shape.
    ///
    /// Canonical V1 encodes `u32`/`u64` and hashes at fixed widths. Every terminal
    /// collection, result variant, diagnostic and completion is fixed by its
    /// existing constructor. The only variable-sized field is the trigger Name,
    /// encoded as raw UTF-8 bounded by [`MAX_NAME_BYTES`], appearing both in the
    /// invocation and its root completion. Canonical framing/length-prefix costs
    /// grow monotonically with that byte length; character count is irrelevant.
    /// Both Pipeline event alternatives are measured, rather than assuming that
    /// the alternative with a payload is always the larger complete frame.
    ///
    /// The four sizing rows are bounded values, not accepted execution claims.
    /// Their scalar extremes cover encodings even when a source index cannot be
    /// admitted under a particular policy. The Time interval itself does not
    /// overflow, and registration can precede an applying height of `u64::MAX`.
    /// Sizing performs real canonical serialization without allocating complete encoded
    /// row buffers and ignores ambient codec layout guards. It does allocate the
    /// small, bounded descriptors/terminals; callers may retain the derived
    /// value instead of repeating this work for each invocation.
    ///
    /// # Errors
    /// Rejects failure to construct the bounded Name or count canonical bytes.
    /// Such failure is local refusal, never a canonical execution rejection.
    pub fn derive() -> Result<Self, String> {
        Self::derive_for_lifecycle(false)
    }

    /// Derive terminals for the execution semantics selected by the caller's root.
    /// V2 adds explicit empty coverage; this scalar never authenticates activation.
    /// # Errors
    /// Returns canonical construction or sizing failures.
    pub fn derive_for_lifecycle(lifecycle: bool) -> Result<Self, String> {
        let trigger = TriggerUseV1 {
            trigger_id: "x"
                .repeat(MAX_NAME_BYTES)
                .parse::<crate::trigger::TriggerId>()
                .map_err(|error| format!("cannot construct maximum terminal Name: {error}"))?,
            registered_at_height: u64::MAX - 1,
            action_hash: Hash::prehashed([0xff; Hash::LENGTH]),
        };
        let network = output_bytes(&coverage(
            ExecutionOutputV1::network_output_limit_rejection(u32::MAX),
            lifecycle,
        ))?;
        let mut pipeline = 0;
        for event in [
            PipelineEventPositionV1::Network(u32::MAX),
            PipelineEventPositionV1::BlockApproved,
        ] {
            let row = ExecutionOutputV1::pipeline_output_limit_rejection(PipelineInvocationV1 {
                event,
                candidate_index: u32::MAX,
                trigger: trigger.clone(),
            });
            pipeline = pipeline.max(output_bytes(&coverage(row, lifecycle))?);
        }
        let mut time = output_bytes(&coverage(
            ExecutionOutputV1::time_output_limit_rejection(TimeInvocationV1 {
                schedule_index: u32::MAX,
                event: TimeEvent {
                    interval: TimeInterval {
                        since_ms: u64::MAX - 1,
                        length_ms: 1,
                    },
                },
                trigger,
            }),
            lifecycle,
        ))?;
        if lifecycle {
            // A native failure policy changes only its root: one Update (retry
            // or quarantine), one progress debit, or one Remove (exhaustion).
            // Retry advancement includes its debit in the same Update. Two compact states
            // bound either case independently of executable/metadata size.
            let trigger = TriggerUseV1 {
                trigger_id: "x"
                    .repeat(MAX_NAME_BYTES)
                    .parse()
                    .map_err(|_| "terminal name")?,
                registered_at_height: u64::MAX - 1,
                action_hash: Hash::prehashed([0xff; Hash::LENGTH]),
            };
            let mut row = coverage(
                ExecutionOutputV1::time_output_limit_rejection(TimeInvocationV1 {
                    schedule_index: u32::MAX,
                    event: TimeEvent {
                        interval: TimeInterval {
                            since_ms: u64::MAX - 1,
                            length_ms: 1,
                        },
                    },
                    trigger: trigger.clone(),
                }),
                true,
            );
            reserve_policy_shape(&mut row, &trigger)?;
            time = time.max(output_bytes(&row)?);
            for event in [
                PipelineEventPositionV1::Network(u32::MAX),
                PipelineEventPositionV1::BlockApproved,
            ] {
                let mut row = coverage(
                    ExecutionOutputV1::pipeline_output_limit_rejection(PipelineInvocationV1 {
                        event,
                        candidate_index: u32::MAX,
                        trigger: trigger.clone(),
                    }),
                    true,
                );
                reserve_policy_shape(&mut row, &trigger)?;
                pipeline = pipeline.max(output_bytes(&row)?);
            }
        }
        Ok(Self {
            bytes: [network, pipeline, time],
        })
    }

    /// Return this phase's complete canonical terminal-row frame ceiling.
    pub fn for_phase(self, phase: ExecutionOutputPhase) -> u64 {
        self.bytes[phase.index()]
    }

    /// Combine derived byte ceilings with prospective callback counts.
    ///
    /// Only an absent phase receives a zero ceiling, as required by the existing
    /// envelope arithmetic. Counts remain untrusted: State must derive them from
    /// agreed applying policy and all eligible registrations, including disabled
    /// actions that earlier work could enable. This does not prove that a complete
    /// admitted source, its metadata and host allocations fit a future carrier.
    pub fn envelope(
        self,
        pipeline_candidates_per_event: u32,
        max_time_invocations: u32,
    ) -> ExecutionOutputEnvelope {
        ExecutionOutputEnvelope {
            pipeline_candidates_per_event,
            max_time_invocations,
            terminal_bytes: [
                self.bytes[0],
                if pipeline_candidates_per_event == 0 {
                    0
                } else {
                    self.bytes[1]
                },
                if max_time_invocations == 0 {
                    0
                } else {
                    self.bytes[2]
                },
            ],
        }
    }
}

#[cfg(test)]
#[path = "output_terminal_ceilings_tests.rs"]
mod tests;

fn coverage(row: ExecutionOutputV1, enabled: bool) -> ExecutionOutputV1 {
    if !enabled {
        return row;
    }
    use crate::block::execution_output::*;
    let trigger_lifecycle = TriggerLifecycleEffectsV1::Complete {
        effects: Vec::new(),
    };
    match row {
        ExecutionOutputV1::Network(execution) => {
            ExecutionOutputV1::NetworkWithLifecycle(NetworkExecutionOutputV2 {
                execution,
                trigger_lifecycle,
            })
        }
        ExecutionOutputV1::Pipeline(execution) => {
            ExecutionOutputV1::PipelineWithLifecycle(PipelineExecutionOutputV2 {
                execution,
                trigger_lifecycle,
            })
        }
        ExecutionOutputV1::Time(execution) => {
            ExecutionOutputV1::TimeWithLifecycle(TimeExecutionOutputV2 {
                execution,
                trigger_lifecycle,
            })
        }
        row => row,
    }
}

// Sizing only: the terminal rejection text is longer than the shortened genuine
// rejection diagnostic, so this bounds complete native policy settlement.
fn reserve_policy_shape(row: &mut ExecutionOutputV1, trigger: &TriggerUseV1) -> Result<(), String> {
    use crate::block::execution_output::*;
    let state = TriggerLifecycleStateV1 {
        kind: TriggerLifecycleStateKindV1::Time,
        registered_at_height: Some(u64::MAX - 1),
        action_hash: Some(trigger.action_hash),
        action: TriggerLifecycleActionFrameV1 {
            codec: TriggerLifecycleActionCodecV1::NoritoCanonicalV1,
            bytes: lifecycle_hash_frame(trigger.action_hash)
                .map_err(|error| error.to_string())?
                .to_vec(),
        },
        retry_state: Some(TriggerLifecycleRetryStateV1 {
            attempt: u32::MAX,
            next_time_ms: u64::MAX,
        }),
    };
    let trace = TriggerLifecycleEffectsV1::Complete {
        effects: vec![TriggerLifecycleEffectV1 {
            ordinal: u32::MAX,
            trigger_id: trigger.trigger_id.clone(),
            origin: TriggerLifecycleOriginV1::InvocationPolicy {
                callback_index: u32::MAX,
                policy: TriggerLifecyclePolicyV1::AdvanceTimeRetry,
            },
            change: TriggerLifecycleChangeV1::Update {
                before: state.clone(),
                after: state,
            },
        }],
    };
    match row {
        ExecutionOutputV1::TimeWithLifecycle(row) => row.trigger_lifecycle = trace,
        ExecutionOutputV1::PipelineWithLifecycle(row) => row.trigger_lifecycle = trace,
        _ => unreachable!("internal policy sizing"),
    }
    Ok(())
}
