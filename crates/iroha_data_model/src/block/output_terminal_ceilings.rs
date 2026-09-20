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
        let trigger = TriggerUseV1 {
            trigger_id: "x"
                .repeat(MAX_NAME_BYTES)
                .parse::<crate::trigger::TriggerId>()
                .map_err(|error| format!("cannot construct maximum terminal Name: {error}"))?,
            registered_at_height: u64::MAX - 1,
            action_hash: Hash::prehashed([0xff; Hash::LENGTH]),
        };
        let network = output_bytes(&ExecutionOutputV1::network_output_limit_rejection(u32::MAX))?;
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
            pipeline = pipeline.max(output_bytes(&row)?);
        }
        let time = output_bytes(&ExecutionOutputV1::time_output_limit_rejection(
            TimeInvocationV1 {
                schedule_index: u32::MAX,
                event: TimeEvent {
                    interval: TimeInterval {
                        since_ms: u64::MAX - 1,
                        length_ms: 1,
                    },
                },
                trigger,
            },
        ))?;
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
