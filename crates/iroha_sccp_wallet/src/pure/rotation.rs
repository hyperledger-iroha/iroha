//! Roster rotation chain verification (spec §4.3.3, §5.1.5, §7.1 steps 2 and 5, §7.4).
//!
//! A destination that lags behind Taira replays every generation handoff in order through
//! `rotateRosters` (up to 16 rotations per call). Before paying for those calls the wallet walks
//! the chain served by `GET /v1/sccp/rosters/rotations` against a simulated copy of the
//! destination's roster light client and checks every bound the contract enforces for each
//! step:
//!
//! 1. the statement's invariants and its EIP-712 digest under the Taira `NetworkId`;
//! 2. the statement is signed by the destination's **current** roster, which is unexpired at
//!    `now_ms`, and the supplied current roster hashes to that digest;
//! 3. at least `t` valid signatures of the current roster;
//! 4. `nextRosterDigest ≠ 0` equals the recomputed digest of the next roster (with the `n`,
//!    `t` and ordering checks), `next.generation = generation + 1`,
//!    `next.validFromMs = A.timestampMs`, and the validity bounds of §5.1.5 step 3;
//! 5. the state transition of step 4: the old roster becomes the previous one with
//!    `prevValidUntilMs = min(validUntilMs, now_ms + 24 h)`.
//!
//! The result is the verified list of `RotationV1` arguments (signatures trimmed to exactly
//! `t`), split into `rotateRosters` batches, and the destination state after all of them. A
//! chain that stops before the target generation is refused, naming the first unattested
//! handoff when Torii reports one (§4.3.3). [`check_roster_horizon`] is the §7.1 step 2 guard
//! against recording a transfer whose destination roster would expire before it can be minted.
//!
//! All rotations of one plan are checked at the same `now_ms`; a plan of more than 16 rotations
//! spans several transactions, which the caller submits in order and re-verifies if their
//! block times move past a validity bound.

use core::fmt;

use iroha_sccp::{
    api::{SccpRotationChainV1, SccpRotationStepV1},
    v1::{
        constants::MAX_ROTATIONS_PER_CALL,
        evm_abi::RotationV1,
        roster::{RosterStateV1, RotationError},
    },
};

use super::bundle::{BundleError, verify_signed};

/// Why a rotation chain cannot bring the destination to the target generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RotationChainError {
    /// Step `index` fails the statement, roster or signature checks.
    Step {
        /// Zero-based step index.
        index: usize,
        /// The failed check.
        error: BundleError,
    },
    /// Step `index` breaks a §5.1.5 rotation bound.
    Rotation {
        /// Zero-based step index.
        index: usize,
        /// The violated bound.
        error: RotationError,
    },
    /// Step `index` is not a rotation statement (`nextRosterDigest = 0`).
    NotARotation {
        /// Zero-based step index.
        index: usize,
    },
    /// The chain ends before the target generation at a handoff Taira has not attested yet.
    UnattestedHandoff {
        /// Rotation height of the unattested handoff.
        height: u64,
        /// Generation the chain reaches.
        reached: u64,
        /// Generation the caller needs.
        target: u64,
    },
    /// The chain ends before the target generation.
    ChainTooShort {
        /// Generation the chain reaches.
        reached: u64,
        /// Generation the caller needs.
        target: u64,
    },
}

impl fmt::Display for RotationChainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Step { index, error } => write!(formatter, "rotation step {index}: {error}"),
            Self::Rotation { index, error } => {
                write!(formatter, "rotation step {index}: {error}")
            }
            Self::NotARotation { index } => {
                write!(
                    formatter,
                    "rotation step {index} is not a rotation statement"
                )
            }
            Self::UnattestedHandoff {
                height,
                reached,
                target,
            } => write!(
                formatter,
                "the handoff at height {height} is not attested yet; the chain reaches generation \
                 {reached} of {target}"
            ),
            Self::ChainTooShort { reached, target } => write!(
                formatter,
                "the rotation chain reaches generation {reached}, not {target}"
            ),
        }
    }
}

impl std::error::Error for RotationChainError {}

/// A verified sequence of rotations and the destination state after them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RotationPlanV1 {
    /// Verified `RotationV1` arguments in application order.
    pub rotations: Vec<RotationV1>,
    /// The destination roster state after every rotation is applied.
    pub final_state: RosterStateV1,
}

impl RotationPlanV1 {
    /// Whether nothing needs to be rotated.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rotations.is_empty()
    }

    /// The rotations grouped into `rotateRosters` calls of at most 16, in order.
    pub fn batches(&self) -> impl Iterator<Item = &[RotationV1]> {
        self.rotations.chunks(MAX_ROTATIONS_PER_CALL)
    }
}

/// Verify one step against `state` and advance `state` on success.
fn apply_step(
    index: usize,
    step: &SccpRotationStepV1,
    state: &mut RosterStateV1,
    taira_network_id: &[u8; 32],
    now_ms: u64,
) -> Result<RotationV1, RotationChainError> {
    let step_error = |error: BundleError| RotationChainError::Step { index, error };
    let rotation_error = |error: RotationError| RotationChainError::Rotation { index, error };
    step.check_shape()
        .map_err(|error| step_error(BundleError::Shape(error)))?;
    if step.statement.next_roster_digest == [0; 32] {
        return Err(RotationChainError::NotARotation { index });
    }
    // The statement must name the destination's current, unexpired roster before any
    // signature is recovered (§5.1.5 step 1).
    if step.statement.roster_digest != state.digest || now_ms > state.valid_until_ms {
        return Err(rotation_error(RotationError::RosterNotAccepted));
    }
    let signed = verify_signed(
        &step.statement,
        &step.digest,
        &step.current_roster,
        &step.signatures,
        taira_network_id,
    )
    .map_err(step_error)?;
    let next = step
        .next_roster
        .to_roster()
        .map_err(|_| rotation_error(RotationError::BadNextRoster))?;
    let next_digest = state
        .check_rotation(
            &signed.attestation,
            &signed.roster,
            &next,
            taira_network_id,
            now_ms,
        )
        .map_err(rotation_error)?;
    if next_digest != step.next_roster.digest {
        return Err(step_error(BundleError::RosterDigestMismatch));
    }
    state.apply_rotation(next_digest, &next, now_ms);
    Ok(RotationV1 {
        attestation: signed.attestation,
        current: signed.roster,
        signatures: signed.signatures,
        next,
    })
}

/// Verify `steps` from the destination `state` up to `target_generation` at `now_ms`.
///
/// Steps past the target are ignored; a destination already at or past the target yields an
/// empty plan (an attestation of the previous generation is accepted within its grace).
///
/// # Errors
///
/// Returns the first failing [`RotationChainError`]; a chain that stops before the target is
/// [`RotationChainError::UnattestedHandoff`] when `first_unattested_handoff` is known, else
/// [`RotationChainError::ChainTooShort`].
pub fn verify_rotation_steps(
    steps: &[SccpRotationStepV1],
    first_unattested_handoff: Option<u64>,
    state: &RosterStateV1,
    target_generation: u64,
    taira_network_id: &[u8; 32],
    now_ms: u64,
) -> Result<RotationPlanV1, RotationChainError> {
    let mut current = *state;
    let mut rotations = Vec::new();
    for (index, step) in steps.iter().enumerate() {
        if current.generation >= target_generation {
            break;
        }
        rotations.push(apply_step(
            index,
            step,
            &mut current,
            taira_network_id,
            now_ms,
        )?);
    }
    if current.generation < target_generation {
        let reached = current.generation;
        return Err(first_unattested_handoff.map_or(
            RotationChainError::ChainTooShort {
                reached,
                target: target_generation,
            },
            |height| RotationChainError::UnattestedHandoff {
                height,
                reached,
                target: target_generation,
            },
        ));
    }
    Ok(RotationPlanV1 {
        rotations,
        final_state: current,
    })
}

/// [`verify_rotation_steps`] over one `GET /v1/sccp/rosters/rotations` page.
///
/// # Errors
///
/// See [`verify_rotation_steps`].
pub fn verify_rotation_chain(
    chain: &SccpRotationChainV1,
    state: &RosterStateV1,
    target_generation: u64,
    taira_network_id: &[u8; 32],
    now_ms: u64,
) -> Result<RotationPlanV1, RotationChainError> {
    verify_rotation_steps(
        &chain.steps,
        chain.first_unattested_handoff,
        state,
        target_generation,
        taira_network_id,
        now_ms,
    )
}

/// Why a transfer must not be recorded for a destination (§7.1 step 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RosterHorizonError {
    /// The destination's current roster has already expired: it is frozen for minting.
    Frozen,
    /// The current roster expires within `outbound_ttl_ms + roster_max_age_ms` and no rotation
    /// to a later generation is available.
    ExpiresTooSoon {
        /// The destination's `validUntilMs`.
        valid_until_ms: u64,
        /// The latest acceptable expiry without a rotation.
        required_until_ms: u64,
    },
}

impl fmt::Display for RosterHorizonError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Frozen => formatter.write_str("the destination roster has expired"),
            Self::ExpiresTooSoon {
                valid_until_ms,
                required_until_ms,
            } => write!(
                formatter,
                "the destination roster expires at {valid_until_ms} ms, before {required_until_ms} \
                 ms, and no rotation is available"
            ),
        }
    }
}

impl std::error::Error for RosterHorizonError {}

/// §7.1 step 2: refuse a new transfer when the destination's current roster is expired, or
/// expires within `outbound_ttl_ms + roster_max_age_ms` while no verified rotation to a later
/// generation is available.
///
/// # Errors
///
/// Returns the violated [`RosterHorizonError`].
pub fn check_roster_horizon(
    state: &RosterStateV1,
    now_ms: u64,
    outbound_ttl_ms: u64,
    roster_max_age_ms: u64,
    rotation_available: bool,
) -> Result<(), RosterHorizonError> {
    if now_ms > state.valid_until_ms {
        return Err(RosterHorizonError::Frozen);
    }
    let required_until_ms = now_ms
        .saturating_add(outbound_ttl_ms)
        .saturating_add(roster_max_age_ms);
    if state.valid_until_ms < required_until_ms && !rotation_available {
        return Err(RosterHorizonError::ExpiresTooSoon {
            valid_until_ms: state.valid_until_ms,
            required_until_ms,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use iroha_data_model::sccp::attestation::SccpAttestationStatementV1;
    use iroha_sccp::{
        api::{SccpRosterViewV1, SccpSignatureSetV1},
        v1::{
            eip712::AttestationFieldsV1,
            hashes::keccak256,
            roster::RosterV1,
            signature::{SignatureSetV1, address_of_secret, sign_digest},
        },
    };

    use super::*;

    const TAIRA: [u8; 32] = [0x11; 32];
    const T0: u64 = 1_800_000_000_000;
    const DAY: u64 = 86_400_000;

    fn secret(index: u8) -> [u8; 32] {
        keccak256(&[b"SCCP/WALLET/ROTATION/TEST", &[index]])
    }

    fn roster(generation: u64, valid_from_ms: u64) -> RosterV1 {
        let mut members: Vec<[u8; 20]> = (0..4)
            .map(|index| address_of_secret(&secret(index)).expect("secret"))
            .collect();
        members.sort_unstable();
        RosterV1 {
            generation,
            valid_from_ms,
            valid_until_ms: valid_from_ms + 14 * DAY,
            members,
        }
    }

    fn step(current: &RosterV1, next: &RosterV1, height: u64) -> SccpRotationStepV1 {
        let statement = SccpAttestationStatementV1 {
            height,
            epoch: 1,
            timestamp_ms: next.valid_from_ms,
            block_hash: [0xab; 32],
            sccp_root: [0; 32],
            message_count: 0,
            history_root: [0; 32],
            history_size: 0,
            roster_digest: current.digest(&TAIRA).expect("digest"),
            next_roster_digest: next.digest(&TAIRA).expect("digest"),
        };
        let digest = AttestationFieldsV1::from(statement).digest(&TAIRA);
        let entries: Vec<(usize, [u8; 65])> = (0..3)
            .map(|key| {
                let address = address_of_secret(&secret(key)).expect("secret");
                let index = current
                    .members
                    .iter()
                    .position(|member| *member == address)
                    .expect("member");
                (index, sign_digest(&secret(key), &digest).expect("sign"))
            })
            .collect();
        let set = SignatureSetV1::from_signers(current.n(), &entries).expect("set");
        SccpRotationStepV1 {
            statement,
            digest,
            signatures: SccpSignatureSetV1::try_from(&set).expect("view"),
            current_roster: SccpRosterViewV1::from_roster(current, &TAIRA).expect("view"),
            next_roster: SccpRosterViewV1::from_roster(next, &TAIRA).expect("view"),
        }
    }

    fn chain(generations: u64) -> (Vec<SccpRotationStepV1>, RosterStateV1) {
        let first = roster(1, T0);
        let state = RosterStateV1::initial(&first, &TAIRA, T0).expect("state");
        let mut current = first;
        let mut steps = Vec::new();
        for offset in 1..=generations {
            let next = roster(current.generation + 1, T0 + offset * 1_000);
            steps.push(step(&current, &next, 100 + offset));
            current = next;
        }
        (steps, state)
    }

    #[test]
    fn a_long_chain_verifies_and_splits_into_batches_of_16() {
        let (steps, state) = chain(17);
        let now = T0 + 60_000;
        let plan = verify_rotation_steps(&steps, None, &state, 18, &TAIRA, now).expect("plan");
        assert_eq!(plan.rotations.len(), 17);
        assert_eq!(plan.final_state.generation, 18);
        // The last outgoing roster keeps the 24 h grace from `now`, not its own expiry.
        assert_eq!(plan.final_state.prev_valid_until_ms, now + DAY);
        let batches: Vec<usize> = plan.batches().map(<[RotationV1]>::len).collect();
        assert_eq!(batches, vec![16, 1]);
        assert!(plan.rotations.iter().all(|r| r.signatures.popcount() == 3));
        assert!(!plan.is_empty());
    }

    #[test]
    fn the_chain_stops_at_the_target_and_reports_breaks() {
        let (steps, state) = chain(3);
        let now = T0 + 60_000;
        let plan = verify_rotation_steps(&steps, None, &state, 2, &TAIRA, now).expect("plan");
        assert_eq!(plan.rotations.len(), 1);
        let empty = verify_rotation_steps(&steps, None, &state, 1, &TAIRA, now).expect("empty");
        assert!(empty.is_empty());
        assert_eq!(empty.final_state, state);
        assert_eq!(
            verify_rotation_steps(&steps, Some(777), &state, 6, &TAIRA, now),
            Err(RotationChainError::UnattestedHandoff {
                height: 777,
                reached: 4,
                target: 6
            })
        );
        assert_eq!(
            verify_rotation_steps(&steps[..1], None, &state, 3, &TAIRA, now),
            Err(RotationChainError::ChainTooShort {
                reached: 2,
                target: 3
            })
        );
        let page = SccpRotationChainV1 {
            steps,
            first_unattested_handoff: None,
        };
        assert_eq!(
            verify_rotation_chain(&page, &state, 4, &TAIRA, now)
                .expect("page")
                .final_state
                .generation,
            4
        );
    }

    #[test]
    fn rotation_bounds_are_enforced() {
        let (steps, state) = chain(2);
        let now = T0 + 60_000;
        // Out of order: the second step first.
        assert_eq!(
            verify_rotation_steps(&steps[1..], None, &state, 3, &TAIRA, now),
            Err(RotationChainError::Rotation {
                index: 0,
                error: RotationError::RosterNotAccepted
            })
        );
        // Expired current roster.
        assert_eq!(
            verify_rotation_steps(&steps, None, &state, 3, &TAIRA, T0 + 14 * DAY + 1),
            Err(RotationChainError::Rotation {
                index: 0,
                error: RotationError::RosterNotAccepted
            })
        );
        // A generation gap.
        let first = roster(1, T0);
        let skip = step(&first, &roster(3, T0 + 1_000), 101);
        assert_eq!(
            verify_rotation_steps(&[skip], None, &state, 3, &TAIRA, now),
            Err(RotationChainError::Rotation {
                index: 0,
                error: RotationError::GenerationNotSequential
            })
        );
        // Validity beyond MAX_ROSTER_VALIDITY_MS.
        let mut long = roster(2, T0 + 1_000);
        long.valid_until_ms = long.valid_from_ms + 31 * DAY;
        assert_eq!(
            verify_rotation_steps(&[step(&first, &long, 101)], None, &state, 2, &TAIRA, now),
            Err(RotationChainError::Rotation {
                index: 0,
                error: RotationError::BadValidity
            })
        );
        // A statement without a successor.
        let mut plain = steps[0].clone();
        plain.statement.next_roster_digest = [0; 32];
        assert_eq!(
            verify_rotation_steps(&[plain], None, &state, 2, &TAIRA, now),
            Err(RotationChainError::NotARotation { index: 0 })
        );
        // A tampered signature.
        let mut forged = steps[0].clone();
        forged.signatures.signatures[0][10] ^= 1;
        assert!(matches!(
            verify_rotation_steps(&[forged], None, &state, 2, &TAIRA, now),
            Err(RotationChainError::Step {
                index: 0,
                error: BundleError::Signatures(_)
            })
        ));
        // A next roster whose claimed digest differs.
        let mut claimed = steps[0].clone();
        claimed.next_roster.digest[0] ^= 1;
        assert_eq!(
            verify_rotation_steps(&[claimed], None, &state, 2, &TAIRA, now),
            Err(RotationChainError::Step {
                index: 0,
                error: BundleError::RosterDigestMismatch
            })
        );
    }

    #[test]
    fn roster_horizon_guard() {
        let state = RosterStateV1 {
            digest: [1; 32],
            generation: 5,
            valid_until_ms: 1_000,
            prev_digest: [0; 32],
            prev_valid_until_ms: 0,
        };
        assert_eq!(check_roster_horizon(&state, 100, 400, 500, false), Ok(()));
        assert_eq!(
            check_roster_horizon(&state, 101, 400, 500, false),
            Err(RosterHorizonError::ExpiresTooSoon {
                valid_until_ms: 1_000,
                required_until_ms: 1_001
            })
        );
        assert_eq!(check_roster_horizon(&state, 101, 400, 500, true), Ok(()));
        assert_eq!(
            check_roster_horizon(&state, 1_001, 0, 0, true),
            Err(RosterHorizonError::Frozen)
        );
        assert!(RosterHorizonError::Frozen.to_string().contains("expired"));
        assert!(
            RotationChainError::ChainTooShort {
                reached: 1,
                target: 2
            }
            .to_string()
            .contains("generation 1")
        );
    }
}
