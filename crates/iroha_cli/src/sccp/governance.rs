//! `iroha sccp governance`: SCCP proposals to the SORA Parliament (`specs/sccp.md` §4.14.3).
//!
//! The Parliament is the only SCCP governance authority. `propose` builds an
//! `SccpGovernanceProposalV1` whose `base_revisions` are Taira's current per-subject revisions,
//! runs the static checks and submits `ProposeSccpRouteGovernance`; attempts and lifecycle
//! transitions are permissionless (`iroha gov parliament`).
//!
//! `drive` is the Parliament driver of §4.14.5 item 4. It has no discretion: it creates the
//! first attempt of every admissible open proposal, oldest first, and submits whatever Core's
//! attempt plan (`GET /v1/gov/parliament/attempts/{id}/plan`) lists: due transitions in one
//! transaction, each exact-height checkpoint in its own transaction timed by the `QueuePlan` lag,
//! corpus relays from published records and combined TLE releases. It ticks an idle tip so block
//! windows elapse. Several drivers are harmless: a duplicate transition fails and pays its fee.
//!
//! TODO(ws42): `show` (body, still-current flag, deployment checks).

use std::{
    collections::BTreeMap,
    path::PathBuf,
    time::{Duration, Instant},
};

use eyre::{Result, WrapErr as _, eyre};
use iroha::{
    client::Client,
    data_model::{
        NetworkId,
        governance::types::{
            BallotAttemptId, GovernanceAttemptId, GovernanceAttemptStatusV1, ProposalKind,
            SccpRouteGovernanceProposal,
        },
        isi::{
            InstructionBox, Log,
            governance::{
                CreateParliamentGovernanceAttemptV1, ParliamentLifecycleTransitionV1,
                ProposeSccpRouteGovernance, SubmitParliamentLifecycleTransitionV1,
            },
        },
        sccp::governance::{
            SccpGovernanceActionV1, SccpGovernanceBaseRevisionV1, SccpGovernanceProposalV1,
        },
    },
};
use iroha_sccp::api::SccpGovernanceProposalStatusV1;
use iroha_torii_shared::parliament_api::ParliamentAttemptPlanResponseV1;
use url::Url;

use crate::{Run, RunContext};

/// `iroha sccp governance` subcommands.
#[derive(clap::Subcommand, Debug)]
pub enum Command {
    /// Propose SCCP actions to the Parliament against Taira's current revisions.
    Propose(ProposeArgs),
    /// Show Taira's nonzero per-subject governance revisions.
    Revisions,
    /// Run the Parliament driver: carry SCCP proposals through their Parliament attempts.
    Drive(DriveArgs),
}

/// Arguments of `iroha sccp governance drive`.
#[derive(clap::Args, Debug)]
pub struct DriveArgs {
    /// Torii root of one TLE signer peer, for combining ballot releases (repeatable; supply
    /// every validator).
    #[arg(
        long = "release-peer",
        value_name = "TORII_URL",
        value_parser = crate::gov::parliament::parse_release_peer_url
    )]
    pub release_peers: Vec<Url>,
    /// Directory of published masked-ballot record files to relay.
    #[arg(long)]
    pub relay_dir: Option<PathBuf>,
    /// Milliseconds between polls.
    #[arg(long, default_value_t = 500)]
    pub poll_interval_ms: u64,
    /// Milliseconds without a new block after which the driver ticks while work is pending.
    #[arg(long, default_value_t = 4_000)]
    pub tick_interval_ms: u64,
    /// Stop after this many polls; 0 runs until interrupted.
    #[arg(long, default_value_t = 0)]
    pub polls: u64,
}

/// One driver submission.
#[derive(Clone, Debug, PartialEq, Eq)]
enum DriveStep {
    /// Create the first attempt of an admissible proposal.
    CreateAttempt(ProposalKind),
    /// Transitions due at the execution height, in one transaction.
    Due(GovernanceAttemptId, Vec<ParliamentLifecycleTransitionV1>),
    /// One exact-height transition in its own transaction, aimed at `execution_height`.
    Exact {
        attempt: GovernanceAttemptId,
        execution_height: u64,
        transition: ParliamentLifecycleTransitionV1,
    },
    /// Relay published masked-ballot records.
    Relay(BallotAttemptId),
    /// Combine the TLE release and finalize the opened ballot.
    Finalize(BallotAttemptId),
}

impl DriveStep {
    /// Key under which a submission is remembered until its execution height has passed.
    fn key(&self) -> String {
        match self {
            Self::CreateAttempt(kind) => {
                format!("create:{}", hex::encode(kind.fingerprint()))
            }
            Self::Due(attempt, transitions) => format!(
                "due:{}:{}",
                attempt.to_hex(),
                transitions
                    .iter()
                    .map(|transition| hex::encode(transition.digest_v1()))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
            Self::Exact {
                attempt,
                execution_height,
                transition,
            } => format!(
                "exact:{}:{execution_height}:{}",
                attempt.to_hex(),
                hex::encode(transition.digest_v1())
            ),
            Self::Relay(ballot) => format!("relay:{}", ballot.to_hex()),
            Self::Finalize(ballot) => format!("finalize:{}", ballot.to_hex()),
        }
    }
}

/// Admissible open proposals without an attempt, oldest first (the listing's order).
fn proposals_to_attempt(proposals: &[SccpGovernanceProposalStatusV1]) -> Vec<ProposalKind> {
    proposals
        .iter()
        .filter(|proposal| proposal.admissible && proposal.latest_attempt.is_none())
        .map(|proposal| {
            ProposalKind::SccpRouteGovernance(SccpRouteGovernanceProposal {
                proposal: Box::new(proposal.proposal.clone()),
            })
        })
        .collect()
}

/// Whether an open proposal still needs blocks: it can get an attempt or its attempt is live.
fn needs_blocks(proposal: &SccpGovernanceProposalStatusV1) -> bool {
    match proposal.latest_attempt {
        None => proposal.admissible,
        Some(attempt) => matches!(
            attempt.status,
            GovernanceAttemptStatusV1::Active | GovernanceAttemptStatusV1::Certified
        ),
    }
}

/// Steps of one attempt plan. A transaction sent now executes at `execution_height`; an exact
/// transition one block later is sent now as well, so a transaction admitted one block late
/// still lands on its height, and the early copy fails harmlessly.
fn plan_steps(plan: &ParliamentAttemptPlanResponseV1) -> Vec<DriveStep> {
    let attempt = plan.governance_attempt_id;
    let mut steps = Vec::new();
    if !plan.due.is_empty() {
        steps.push(DriveStep::Due(attempt, plan.due.clone()));
    }
    for exact in &plan.exact {
        if exact.height == plan.execution_height
            || Some(exact.height) == plan.execution_height.checked_add(1)
        {
            steps.push(DriveStep::Exact {
                attempt,
                execution_height: plan.execution_height,
                transition: exact.transition.clone(),
            });
        }
    }
    steps.extend(plan.relay_ballots.iter().copied().map(DriveStep::Relay));
    steps.extend(
        plan.finalize_ballots
            .iter()
            .copied()
            .map(DriveStep::Finalize),
    );
    steps
}

/// The instructions of `step`'s transaction, or `None` when this driver lacks the off-chain
/// input (published ballot records or signer peers).
fn step_instructions(
    parliament: &Client,
    args: &DriveArgs,
    step: &DriveStep,
) -> Result<Option<Vec<InstructionBox>>> {
    let submit = |attempt: GovernanceAttemptId, transition: &ParliamentLifecycleTransitionV1| {
        InstructionBox::from(SubmitParliamentLifecycleTransitionV1 {
            governance_attempt_id: attempt,
            transition: transition.clone(),
        })
    };
    Ok(Some(match step {
        DriveStep::CreateAttempt(kind) => {
            vec![InstructionBox::from(CreateParliamentGovernanceAttemptV1 {
                proposal: kind.clone(),
                attempt_sequence: 0,
            })]
        }
        DriveStep::Due(attempt, transitions) => transitions
            .iter()
            .map(|transition| submit(*attempt, transition))
            .collect(),
        DriveStep::Exact {
            attempt,
            transition,
            ..
        } => vec![submit(*attempt, transition)],
        DriveStep::Relay(ballot) => {
            let Some(dir) = &args.relay_dir else {
                return Ok(None);
            };
            let records = std::fs::read_dir(dir)
                .wrap_err_with(|| format!("read {}", dir.display()))?
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .filter(|path| path.is_file())
                .collect::<Vec<_>>();
            vec![
                crate::gov::parliament::ballot::relay_instruction(parliament, *ballot, &records)?.2,
            ]
        }
        DriveStep::Finalize(ballot) => {
            if args.release_peers.is_empty() {
                return Ok(None);
            }
            vec![InstructionBox::from(
                crate::gov::parliament::finalize_opened_ballot_instruction(
                    parliament,
                    *ballot,
                    args.release_peers.clone(),
                )?,
            )]
        }
    }))
}

/// Run the Parliament driver until `args.polls` polls (0: until interrupted).
fn drive<C: RunContext>(context: &mut C, args: &DriveArgs) -> Result<()> {
    use iroha_core::governance::parliament::PARLIAMENT_DRIVER_EXECUTION_LAG_BLOCKS as LAG;
    let client = super::blocking(context)?;
    let parliament: Client = context.client_from_config()?;
    let poll_interval = Duration::from_millis(args.poll_interval_ms.max(100));
    let tick_interval = Duration::from_millis(args.tick_interval_ms.max(1_000));
    // Submissions by key, with the tip they were sent at; a step is resent only after the
    // height it aimed at has passed without taking effect.
    let mut sent: BTreeMap<String, u64> = BTreeMap::new();
    let mut last_block: Option<(u64, Instant)> = None;
    let mut polls = 0_u64;
    loop {
        polls = polls.saturating_add(1);
        let tip = client.status().get()?.blocks;
        if last_block.is_none_or(|(height, _)| height != tip) {
            last_block = Some((tip, Instant::now()));
        }
        sent.retain(|_, at| at.saturating_add(LAG).saturating_add(1) > tip);
        let proposals = client.sccp().governance_proposals()?;
        let pending = proposals.iter().any(needs_blocks);
        let mut steps: Vec<_> = proposals_to_attempt(&proposals)
            .into_iter()
            .map(DriveStep::CreateAttempt)
            .collect();
        for attempt in proposals
            .iter()
            .filter_map(|proposal| proposal.latest_attempt)
        {
            if attempt.status != GovernanceAttemptStatusV1::Active {
                continue;
            }
            match parliament.get_parliament_attempt_plan(attempt.id) {
                Ok(plan) => steps.extend(plan_steps(&plan)),
                Err(error) => context.println(format_args!(
                    "plan of attempt {}: {error:#}",
                    attempt.id.to_hex()
                ))?,
            }
        }
        for step in steps {
            let key = step.key();
            if sent.contains_key(&key) {
                continue;
            }
            sent.insert(key.clone(), tip);
            match step_instructions(&parliament, args, &step) {
                Ok(Some(instructions)) => {
                    context.println(format_args!("tip {tip}: {key}"))?;
                    if let Err(error) = context.finish_unconfirmed(instructions) {
                        context.println(format_args!("{key}: {error:#}"))?;
                    }
                }
                Ok(None) => context.println(format_args!(
                    "{key}: waiting for its off-chain input (--relay-dir or --release-peer)"
                ))?,
                Err(error) => context.println(format_args!("{key}: {error:#}"))?,
            }
        }
        if pending && last_block.is_some_and(|(_, since)| since.elapsed() >= tick_interval) {
            context.println(format_args!("tip {tip}: tick"))?;
            if let Err(error) = context.finish_unconfirmed(vec![InstructionBox::from(Log::new(
                iroha::data_model::Level::INFO,
                "SCCP Parliament driver tick".to_owned(),
            ))]) {
                context.println(format_args!("tick: {error:#}"))?;
            }
            last_block = Some((tip, Instant::now()));
        }
        if args.polls != 0 && polls >= args.polls {
            return Ok(());
        }
        std::thread::sleep(poll_interval);
    }
}

/// Arguments of `iroha sccp governance propose`.
#[derive(clap::Args, Debug)]
pub struct ProposeArgs {
    /// JSON file with the array of `SccpGovernanceActionV1` to apply in order.
    #[arg(long)]
    pub actions: PathBuf,
    /// Print the proposal instead of submitting it.
    #[arg(long)]
    pub dry_run: bool,
}

/// Build the proposal of `actions` under `network_id` with the current `revisions` (absent
/// subjects are at 0) and run every static check (§4.14.3 step 1).
///
/// # Errors
/// Returns the first failed static check.
pub(crate) fn build_proposal(
    network_id: NetworkId,
    revisions: &[SccpGovernanceBaseRevisionV1],
    actions: Vec<SccpGovernanceActionV1>,
) -> Result<SccpGovernanceProposalV1> {
    let mut proposal = SccpGovernanceProposalV1 {
        network_id,
        base_revisions: Vec::new(),
        actions,
    };
    proposal.base_revisions = proposal
        .subjects()
        .into_iter()
        .map(|subject| {
            let revision = revisions
                .iter()
                .find(|current| current.subject == subject)
                .map_or(0, |current| current.revision);
            (subject, revision).into()
        })
        .collect();
    proposal
        .validate_static(&network_id)
        .map_err(|error| eyre!("the proposal fails static validation: {error:?}"))?;
    Ok(proposal)
}

impl Run for Command {
    fn run<C: RunContext>(self, context: &mut C) -> Result<()> {
        let client = super::blocking(context)?;
        match self {
            Self::Revisions => {
                let revisions = client.sccp().governance_revisions()?;
                context.print_data(&revisions)
            }
            Self::Drive(args) => drive(context, &args),
            Self::Propose(args) => {
                let text = std::fs::read_to_string(&args.actions)
                    .wrap_err_with(|| format!("read {}", args.actions.display()))?;
                let actions: Vec<SccpGovernanceActionV1> = norito::json::from_str(&text)
                    .map_err(|error| eyre!("actions file is not SCCP actions JSON: {error}"))?;
                let revisions = client.sccp().governance_revisions()?;
                let proposal = build_proposal(context.config().network_id, &revisions, actions)?;
                if args.dry_run {
                    return context.print_data(&proposal);
                }
                context.finish(vec![InstructionBox::from(ProposeSccpRouteGovernance {
                    proposal,
                })])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha::data_model::sccp::{
        governance::{SccpGovernanceSubjectV1, SccpSetParametersActionV1},
        params::SccpParametersV1,
    };

    fn network_id() -> NetworkId {
        NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::new([3; 32]),
        ))
    }

    fn open_proposal(
        admissible: bool,
        latest_attempt: Option<iroha::data_model::governance::types::GovernanceAttemptV1>,
    ) -> SccpGovernanceProposalStatusV1 {
        let proposal = build_proposal(
            network_id(),
            &[],
            vec![SccpGovernanceActionV1::SetParameters(
                SccpSetParametersActionV1 {
                    next: SccpParametersV1::taira_default(),
                },
            )],
        )
        .expect("valid");
        SccpGovernanceProposalStatusV1 {
            content_id: iroha::data_model::governance::types::ProposalContentId::new([4; 32]),
            created_height: 7,
            proposal,
            admissible,
            latest_attempt,
        }
    }

    fn attempt(
        status: GovernanceAttemptStatusV1,
    ) -> iroha::data_model::governance::types::GovernanceAttemptV1 {
        let content = iroha::data_model::governance::types::ProposalContentId::new([4; 32]);
        iroha::data_model::governance::types::GovernanceAttemptV1 {
            id: GovernanceAttemptId::derive_v1(content, 0),
            proposal_content_id: content,
            sequence: 0,
            risk_tier: iroha::data_model::governance::types::RiskTierV1::Standard,
            stage: iroha::data_model::governance::types::GovernanceStageV1::Qualification,
            status,
        }
    }

    #[test]
    fn only_admissible_proposals_without_an_attempt_get_one() {
        let fresh = open_proposal(true, None);
        let stale = open_proposal(false, None);
        let running = open_proposal(true, Some(attempt(GovernanceAttemptStatusV1::Active)));
        let kinds = proposals_to_attempt(&[fresh.clone(), stale.clone(), running.clone()]);
        assert_eq!(kinds.len(), 1);
        assert_eq!(
            kinds[0],
            ProposalKind::SccpRouteGovernance(SccpRouteGovernanceProposal {
                proposal: Box::new(fresh.proposal.clone()),
            })
        );
        assert!(needs_blocks(&fresh));
        assert!(!needs_blocks(&stale));
        assert!(needs_blocks(&running));
        assert!(needs_blocks(&open_proposal(
            true,
            Some(attempt(GovernanceAttemptStatusV1::Certified))
        )));
        assert!(!needs_blocks(&open_proposal(
            true,
            Some(attempt(GovernanceAttemptStatusV1::Rejected))
        )));
    }

    #[test]
    fn plans_become_due_exact_relay_and_finalize_steps() {
        use iroha::data_model::isi::governance::ParliamentCloseBallotRegistrationV1;
        use iroha_torii_shared::parliament_api::ParliamentExactTransitionProjectionV1;
        let attempt_id = attempt(GovernanceAttemptStatusV1::Active).id;
        let ballot = BallotAttemptId::new([9; 32]);
        let close = |height| ParliamentExactTransitionProjectionV1 {
            height,
            transition: ParliamentLifecycleTransitionV1::CloseBallotRegistration(
                ParliamentCloseBallotRegistrationV1 {
                    ballot_attempt_id: ballot,
                },
            ),
        };
        let plan = ParliamentAttemptPlanResponseV1 {
            version: 1,
            governance_attempt_id: attempt_id,
            current_height: 40,
            execution_height: 43,
            due: vec![ParliamentLifecycleTransitionV1::CompleteQualification],
            exact: vec![close(43), close(44), close(50)],
            relay_ballots: vec![ballot],
            finalize_ballots: vec![ballot],
        };
        let steps = plan_steps(&plan);
        assert_eq!(
            steps,
            vec![
                DriveStep::Due(
                    attempt_id,
                    vec![ParliamentLifecycleTransitionV1::CompleteQualification]
                ),
                DriveStep::Exact {
                    attempt: attempt_id,
                    execution_height: 43,
                    transition: close(43).transition,
                },
                DriveStep::Exact {
                    attempt: attempt_id,
                    execution_height: 43,
                    transition: close(44).transition,
                },
                DriveStep::Relay(ballot),
                DriveStep::Finalize(ballot),
            ],
            "the exact checkpoint seven blocks out waits"
        );
        // The same exact transition aimed at two execution heights is sent twice.
        let later = DriveStep::Exact {
            attempt: attempt_id,
            execution_height: 44,
            transition: close(44).transition,
        };
        assert_ne!(steps[2].key(), later.key());
        let keys: std::collections::BTreeSet<_> = steps.iter().map(DriveStep::key).collect();
        assert_eq!(
            keys.len(),
            steps.len() - 1,
            "exact copies share a key per height only"
        );
    }

    #[test]
    fn proposals_carry_current_revisions_of_their_subjects() {
        let actions = vec![SccpGovernanceActionV1::SetParameters(
            SccpSetParametersActionV1 {
                next: SccpParametersV1::taira_default(),
            },
        )];
        let revisions = vec![
            (SccpGovernanceSubjectV1::Parameters, 4).into(),
            (
                SccpGovernanceSubjectV1::LightClient(
                    iroha::data_model::bridge::SccpNetworkV1::TonMainnet,
                ),
                9,
            )
                .into(),
        ];
        let proposal = build_proposal(network_id(), &revisions, actions.clone()).expect("valid");
        assert_eq!(proposal.base_revisions.len(), 1);
        assert_eq!(proposal.base_revisions[0].revision, 4);
        let fresh = build_proposal(network_id(), &[], actions).expect("valid");
        assert_eq!(fresh.base_revisions[0].revision, 0);
        assert!(build_proposal(network_id(), &[], Vec::new()).is_err());
    }
}
