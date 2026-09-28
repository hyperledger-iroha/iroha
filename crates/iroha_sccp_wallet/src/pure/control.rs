//! Parliament destination controls (spec §4.14.6, §5.1.6, §7.1 steps 2 and 5, §7.4).
//!
//! The minting pause of a destination changes only through control leaves that the Parliament
//! enacts on Taira and anyone applies with `applyControl` once the enacting block is attested.
//! The destination accepts a control only with a nonce above its last applied one, so only the
//! newest control matters. This module holds the two wallet decisions of §7.1 and the local
//! verification of a control proof bundle:
//!
//! - [`check_may_record`] (step 2): refuse to record a transfer while the newest control, taken
//!   over the destination's `controlNonce()`/`mintingPaused()` and Taira's newest control
//!   (recorded or attested), is a pause. A resume that Taira recorded but nobody applied yet
//!   does not refuse, because step 5 applies it before finalizing.
//! - [`plan_before_finalize`] (step 5): apply Taira's newest control first when its nonce is
//!   above the destination's, wait while it is not attested, and never finalize on a paused
//!   destination. Wallet software must not skip this step.
//! - [`verify_control_bundle`]: the attestation, roster, acceptance and signature checks of
//!   [`super::bundle`], the bundle's network and revision equal to the deployment's, the control
//!   leaf computed from the deployment's own immutables and the bundle's nonce and pause flag
//!   (never taken from the bundle), its Merkle path directly or through the history root, and a
//!   nonce above the destination's (a stale control would revert with `StaleControl()`).

use core::fmt;

use iroha_sccp::{
    api::SccpControlProofBundleV1,
    v1::proof::{ControlProofV1, HistoryProofV1, verify_control_direct, verify_control_historical},
};

use super::bundle::{BundleError, DestinationContextV1, VerifiedAttestationV1, verify_attested};

/// The destination's applied control state (`controlNonce()`, `mintingPaused()`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct DestinationControlStateV1 {
    /// Nonce of the last applied control, 0 before the first.
    pub control_nonce: u64,
    /// Whether minting is paused.
    pub minting_paused: bool,
}

/// Taira's newest control for the deployment (`GET /v1/sccp/controls/{network}/{revision}`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TairaControlV1 {
    /// Control nonce.
    pub control_nonce: u64,
    /// Pause (`true`) or resume (`false`).
    pub paused: bool,
    /// Whether the enacting block is attested, so the control can be applied.
    pub attested: bool,
}

/// Where the newest control comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControlSourceV1 {
    /// The destination's own applied state is the newest.
    Destination,
    /// Taira holds a newer control that the destination has not applied.
    Taira,
}

/// The newest control over the destination and Taira (§7.1 step 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NewestControlV1 {
    /// Its nonce.
    pub control_nonce: u64,
    /// Whether it pauses minting.
    pub paused: bool,
    /// Where it comes from.
    pub source: ControlSourceV1,
}

/// The newest of the destination's applied control and Taira's newest control.
#[must_use]
pub fn newest_control(
    destination: &DestinationControlStateV1,
    taira: Option<&TairaControlV1>,
) -> NewestControlV1 {
    match taira {
        Some(taira) if taira.control_nonce > destination.control_nonce => NewestControlV1 {
            control_nonce: taira.control_nonce,
            paused: taira.paused,
            source: ControlSourceV1::Taira,
        },
        _ => NewestControlV1 {
            control_nonce: destination.control_nonce,
            paused: destination.minting_paused,
            source: ControlSourceV1::Destination,
        },
    }
}

/// Why a transfer must not be recorded (§7.1 step 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NewestControlIsPause {
    /// The pausing control.
    pub newest: NewestControlV1,
}

impl fmt::Display for NewestControlIsPause {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "the newest destination control (nonce {}) pauses minting",
            self.newest.control_nonce
        )
    }
}

impl std::error::Error for NewestControlIsPause {}

/// §7.1 step 2: refuse to record a transfer while the newest control is a pause.
///
/// # Errors
///
/// Returns [`NewestControlIsPause`].
pub fn check_may_record(
    destination: &DestinationControlStateV1,
    taira: Option<&TairaControlV1>,
) -> Result<(), NewestControlIsPause> {
    let newest = newest_control(destination, taira);
    if newest.paused {
        Err(NewestControlIsPause { newest })
    } else {
        Ok(())
    }
}

/// What must happen on the destination before a finalization (§7.1 step 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PreFinalizeActionV1 {
    /// The destination is up to date and minting: finalize.
    Finalize,
    /// Apply Taira's newer, attested control first; finalize afterwards only if it resumes.
    ApplyControl {
        /// The control to apply.
        control_nonce: u64,
        /// Whether it pauses minting (then the finalization stops and the message is refunded
        /// through a void after its deadline).
        paused: bool,
    },
    /// Taira's newer control is not attested yet: wait, then apply it.
    AwaitAttestation {
        /// The pending control.
        control_nonce: u64,
        /// Whether it pauses minting.
        paused: bool,
    },
    /// The destination is up to date and paused: do not finalize.
    Paused,
}

/// §7.1 step 5: decide what must happen before `finalizeFromTaira*`.
#[must_use]
pub fn plan_before_finalize(
    destination: &DestinationControlStateV1,
    taira: Option<&TairaControlV1>,
) -> PreFinalizeActionV1 {
    match taira {
        Some(taira) if taira.control_nonce > destination.control_nonce => {
            if taira.attested {
                PreFinalizeActionV1::ApplyControl {
                    control_nonce: taira.control_nonce,
                    paused: taira.paused,
                }
            } else {
                PreFinalizeActionV1::AwaitAttestation {
                    control_nonce: taira.control_nonce,
                    paused: taira.paused,
                }
            }
        }
        _ if destination.minting_paused => PreFinalizeActionV1::Paused,
        _ => PreFinalizeActionV1::Finalize,
    }
}

/// Why a control bundle cannot be applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ControlBundleError {
    /// The bundle names another network or revision than the deployment.
    WrongDeployment,
    /// The attestation, roster or signatures fail (see [`BundleError`]).
    Attestation(BundleError),
    /// The control nonce is not above the destination's `controlNonce()` (the call would
    /// revert with `StaleControl()`).
    Stale,
}

impl fmt::Display for ControlBundleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongDeployment => formatter.write_str(
                "control bundle targets another network or revision than the deployment",
            ),
            Self::Attestation(error) => write!(formatter, "control bundle: {error}"),
            Self::Stale => formatter
                .write_str("the control nonce is not above the destination's applied nonce"),
        }
    }
}

impl std::error::Error for ControlBundleError {}

impl From<BundleError> for ControlBundleError {
    fn from(error: BundleError) -> Self {
        Self::Attestation(error)
    }
}

/// A control bundle verified for one deployment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedControlBundleV1 {
    /// The accepted attestation with trimmed signatures.
    pub attested: VerifiedAttestationV1,
    /// `ControlProofV1` for the calldata.
    pub control: ControlProofV1,
    /// `HistoryProofV1` in historical mode.
    pub history: Option<HistoryProofV1>,
    /// The control leaf computed from the deployment's immutables.
    pub leaf: [u8; 32],
}

impl VerifiedControlBundleV1 {
    /// Whether the control is proven through the history root.
    #[must_use]
    pub fn is_historical(&self) -> bool {
        self.history.is_some()
    }

    /// Whether a finalization may follow once the control is applied (it resumes minting).
    #[must_use]
    pub fn finalize_allowed_after_apply(&self) -> bool {
        !self.control.paused
    }
}

/// Verify a control bundle for the deployment in `context` whose applied control state is
/// `applied` (§5.1.6 steps 2–4).
///
/// # Errors
///
/// Returns the first failing [`ControlBundleError`].
pub fn verify_control_bundle(
    bundle: &SccpControlProofBundleV1,
    context: &DestinationContextV1,
    applied: &DestinationControlStateV1,
) -> Result<VerifiedControlBundleV1, ControlBundleError> {
    bundle.check_shape().map_err(BundleError::Shape)?;
    if bundle.network != context.destination.network
        || bundle.revision != context.destination.route_revision
    {
        return Err(ControlBundleError::WrongDeployment);
    }
    let attested = verify_attested(
        &bundle.statement,
        &bundle.digest,
        &bundle.roster,
        &bundle.signatures,
        context,
    )?;
    let attestation = &attested.signed.attestation;
    let control = bundle.control_proof();
    let history = bundle.history_proof();
    let leaf = match &history {
        None => {
            if bundle.message_count != attestation.message_count {
                return Err(BundleError::MessageCountMismatch.into());
            }
            verify_control_direct(
                attestation,
                &control,
                &context.taira_network_id,
                &context.destination,
            )
            .map_err(BundleError::Proof)?
        }
        Some(history) => {
            if bundle.message_count != history.block.message_count {
                return Err(BundleError::MessageCountMismatch.into());
            }
            verify_control_historical(
                attestation,
                history,
                &control,
                &context.taira_network_id,
                &context.destination,
            )
            .map_err(BundleError::Proof)?
        }
    };
    if control.control_nonce <= applied.control_nonce {
        return Err(ControlBundleError::Stale);
    }
    Ok(VerifiedControlBundleV1 {
        attested,
        control,
        history,
        leaf,
    })
}

#[cfg(test)]
mod tests {
    use iroha_data_model::{bridge::SccpNetworkV1, sccp::attestation::SccpAttestationStatementV1};
    use iroha_sccp::{
        api::{SccpRosterViewV1, SccpSignatureSetV1},
        v1::{
            eip712::AttestationFieldsV1,
            hashes::{control_leaf, keccak256, word_address},
            merkle::PromoteOddTree,
            proof::{DestinationV1, ProofError},
            roster::{RosterStateV1, RosterV1},
            signature::{SignatureSetV1, address_of_secret, sign_digest},
        },
    };

    use super::*;

    const TAIRA: [u8; 32] = [0x11; 32];
    const T0: u64 = 1_800_000_000_000;

    fn secret(index: u8) -> [u8; 32] {
        keccak256(&[b"SCCP/WALLET/CONTROL/TEST", &[index]])
    }

    fn deployment(revision: u32) -> DestinationV1 {
        DestinationV1 {
            network: SccpNetworkV1::BscMainnet,
            route_revision: revision,
            destination_word: word_address(&[0x22; 20]),
        }
    }

    fn fixture() -> (SccpControlProofBundleV1, DestinationContextV1) {
        let mut members: Vec<[u8; 20]> = (0..4)
            .map(|index| address_of_secret(&secret(index)).expect("secret"))
            .collect();
        members.sort_unstable();
        let roster = RosterV1 {
            generation: 1,
            valid_from_ms: T0,
            valid_until_ms: T0 + 86_400_000,
            members,
        };
        let destination = deployment(2);
        let leaf = control_leaf(
            &TAIRA,
            destination.network,
            &destination.destination_word,
            2,
            4,
            true,
        )
        .expect("leaf");
        let tree = PromoteOddTree::block(&[[0x05; 32], leaf]).expect("tree");
        let statement = SccpAttestationStatementV1 {
            height: 9,
            epoch: 0,
            timestamp_ms: T0 + 500,
            block_hash: [0x99; 32],
            sccp_root: tree.root(),
            message_count: 2,
            history_root: [0x88; 32],
            history_size: 4,
            roster_digest: roster.digest(&TAIRA).expect("digest"),
            next_roster_digest: [0; 32],
        };
        let digest = AttestationFieldsV1::from(statement).digest(&TAIRA);
        let entries: Vec<(usize, [u8; 65])> = (0..3)
            .map(|key| {
                let address = address_of_secret(&secret(key)).expect("secret");
                let index = roster
                    .members
                    .iter()
                    .position(|member| *member == address)
                    .expect("member");
                (index, sign_digest(&secret(key), &digest).expect("sign"))
            })
            .collect();
        let set = SignatureSetV1::from_signers(roster.n(), &entries).expect("set");
        let bundle = SccpControlProofBundleV1 {
            network: destination.network,
            revision: 2,
            control_nonce: 4,
            paused: true,
            leaf_index: 1,
            message_count: 2,
            path: tree.path(1).expect("path"),
            statement,
            digest,
            roster: SccpRosterViewV1::from_roster(&roster, &TAIRA).expect("view"),
            signatures: SccpSignatureSetV1::try_from(&set).expect("view"),
            history: None,
        };
        let context = DestinationContextV1 {
            taira_network_id: TAIRA,
            destination,
            roster_state: RosterStateV1::initial(&roster, &TAIRA, T0).expect("state"),
            now_ms: T0 + 1_000,
        };
        (bundle, context)
    }

    #[test]
    fn newest_control_prefers_the_higher_nonce() {
        let destination = DestinationControlStateV1 {
            control_nonce: 3,
            minting_paused: false,
        };
        let newer = TairaControlV1 {
            control_nonce: 4,
            paused: true,
            attested: false,
        };
        let older = TairaControlV1 {
            control_nonce: 3,
            paused: true,
            attested: true,
        };
        assert_eq!(
            newest_control(&destination, Some(&newer)),
            NewestControlV1 {
                control_nonce: 4,
                paused: true,
                source: ControlSourceV1::Taira
            }
        );
        assert_eq!(
            newest_control(&destination, Some(&older)).source,
            ControlSourceV1::Destination
        );
        assert_eq!(
            newest_control(&destination, None).source,
            ControlSourceV1::Destination
        );
    }

    #[test]
    fn recording_is_refused_only_behind_a_newest_pause() {
        let running = DestinationControlStateV1 {
            control_nonce: 1,
            minting_paused: false,
        };
        let paused = DestinationControlStateV1 {
            control_nonce: 1,
            minting_paused: true,
        };
        let pending_pause = TairaControlV1 {
            control_nonce: 2,
            paused: true,
            attested: false,
        };
        let pending_resume = TairaControlV1 {
            control_nonce: 2,
            paused: false,
            attested: false,
        };
        assert!(check_may_record(&running, None).is_ok());
        assert!(check_may_record(&running, Some(&pending_pause)).is_err());
        assert!(check_may_record(&paused, None).is_err());
        assert!(check_may_record(&paused, Some(&pending_resume)).is_ok());
        let error = check_may_record(&paused, None).expect_err("paused");
        assert!(error.to_string().contains("nonce 1"));
    }

    #[test]
    fn finalization_applies_newer_controls_first() {
        let running = DestinationControlStateV1 {
            control_nonce: 1,
            minting_paused: false,
        };
        let paused = DestinationControlStateV1 {
            control_nonce: 1,
            minting_paused: true,
        };
        let attested_resume = TairaControlV1 {
            control_nonce: 3,
            paused: false,
            attested: true,
        };
        let unattested_pause = TairaControlV1 {
            control_nonce: 3,
            paused: true,
            attested: false,
        };
        let stale = TairaControlV1 {
            control_nonce: 1,
            paused: false,
            attested: true,
        };
        assert_eq!(
            plan_before_finalize(&running, None),
            PreFinalizeActionV1::Finalize
        );
        assert_eq!(
            plan_before_finalize(&paused, None),
            PreFinalizeActionV1::Paused
        );
        assert_eq!(
            plan_before_finalize(&paused, Some(&attested_resume)),
            PreFinalizeActionV1::ApplyControl {
                control_nonce: 3,
                paused: false
            }
        );
        assert_eq!(
            plan_before_finalize(&running, Some(&unattested_pause)),
            PreFinalizeActionV1::AwaitAttestation {
                control_nonce: 3,
                paused: true
            }
        );
        assert_eq!(
            plan_before_finalize(&paused, Some(&stale)),
            PreFinalizeActionV1::Paused
        );
    }

    #[test]
    fn control_bundles_verify_for_their_own_deployment_only() {
        let (bundle, context) = fixture();
        let applied = DestinationControlStateV1::default();
        let verified = verify_control_bundle(&bundle, &context, &applied).expect("verifies");
        assert!(!verified.is_historical());
        assert!(!verified.finalize_allowed_after_apply());
        assert_eq!(verified.control.control_nonce, 4);
        assert_eq!(verified.attested.attested().signatures.popcount(), 3);

        let mut other_revision = bundle.clone();
        other_revision.revision = 3;
        assert_eq!(
            verify_control_bundle(&other_revision, &context, &applied),
            Err(ControlBundleError::WrongDeployment)
        );
        let foreign = DestinationContextV1 {
            destination: deployment(3),
            ..context
        };
        let mut relabeled = bundle.clone();
        relabeled.revision = 3;
        assert_eq!(
            verify_control_bundle(&relabeled, &foreign, &applied),
            Err(ControlBundleError::Attestation(BundleError::Proof(
                ProofError::BadBlockPath
            )))
        );
        let mut flipped = bundle.clone();
        flipped.paused = false;
        assert_eq!(
            verify_control_bundle(&flipped, &context, &applied),
            Err(ControlBundleError::Attestation(BundleError::Proof(
                ProofError::BadBlockPath
            )))
        );
        let stale = DestinationControlStateV1 {
            control_nonce: 4,
            minting_paused: false,
        };
        assert_eq!(
            verify_control_bundle(&bundle, &context, &stale),
            Err(ControlBundleError::Stale)
        );
        let mut wrong_count = bundle;
        wrong_count.message_count = 3;
        assert_eq!(
            verify_control_bundle(&wrong_count, &context, &applied),
            Err(ControlBundleError::Attestation(
                BundleError::MessageCountMismatch
            ))
        );
        assert!(
            ControlBundleError::WrongDeployment
                .to_string()
                .contains("revision")
        );
    }
}
