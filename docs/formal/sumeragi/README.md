# Sumeragi Formal Model (TLA+ / Apalache)

This directory contains bounded formal models for Sumeragi safety and liveness.

## Scope

`Sumeragi.tla` captures the commit path:
- phase progression (`Propose`, `Prepare`, `CommitVote`, `NewView`, `Committed`),
- vote and quorum thresholds (`CommitQuorum`, `ViewQuorum`),
- weighted stake quorum (`StakeQuorum`) for NPoS-style commit guards,
- RBC causality (`Init -> Chunk -> Ready -> Deliver`) with header/digest evidence,
- GST and weak fairness assumptions over honest progress actions.

`SumeragiForkSafety.tla` captures same-height fork safety with two conflicting
branches:
- honest and Byzantine commit signer sets,
- permissioned count quorum and optional stake quorum,
- locked-QC gating for same-height branch replacement,
- honest single-vote discipline across branches,
- commit-certificate formation for each branch, plus a mutation that disables
  the single-vote/locked-QC guards and must produce a counterexample.

`SumeragiQuorumPolicy.tla` captures fail-closed quorum-policy arithmetic:
- permissioned count quorum requires a strict two-thirds supermajority plus
  one and rejects signer counts above the active validator count,
- NPoS stake quorum requires signed stake to strictly exceed two thirds of
  total stake,
- missing/negative stake, zero/negative total stake, over-total stake, exact
  two-thirds stake, and overflow all fail closed.

`SumeragiRbcDeliverQuorum.tla` captures the RBC deliver-quorum gate:
- the default deliver threshold equals the commit quorum over the deduplicated
  validator topology,
- topologies of one to three validators require all validators; larger
  topologies require `floor(2 * validators / 3) + 1`,
- the debug force-one path uses threshold one,
- READY counting uses distinct senders, so duplicate READY observations cannot
  inflate the deliver decision,
- deliver is impossible before the distinct READY count reaches the required
  threshold.

`SumeragiQcSignerBitmap.tla` captures QC signer-bitmap admission:
- bitmap length must match the topology-derived byte length,
- signer bits outside the topology are rejected,
- only signer indices inside the voting validator set count toward quorum,
- observer or padding indices cannot satisfy quorum on behalf of voting
  validators,
- accepted QC signer evidence must match the voting-set quorum predicate.

`SumeragiCommitRootConsistency.tla` captures commit-QC execution-root
consistency:
- commit votes are filtered into one same-root group before quorum is
  evaluated,
- permissioned mode selects the largest same-root signer group with a
  deterministic low-root tie-break,
- NPoS mode selects the heaviest same-root stake group with the same tie-break,
- wrong-context votes cannot help satisfy quorum,
- QC validation rejects signer votes whose roots do not match the QC roots.

`SumeragiCommitPipelineRecoveryGate.tla` captures adapter-side commit-pipeline
recovery ordering:
- cached commit-vote quorums must be aggregated into a local commit QC before
  peer missing-QC recovery is armed,
- the locally formed commit-QC marker must stay attached to the pending block,
- missing commit-QC recovery is armed only for a valid, payload-local, stale,
  locally voted pending block that still extends the committed tip,
- fresh local votes, existing commit QCs, missing local DA payloads, invalid
  pending blocks, missing local votes, and off-tip candidates do not arm peer
  recovery,
- cached near-quorum commit votes are rebroadcast to quorum missing-signer
  targets, not the proposal collector subset, and empty/committed vote sets do
  not rebroadcast.

`SumeragiCommitEvidenceReplayGate.tla` captures known-block commit-evidence
replay pacing:
- inactive pending blocks, wrong-round calls, aborted pending blocks, cooldown
  hits, zero-evidence states, and local-only target sets cannot emit replay
  traffic,
- first evidence, vote-count progress, commit-QC progress, view progress, and
  stalled positive evidence after cooldown may replay,
- vote evidence is replayed as `QcVote` and commit-QC evidence as `CommitCert`,
- replay never falls back to `BlockCreated` payload broadcasts or
  `BlockSyncUpdate` hydration,
- explicit replay targets exclude the local peer and are deduplicated before
  outbound work is scheduled.

`SumeragiBlockSyncRecoveryGate.tla` captures BlockSyncUpdate recovery
admission into the BlockCreated owner path:
- stale-view updates require a missing-block request, cached commit evidence,
  or an explicit commit-QC repair mode,
- payload-only recovery may hydrate or retain a branch, but cannot steal
  authoritative same-height/frontier ownership or clear stale commit inflight,
- commit-QC/certified recovery revives aborted placeholders, keeps commit-QC
  evidence attached, supersedes stale same-height owners, and clears stale
  commit inflight,
- sparse next-height payloads and vote-only unknown-frontier updates track
  missing commit-QC repair,
- unvalidated commit-QC sidecars cannot promote lock or highest-QC state.

`SumeragiNativeAmxAttestationGate.tla` captures native AMX proposer-side
prepare/commit attestation gating:
- non-AMX plans return no receipt, and native AMX plans without a BLS-capable
  roster fail closed,
- prepare quorum must exist before commit requests are broadcast,
- every participant leg must have both prepare and commit QCs before proposal
  assembly seals a native AMX receipt,
- invalid duplicate, wrong-body, or outsider vote sets cannot build QCs,
- vote projection is deterministic in validator-set order, and retried bodies
  plus distinct participant legs stay separate in the vote cache.

`SumeragiNativeAmxJournalReplay.tla` captures native AMX queue-plan journal
replay across restart:
- full native AMX routing plans, entrypoints, and gossip payloads are replayed
  rather than collapsed into single-lane records,
- tombstones are scoped by `(signed_transaction_hash, plan_digest)`, so removing
  one digest cannot delete a re-admitted transaction with a new plan,
- unsupported journal record versions are ignored,
- duplicate puts for the same key keep the last record,
- compaction preserves exactly the live records,
- torn payload or length tails are repaired while preserving the last complete
  native AMX record.

`SumeragiPrecommitVoteGate.tla` captures local precommit vote emission:
- pending blocks must be validated before the local node signs a precommit,
- observers and peers outside the view-aligned voting topology cannot sign,
- duplicate same-slot votes and unsuperseded same-height conflicts are
  rejected,
- a newer conflicting branch may be signed only when it is superseded by
  accepted new-view evidence or the local vote completes the newer-view quorum,
- older conflicting branches cannot use quorum-completion as an escape hatch,
- locked-QC conflicts, missing locked payloads at the same or older view, and
  non-extending locked-chain candidates fail closed.

`SumeragiProposalAssemblyGate.tla` captures local proposal assembly before
prepare voting:
- observers and non-leaders cannot assemble fresh proposals,
- active local same-height vote conflicts and pending same-height vote
  verification defer proposal assembly without mutating proposal-cache state,
- missing highest-QC payloads and non-extending highest-QC ancestry defer
  proposal assembly,
- split same-height vote locks and committed-edge highest-QC conflicts do not
  produce fresh proposals,
- stale retired prior-view vote history, accepted new-view supersession,
  locked-QC fallback, and locked-chain extension remain permitted liveness
  cases.

`SumeragiEngineTickGate.tla` captures the pure engine pacemaker tick gate:
- every tick advances the local view by one,
- ticks return the engine to proposal phase and emit both a `NewView` vote and
  an `AdvanceView` output,
- any in-flight proposal validation is cleared before late callbacks arrive,
- highest-QC state is bound into the `NewView` vote subject and highest-QC
  field when present; otherwise the vote uses the zero subject with no highest
  QC,
- pending finality is preserved across view changes so exact payload recovery
  can still complete.

`SumeragiEngineNewViewQcGate.tla` captures the pure engine NewView-QC gate:
- NewView certificates must match the current height, epoch, validator set, and
  quorum policy,
- accepted NewView certificates must carry a strictly newer view,
- carried highest-QC evidence must be compatible with the certificate round,
- accepted NewView QCs emit `AdvanceView`, return to proposal phase, clear
  in-flight validation, and preserve pending finality,
- accepted highest-QC evidence updates local highest-QC state only when it
  improves the existing reference.

`SumeragiEngineProposalGate.tla` captures the pure engine proposal-ingress
gate:
- proposals are accepted only while the engine is in proposal phase,
- proposal rounds must match the current height, epoch, validator set, and
  view,
- carried highest-QC evidence must be compatible with the proposal round,
- locked conflicting proposals require a strictly higher compatible QC, while
  unlocked proposals and proposals for the locked subject remain safe,
- accepted proposals must request validation, sign a prepare vote, and enter
  prepare phase.

`SumeragiEnginePrepareQcGate.tla` captures the pure engine prepare-QC to
commit-vote transition:
- prepare certificates must match the current height, epoch, validator set,
  view, and quorum policy before they can make the engine sign a commit vote,
- prepare certificates for already committed heights are ignored,
- replayed same-subject and conflicting prepare QCs for a round do not emit
  additional commit votes,
- prepare QCs cannot emit commit votes while the engine is waiting for pending
  finality payload recovery,
- accepted prepare QCs must record both the locked QC and highest QC.

`SumeragiEngineCommitQcGate.tla` captures the pure engine commit-QC finality
gate:
- commit certificates must match the current height, epoch, validator set,
  view, and quorum policy before they can affect finality,
- commit QCs for already committed heights, pending-finality replays, and
  conflicting pending-finality subjects are ignored,
- payload-available commit QCs finalize immediately,
- missing-payload commit QCs request exact payload recovery instead of
  finalizing,
- accepted commit QCs must record highest-QC state.

`SumeragiEnginePayloadAvailabilityGate.tla` captures the pure engine
payload-availability gate:
- payload availability alone cannot finalize a block,
- when a commit QC is pending, only the exact certified subject can commit,
- payload hash mismatches, parent mismatches, and unrelated block hashes are
  ignored without dropping pending finality,
- the exact matching payload clears pending finality and returns the engine to
  proposal phase.

`SumeragiEngineValidationResultGate.tla` captures the pure engine
validation-result gate:
- only the exact current in-flight validation result can mutate consensus
  state,
- valid current results clear validation ownership without emitting consensus
  outputs,
- invalid current results clear ownership, advance the view, emit a `NewView`
  vote and `AdvanceView`, and bind the correct highest-QC or fallback subject,
- wrong-round, wrong-block, replayed, no-in-flight, commit-superseded, and
  storage-committed callbacks are ignored without dropping pending finality or
  overwriting committed state.

`SumeragiEngineCommittedBlockGate.tla` captures the pure engine committed-block
notification gate:
- fresh committed-block notifications record the height,
- only a fresh boundary reconfiguration notification emits validator-set
  activation,
- duplicate same-height notifications are idempotent,
- conflicting same-height notifications cannot overwrite the committed hash or
  activate a validator set.

`SumeragiValidatorSetTransition.tla` captures the validator-set activation
gate for one scheduled reconfiguration:
- old-set finality at the activation boundary,
- staged activation only after that old-set certificate,
- new-set certificates only after activation,
- old-set certificates stopping before the activation height,
- rejection of mixed-set certificates and multiple validator-set certificates
  for one height.

`SumeragiCertifiedRecovery.tla` captures certified block recovery when a commit
QC arrives before the matching payload:
- pending finality is anchored to an observed commit QC,
- exact payload recovery is required before state application,
- mismatched certified block responses are rejected without dropping the
  pending QC,
- a same-height conflicting subject cannot finalize after another subject is
  already committed.

`SumeragiViewChangeSafety.tla` captures the view-change and locked-proposal
gate:
- accepted new-view certificates move the local view forward only,
- highest-QC tracking is monotonic over accepted evidence,
- locked validators reject conflicting proposals unless the proposal carries a
  strictly higher QC,
- conflicting prepare evidence cannot overwrite an existing same-height lock at
  the same or lower QC rank.

`SumeragiValidationGate.tla` captures asynchronous proposal-validation callback
ownership:
- only the current in-flight validation result may advance the view on failure,
- unknown validation results are ignored,
- completed validation result replays are ignored,
- timeout-stale validation failures cannot advance the view after timeout
  already cleared the in-flight proposal,
- one invalid validation result cannot advance the same proposal twice.

`SumeragiCertificateAdmission.tla` captures certificate admission before
evidence mutates consensus state:
- wrong height, epoch, validator-set, or quorum-policy certificates are ignored,
- stale prepare/commit certificates after view advance are ignored,
- future-height certificates are ignored,
- certificates for already committed heights are ignored.

`SumeragiHighestQcSelection.tla` captures deterministic highest-QC selection
from new-view evidence:
- only `NewView` certificates contribute embedded highest-QC evidence,
- QCs are ordered by height, then view, then phase rank, then subject hash,
- two replicas observing the same certificate set in different orders must
  select the same QC,
- mutations that ignore height priority, phase rank, subject tie-breaking, or
  certificate phase must produce counterexamples.

`SumeragiFrontierRecovery.tla` captures the focused Taira hang class around one
active pending contiguous frontier block plus one concrete future frontier slot:
- commit-vote evidence below or at quorum,
- vote queue backlog and local drain,
- missing vs. local payload state,
- fresh vs. stale frontier recovery ownership,
- quorum-reschedule marker/window pacing,
- future slot presence, contiguity, vote evidence, payload state, and recovery
  ownership,
- future frontier/new-view evidence derived from that future slot and consumed
  through a two-step reanchor/promotion path,
- late arrival of future frontier evidence after GST,
- promotion freshness, so a promoted second slot cannot inherit stale active
  payload recovery, retransmit, quorum-window, or view-rotation progress,
- active pending progress age/event tracking, so validation, local commit-vote,
  commit-QC, payload recovery, retransmit, reanchor, promotion, and rotation
  progress must explicitly touch the pending block progress marker,
- same-height stale recovery unlocks scoped to the subject view that was
  rotated, not just to the block height,
- deterministic post-GST commit, retransmit, bounded view-rotation, and
  zero-evidence drop outcomes.

All models intentionally abstract away wire formats, ECDSA/signature
verification, and full networking details.

## Files

- `Sumeragi.tla`: protocol model and properties.
- `Sumeragi_fast.cfg`: smaller CI-friendly parameter set.
- `Sumeragi_deep.cfg`: larger stress parameter set.
- `SumeragiForkSafety.tla`: same-height conflicting-branch commit-certificate safety model.
- `SumeragiForkSafety_fast.cfg`: permissioned count-quorum fork-safety check.
- `SumeragiForkSafety_npos.cfg`: NPoS-style stake-quorum fork-safety check.
- `SumeragiForkSafety_bug_double_sign.cfg`: expected-failure double-sign/lock-gate mutation.
- `SumeragiQuorumPolicy.tla`: fail-closed quorum-policy arithmetic model.
- `SumeragiQuorumPolicy_fast.cfg`: CI-friendly quorum-policy arithmetic check.
- `SumeragiQuorumPolicy_bug_count_under_threshold.cfg`: expected-failure under-threshold count mutation.
- `SumeragiQuorumPolicy_bug_count_over_validators.cfg`: expected-failure over-validator count mutation.
- `SumeragiQuorumPolicy_bug_stake_exact_two_thirds.cfg`: expected-failure exact two-thirds stake mutation.
- `SumeragiQuorumPolicy_bug_stake_over_total.cfg`: expected-failure over-total stake mutation.
- `SumeragiQuorumPolicy_bug_stake_invalid_input.cfg`: expected-failure invalid stake input mutation.
- `SumeragiQuorumPolicy_bug_stake_overflow.cfg`: expected-failure stake overflow mutation.
- `SumeragiRbcDeliverQuorum.tla`: RBC deliver-quorum gate model.
- `SumeragiRbcDeliverQuorum_fast.cfg`: CI-friendly RBC deliver-quorum check.
- `SumeragiRbcDeliverQuorum_bug_duplicate_ready_count.cfg`: expected-failure duplicate READY counting mutation.
- `SumeragiRbcDeliverQuorum_bug_under_quorum_deliver.cfg`: expected-failure under-quorum delivery mutation.
- `SumeragiRbcDeliverQuorum_bug_wrong_commit_formula.cfg`: expected-failure commit-quorum arithmetic mutation.
- `SumeragiRbcDeliverQuorum_bug_force_one_ignored.cfg`: expected-failure force-one debug path mutation.
- `SumeragiQcSignerBitmap.tla`: QC signer-bitmap admission model.
- `SumeragiQcSignerBitmap_fast.cfg`: CI-friendly QC signer-bitmap admission check.
- `SumeragiQcSignerBitmap_bug_count_observers.cfg`: expected-failure observer-counting mutation.
- `SumeragiQcSignerBitmap_bug_ignore_bitmap_length.cfg`: expected-failure bitmap-length mutation.
- `SumeragiQcSignerBitmap_bug_ignore_out_of_bounds.cfg`: expected-failure out-of-bounds signer mutation.
- `SumeragiQcSignerBitmap_bug_under_quorum_accept.cfg`: expected-failure under-quorum acceptance mutation.
- `SumeragiCommitRootConsistency.tla`: commit-QC execution-root consistency model.
- `SumeragiCommitRootConsistency_fast.cfg`: CI-friendly commit-root consistency check.
- `SumeragiCommitRootConsistency_bug_mix_root_signers.cfg`: expected-failure mixed-root quorum mutation.
- `SumeragiCommitRootConsistency_bug_count_wrong_context.cfg`: expected-failure wrong-context vote counting mutation.
- `SumeragiCommitRootConsistency_bug_tie_high_root.cfg`: expected-failure nondeterministic/high-root tie mutation.
- `SumeragiCommitRootConsistency_bug_stake_ignores_weight.cfg`: expected-failure NPoS stake root-selection mutation.
- `SumeragiCommitRootConsistency_bug_under_quorum_accept.cfg`: expected-failure under-quorum root group mutation.
- `SumeragiCommitRootConsistency_bug_validate_mismatched_roots.cfg`: expected-failure root-mismatch validation mutation.
- `SumeragiCommitPipelineRecoveryGate.tla`: commit-pipeline recovery ordering model.
- `SumeragiCommitPipelineRecoveryGate_fast.cfg`: CI-friendly commit-pipeline recovery gate check.
- `SumeragiCommitPipelineRecoveryGate_bug_skip_local_qc_formation.cfg`: expected-failure missing local commit-QC aggregation mutation.
- `SumeragiCommitPipelineRecoveryGate_bug_recover_despite_local_quorum.cfg`: expected-failure peer recovery before using local quorum mutation.
- `SumeragiCommitPipelineRecoveryGate_bug_request_recovery_before_timeout.cfg`: expected-failure fresh local-vote recovery mutation.
- `SumeragiCommitPipelineRecoveryGate_bug_request_recovery_without_local_vote.cfg`: expected-failure no-local-vote recovery mutation.
- `SumeragiCommitPipelineRecoveryGate_bug_request_recovery_with_commit_qc.cfg`: expected-failure recovery despite observed commit QC mutation.
- `SumeragiCommitPipelineRecoveryGate_bug_request_recovery_with_missing_data.cfg`: expected-failure missing-local-data recovery mutation.
- `SumeragiCommitPipelineRecoveryGate_bug_request_recovery_invalid_pending.cfg`: expected-failure invalid-pending recovery mutation.
- `SumeragiCommitPipelineRecoveryGate_bug_request_recovery_off_tip.cfg`: expected-failure off-tip recovery mutation.
- `SumeragiCommitPipelineRecoveryGate_bug_skip_missing_qc_request.cfg`: expected-failure missing peer recovery mutation.
- `SumeragiCommitPipelineRecoveryGate_bug_drop_commit_qc_marker.cfg`: expected-failure dropped commit-QC marker mutation.
- `SumeragiCommitPipelineRecoveryGate_bug_skip_quorum_retransmit.cfg`: expected-failure missing near-quorum retransmit mutation.
- `SumeragiCommitPipelineRecoveryGate_bug_use_collector_targets.cfg`: expected-failure collector-target retransmit mutation.
- `SumeragiCommitPipelineRecoveryGate_bug_rebroadcast_without_votes.cfg`: expected-failure empty-vote rebroadcast mutation.
- `SumeragiCommitPipelineRecoveryGate_bug_rebroadcast_after_qc.cfg`: expected-failure post-QC rebroadcast mutation.
- `SumeragiCommitEvidenceReplayGate.tla`: known-block commit-evidence replay gate model.
- `SumeragiCommitEvidenceReplayGate_fast.cfg`: CI-friendly commit-evidence replay gate check.
- `SumeragiCommitEvidenceReplayGate_bug_replay_inactive.cfg`: expected-failure inactive pending replay mutation.
- `SumeragiCommitEvidenceReplayGate_bug_ignore_cooldown.cfg`: expected-failure cooldown bypass mutation.
- `SumeragiCommitEvidenceReplayGate_bug_replay_without_targets.cfg`: expected-failure local-only/no-target replay mutation.
- `SumeragiCommitEvidenceReplayGate_bug_skip_first_evidence.cfg`: expected-failure first-evidence replay drop mutation.
- `SumeragiCommitEvidenceReplayGate_bug_skip_progress.cfg`: expected-failure progress replay drop mutation.
- `SumeragiCommitEvidenceReplayGate_bug_skip_stalled_retry.cfg`: expected-failure stalled positive-evidence retry drop mutation.
- `SumeragiCommitEvidenceReplayGate_bug_replay_no_evidence.cfg`: expected-failure zero-evidence replay mutation.
- `SumeragiCommitEvidenceReplayGate_bug_votes_use_payload_fallback.cfg`: expected-failure vote replay payload-fallback mutation.
- `SumeragiCommitEvidenceReplayGate_bug_commit_qc_uses_votes.cfg`: expected-failure commit-QC replay as votes mutation.
- `SumeragiCommitEvidenceReplayGate_bug_drop_commit_qc_replay.cfg`: expected-failure dropped commit-QC replay mutation.
- `SumeragiCommitEvidenceReplayGate_bug_use_local_targets.cfg`: expected-failure local-target replay mutation.
- `SumeragiCommitEvidenceReplayGate_bug_use_duplicate_targets.cfg`: expected-failure duplicate-target replay mutation.
- `SumeragiBlockSyncRecoveryGate.tla`: block-sync recovery admission model.
- `SumeragiBlockSyncRecoveryGate_fast.cfg`: CI-friendly block-sync recovery gate check.
- `SumeragiBlockSyncRecoveryGate_bug_accept_stale_without_request.cfg`: expected-failure stale update accepted without request/evidence mutation.
- `SumeragiBlockSyncRecoveryGate_bug_drop_requested_stale.cfg`: expected-failure requested stale payload drop mutation.
- `SumeragiBlockSyncRecoveryGate_bug_accept_future_unrequested.cfg`: expected-failure unrequested future-height acceptance mutation.
- `SumeragiBlockSyncRecoveryGate_bug_revive_aborted_without_commit_qc.cfg`: expected-failure payload-only aborted revival mutation.
- `SumeragiBlockSyncRecoveryGate_bug_keep_aborted_with_commit_qc.cfg`: expected-failure commit-QC aborted placeholder retention mutation.
- `SumeragiBlockSyncRecoveryGate_bug_skip_vote_backed_owner.cfg`: expected-failure vote-backed stale owner drop mutation.
- `SumeragiBlockSyncRecoveryGate_bug_steal_owner_with_payload_only.cfg`: expected-failure payload-only owner steal mutation.
- `SumeragiBlockSyncRecoveryGate_bug_skip_certified_owner.cfg`: expected-failure certified recovery owner drop mutation.
- `SumeragiBlockSyncRecoveryGate_bug_activate_uncertified_conflict.cfg`: expected-failure raw same-height conflict activation mutation.
- `SumeragiBlockSyncRecoveryGate_bug_drop_commit_qc_marker.cfg`: expected-failure commit-QC marker loss mutation.
- `SumeragiBlockSyncRecoveryGate_bug_skip_missing_commit_qc_request.cfg`: expected-failure missing commit-QC repair tracking mutation.
- `SumeragiBlockSyncRecoveryGate_bug_keep_missing_request.cfg`: expected-failure missing-block request retention mutation.
- `SumeragiBlockSyncRecoveryGate_bug_clear_inflight_for_payload_only.cfg`: expected-failure payload-only stale inflight clear mutation.
- `SumeragiBlockSyncRecoveryGate_bug_keep_inflight_for_certified.cfg`: expected-failure certified stale inflight retention mutation.
- `SumeragiBlockSyncRecoveryGate_bug_promote_unvalidated_qc.cfg`: expected-failure unvalidated commit-QC promotion mutation.
- `SumeragiNativeAmxAttestationGate.tla`: native AMX attestation gate model.
- `SumeragiNativeAmxAttestationGate_fast.cfg`: CI-friendly native AMX attestation gate check.
- `SumeragiNativeAmxAttestationGate_bug_seal_non_native_plan.cfg`: expected-failure non-AMX receipt mutation.
- `SumeragiNativeAmxAttestationGate_bug_seal_empty_roster.cfg`: expected-failure empty-roster receipt mutation.
- `SumeragiNativeAmxAttestationGate_bug_skip_prepare_request.cfg`: expected-failure missing prepare-request mutation.
- `SumeragiNativeAmxAttestationGate_bug_skip_commit_request.cfg`: expected-failure missing commit-request mutation.
- `SumeragiNativeAmxAttestationGate_bug_request_commit_before_prepare.cfg`: expected-failure commit-before-prepare mutation.
- `SumeragiNativeAmxAttestationGate_bug_retry_prepare_after_quorum.cfg`: expected-failure redundant prepare retry mutation.
- `SumeragiNativeAmxAttestationGate_bug_seal_with_prepare_only.cfg`: expected-failure prepare-only receipt mutation.
- `SumeragiNativeAmxAttestationGate_bug_seal_with_commit_only.cfg`: expected-failure commit-only receipt mutation.
- `SumeragiNativeAmxAttestationGate_bug_seal_partial_multi_leg.cfg`: expected-failure partial multi-leg receipt mutation.
- `SumeragiNativeAmxAttestationGate_bug_accept_duplicate_prepare.cfg`: expected-failure duplicate prepare signer mutation.
- `SumeragiNativeAmxAttestationGate_bug_accept_duplicate_commit.cfg`: expected-failure duplicate commit signer mutation.
- `SumeragiNativeAmxAttestationGate_bug_accept_wrong_prepare_body.cfg`: expected-failure wrong prepare-body mutation.
- `SumeragiNativeAmxAttestationGate_bug_accept_wrong_commit_body.cfg`: expected-failure wrong commit-body mutation.
- `SumeragiNativeAmxAttestationGate_bug_accept_outsider_signer.cfg`: expected-failure outsider signer mutation.
- `SumeragiNativeAmxAttestationGate_bug_use_arrival_order_bitmap.cfg`: expected-failure nondeterministic signer projection mutation.
- `SumeragiNativeAmxAttestationGate_bug_collapse_retry_bodies.cfg`: expected-failure retried-body cache collision mutation.
- `SumeragiNativeAmxAttestationGate_bug_collapse_participant_legs.cfg`: expected-failure participant-leg cache collision mutation.
- `SumeragiNativeAmxJournalReplay.tla`: native AMX queue-plan journal replay model.
- `SumeragiNativeAmxJournalReplay_fast.cfg`: CI-friendly native AMX journal replay check.
- `SumeragiNativeAmxJournalReplay_bug_drop_native_plan.cfg`: expected-failure native plan drop mutation.
- `SumeragiNativeAmxJournalReplay_bug_collapse_native_to_single.cfg`: expected-failure native plan collapsed to single-route mutation.
- `SumeragiNativeAmxJournalReplay_bug_single_plan_as_native.cfg`: expected-failure single-route plan replayed as native AMX mutation.
- `SumeragiNativeAmxJournalReplay_bug_drop_participants.cfg`: expected-failure participant-leg drop mutation.
- `SumeragiNativeAmxJournalReplay_bug_reorder_participants.cfg`: expected-failure participant ordering mutation.
- `SumeragiNativeAmxJournalReplay_bug_keep_duplicate_participant.cfg`: expected-failure participant deduplication mutation.
- `SumeragiNativeAmxJournalReplay_bug_recompute_digest_wrong.cfg`: expected-failure plan digest corruption mutation.
- `SumeragiNativeAmxJournalReplay_bug_drop_gossip_payload.cfg`: expected-failure gossip payload loss mutation.
- `SumeragiNativeAmxJournalReplay_bug_drop_entrypoint.cfg`: expected-failure entrypoint loss mutation.
- `SumeragiNativeAmxJournalReplay_bug_remove_by_hash_only.cfg`: expected-failure hash-only tombstone mutation.
- `SumeragiNativeAmxJournalReplay_bug_ignore_exact_remove.cfg`: expected-failure ignored exact tombstone mutation.
- `SumeragiNativeAmxJournalReplay_bug_replay_unsupported_version.cfg`: expected-failure unsupported-version replay mutation.
- `SumeragiNativeAmxJournalReplay_bug_first_put_wins.cfg`: expected-failure first-put-wins replacement mutation.
- `SumeragiNativeAmxJournalReplay_bug_compaction_drops_live.cfg`: expected-failure compaction live-record drop mutation.
- `SumeragiNativeAmxJournalReplay_bug_compaction_keeps_removed.cfg`: expected-failure compaction removed-record retention mutation.
- `SumeragiNativeAmxJournalReplay_bug_keep_torn_tail.cfg`: expected-failure torn-tail retention mutation.
- `SumeragiNativeAmxJournalReplay_bug_drop_prior_on_tail_repair.cfg`: expected-failure complete-prefix loss during tail repair mutation.
- `SumeragiPrecommitVoteGate.tla`: local precommit vote-emission gate model.
- `SumeragiPrecommitVoteGate_fast.cfg`: CI-friendly precommit vote-emission check.
- `SumeragiPrecommitVoteGate_bug_invalid_validation.cfg`: expected-failure invalid-validation emission mutation.
- `SumeragiPrecommitVoteGate_bug_observer.cfg`: expected-failure observer/out-of-topology emission mutation.
- `SumeragiPrecommitVoteGate_bug_duplicate.cfg`: expected-failure duplicate same-slot emission mutation.
- `SumeragiPrecommitVoteGate_bug_unsuperseded_conflict.cfg`: expected-failure unsuperseded same-height conflict mutation.
- `SumeragiPrecommitVoteGate_bug_older_quorum_completion.cfg`: expected-failure older-branch quorum-completion mutation.
- `SumeragiPrecommitVoteGate_bug_locked_conflict.cfg`: expected-failure locked same-height conflict mutation.
- `SumeragiPrecommitVoteGate_bug_missing_locked_payload.cfg`: expected-failure missing locked-payload mutation.
- `SumeragiPrecommitVoteGate_bug_non_extending_lock.cfg`: expected-failure non-extending locked-chain mutation.
- `SumeragiPrecommitVoteGate_bug_reject_safe.cfg`: expected-failure safe-candidate rejection mutation.
- `SumeragiProposalAssemblyGate.tla`: local proposal assembly gate model.
- `SumeragiProposalAssemblyGate_fast.cfg`: CI-friendly proposal assembly gate check.
- `SumeragiProposalAssemblyGate_bug_observer.cfg`: expected-failure observer/non-leader assembly mutation.
- `SumeragiProposalAssemblyGate_bug_active_vote_conflict.cfg`: expected-failure active same-height vote conflict mutation.
- `SumeragiProposalAssemblyGate_bug_pending_vote_verification.cfg`: expected-failure pending vote-verification mutation.
- `SumeragiProposalAssemblyGate_bug_missing_highest_qc.cfg`: expected-failure missing highest-QC mutation.
- `SumeragiProposalAssemblyGate_bug_non_extending_highest.cfg`: expected-failure non-extending highest-QC mutation.
- `SumeragiProposalAssemblyGate_bug_split_vote_lock.cfg`: expected-failure split-vote lock mutation.
- `SumeragiProposalAssemblyGate_bug_committed_edge_conflict.cfg`: expected-failure committed-edge highest-QC mutation.
- `SumeragiProposalAssemblyGate_bug_reject_safe.cfg`: expected-failure safe proposal rejection mutation.
- `SumeragiProposalAssemblyGate_bug_reject_stale_retired.cfg`: expected-failure stale retired vote-history rejection mutation.
- `SumeragiProposalAssemblyGate_bug_reject_locked_fallback.cfg`: expected-failure locked fallback rejection mutation.
- `SumeragiEngineTickGate.tla`: pure engine pacemaker tick gate model.
- `SumeragiEngineTickGate_fast.cfg`: CI-friendly engine tick gate check.
- `SumeragiEngineTickGate_bug_skip_round_advance.cfg`: expected-failure missing round-advance mutation.
- `SumeragiEngineTickGate_bug_skip_new_view_vote.cfg`: expected-failure missing NewView vote mutation.
- `SumeragiEngineTickGate_bug_skip_advance_output.cfg`: expected-failure missing AdvanceView output mutation.
- `SumeragiEngineTickGate_bug_wrong_phase.cfg`: expected-failure wrong post-tick phase mutation.
- `SumeragiEngineTickGate_bug_keep_validation.cfg`: expected-failure retained validation-in-flight mutation.
- `SumeragiEngineTickGate_bug_drop_pending_finality.cfg`: expected-failure dropped pending-finality mutation.
- `SumeragiEngineTickGate_bug_use_zero_despite_highest.cfg`: expected-failure highest-QC subject loss mutation.
- `SumeragiEngineTickGate_bug_use_highest_without_highest.cfg`: expected-failure false highest-QC subject mutation.
- `SumeragiEngineTickGate_bug_omit_highest_binding.cfg`: expected-failure missing highest-QC binding mutation.
- `SumeragiEngineTickGate_bug_bind_highest_without_highest.cfg`: expected-failure spurious highest-QC binding mutation.
- `SumeragiEngineNewViewQcGate.tla`: pure engine NewView-QC gate model.
- `SumeragiEngineNewViewQcGate_fast.cfg`: CI-friendly engine NewView-QC gate check.
- `SumeragiEngineNewViewQcGate_bug_accept_wrong_context.cfg`: expected-failure wrong round-context NewView-QC mutation.
- `SumeragiEngineNewViewQcGate_bug_accept_wrong_quorum.cfg`: expected-failure wrong quorum-policy NewView-QC mutation.
- `SumeragiEngineNewViewQcGate_bug_accept_stale_view.cfg`: expected-failure stale or same-view NewView-QC mutation.
- `SumeragiEngineNewViewQcGate_bug_accept_incompatible_highest.cfg`: expected-failure incompatible highest-QC mutation.
- `SumeragiEngineNewViewQcGate_bug_reject_safe_no_highest.cfg`: expected-failure safe no-highest NewView-QC rejection mutation.
- `SumeragiEngineNewViewQcGate_bug_reject_safe_improving_highest.cfg`: expected-failure safe improving-highest NewView-QC rejection mutation.
- `SumeragiEngineNewViewQcGate_bug_reject_safe_lower_highest.cfg`: expected-failure safe lower-highest NewView-QC rejection mutation.
- `SumeragiEngineNewViewQcGate_bug_skip_advance_output.cfg`: expected-failure missing AdvanceView output mutation.
- `SumeragiEngineNewViewQcGate_bug_wrong_phase.cfg`: expected-failure wrong post-NewView phase mutation.
- `SumeragiEngineNewViewQcGate_bug_keep_validation.cfg`: expected-failure retained validation-in-flight mutation.
- `SumeragiEngineNewViewQcGate_bug_drop_pending_finality.cfg`: expected-failure dropped pending-finality mutation.
- `SumeragiEngineNewViewQcGate_bug_overwrite_lower_highest.cfg`: expected-failure lower highest-QC overwrite mutation.
- `SumeragiEngineNewViewQcGate_bug_skip_highest_record.cfg`: expected-failure missing improving highest-QC record mutation.
- `SumeragiEngineProposalGate.tla`: pure engine proposal-ingress gate model.
- `SumeragiEngineProposalGate_fast.cfg`: CI-friendly engine proposal-ingress gate check.
- `SumeragiEngineProposalGate_bug_wrong_phase.cfg`: expected-failure wrong phase proposal mutation.
- `SumeragiEngineProposalGate_bug_wrong_round.cfg`: expected-failure wrong round-context proposal mutation.
- `SumeragiEngineProposalGate_bug_incompatible_highest.cfg`: expected-failure incompatible highest-QC mutation.
- `SumeragiEngineProposalGate_bug_locked_conflict_no_qc.cfg`: expected-failure locked conflict without QC mutation.
- `SumeragiEngineProposalGate_bug_locked_conflict_equal_qc.cfg`: expected-failure locked conflict with equal-QC mutation.
- `SumeragiEngineProposalGate_bug_locked_conflict_lower_qc.cfg`: expected-failure locked conflict with lower-QC mutation.
- `SumeragiEngineProposalGate_bug_reject_unlocked.cfg`: expected-failure unlocked safe proposal rejection mutation.
- `SumeragiEngineProposalGate_bug_reject_locked_subject.cfg`: expected-failure locked-subject safe proposal rejection mutation.
- `SumeragiEngineProposalGate_bug_reject_higher_qc.cfg`: expected-failure higher-QC safe proposal rejection mutation.
- `SumeragiEngineProposalGate_bug_skip_validation_request.cfg`: expected-failure missing validation output mutation.
- `SumeragiEngineProposalGate_bug_skip_prepare_vote.cfg`: expected-failure missing prepare-vote output mutation.
- `SumeragiEngineProposalGate_bug_skip_prepare_phase.cfg`: expected-failure missing prepare-phase transition mutation.
- `SumeragiEnginePrepareQcGate.tla`: pure engine prepare-QC commit-vote gate model.
- `SumeragiEnginePrepareQcGate_fast.cfg`: CI-friendly engine prepare-QC gate check.
- `SumeragiEnginePrepareQcGate_bug_wrong_context.cfg`: expected-failure wrong round-context mutation.
- `SumeragiEnginePrepareQcGate_bug_wrong_quorum_policy.cfg`: expected-failure wrong quorum-policy mutation.
- `SumeragiEnginePrepareQcGate_bug_stale_view.cfg`: expected-failure stale prepare-view mutation.
- `SumeragiEnginePrepareQcGate_bug_committed_height.cfg`: expected-failure committed-height prepare mutation.
- `SumeragiEnginePrepareQcGate_bug_replay_prepare.cfg`: expected-failure prepare-QC replay mutation.
- `SumeragiEnginePrepareQcGate_bug_conflicting_prepare.cfg`: expected-failure conflicting prepare-QC mutation.
- `SumeragiEnginePrepareQcGate_bug_pending_finality.cfg`: expected-failure pending-finality prepare mutation.
- `SumeragiEnginePrepareQcGate_bug_reject_safe.cfg`: expected-failure safe prepare-QC rejection mutation.
- `SumeragiEnginePrepareQcGate_bug_missing_lock_record.cfg`: expected-failure missing lock/highest-QC record mutation.
- `SumeragiEngineCommitQcGate.tla`: pure engine commit-QC finality gate model.
- `SumeragiEngineCommitQcGate_fast.cfg`: CI-friendly engine commit-QC gate check.
- `SumeragiEngineCommitQcGate_bug_wrong_context.cfg`: expected-failure wrong round-context mutation.
- `SumeragiEngineCommitQcGate_bug_wrong_quorum_policy.cfg`: expected-failure wrong quorum-policy mutation.
- `SumeragiEngineCommitQcGate_bug_stale_view.cfg`: expected-failure stale commit-view mutation.
- `SumeragiEngineCommitQcGate_bug_committed_height.cfg`: expected-failure committed-height commit-QC mutation.
- `SumeragiEngineCommitQcGate_bug_pending_replay.cfg`: expected-failure pending-finality replay mutation.
- `SumeragiEngineCommitQcGate_bug_pending_conflict.cfg`: expected-failure pending-finality conflict mutation.
- `SumeragiEngineCommitQcGate_bug_commit_without_payload.cfg`: expected-failure missing-payload commit mutation.
- `SumeragiEngineCommitQcGate_bug_fetch_despite_payload.cfg`: expected-failure payload-available fetch mutation.
- `SumeragiEngineCommitQcGate_bug_reject_available.cfg`: expected-failure payload-available commit-QC rejection mutation.
- `SumeragiEngineCommitQcGate_bug_reject_missing_payload.cfg`: expected-failure missing-payload commit-QC rejection mutation.
- `SumeragiEngineCommitQcGate_bug_missing_highest_record.cfg`: expected-failure missing highest-QC record mutation.
- `SumeragiEnginePayloadAvailabilityGate.tla`: pure engine payload-availability gate model.
- `SumeragiEnginePayloadAvailabilityGate_fast.cfg`: CI-friendly engine payload-availability gate check.
- `SumeragiEnginePayloadAvailabilityGate_bug_skip_available_record.cfg`: expected-failure missing payload-availability record mutation.
- `SumeragiEnginePayloadAvailabilityGate_bug_commit_without_pending.cfg`: expected-failure payload-only commit mutation.
- `SumeragiEnginePayloadAvailabilityGate_bug_commit_mismatched_payload.cfg`: expected-failure mismatched-payload commit mutation.
- `SumeragiEnginePayloadAvailabilityGate_bug_drop_pending_on_mismatch.cfg`: expected-failure pending-finality drop on mismatch mutation.
- `SumeragiEnginePayloadAvailabilityGate_bug_reject_matching_payload.cfg`: expected-failure exact matching payload rejection mutation.
- `SumeragiEnginePayloadAvailabilityGate_bug_keep_pending_after_commit.cfg`: expected-failure stale pending-finality after commit mutation.
- `SumeragiEnginePayloadAvailabilityGate_bug_wrong_phase_after_commit.cfg`: expected-failure wrong post-commit phase mutation.
- `SumeragiEngineValidationResultGate.tla`: pure engine validation-result gate model.
- `SumeragiEngineValidationResultGate_fast.cfg`: CI-friendly engine validation-result gate check.
- `SumeragiEngineValidationResultGate_bug_accept_wrong_round.cfg`: expected-failure wrong-round validation callback mutation.
- `SumeragiEngineValidationResultGate_bug_accept_wrong_block_hash.cfg`: expected-failure wrong-block validation callback mutation.
- `SumeragiEngineValidationResultGate_bug_accept_no_inflight.cfg`: expected-failure no-in-flight/replayed validation callback mutation.
- `SumeragiEngineValidationResultGate_bug_accept_superseded.cfg`: expected-failure commit-superseded validation callback mutation.
- `SumeragiEngineValidationResultGate_bug_reject_current_valid.cfg`: expected-failure current valid-result rejection mutation.
- `SumeragiEngineValidationResultGate_bug_reject_current_invalid.cfg`: expected-failure current invalid-result rejection mutation.
- `SumeragiEngineValidationResultGate_bug_keep_validation.cfg`: expected-failure retained validation owner mutation.
- `SumeragiEngineValidationResultGate_bug_valid_emits_output.cfg`: expected-failure valid-result output mutation.
- `SumeragiEngineValidationResultGate_bug_skip_round_advance.cfg`: expected-failure invalid-result round-advance mutation.
- `SumeragiEngineValidationResultGate_bug_skip_new_view_vote.cfg`: expected-failure invalid-result missing NewView vote mutation.
- `SumeragiEngineValidationResultGate_bug_skip_advance_output.cfg`: expected-failure invalid-result missing AdvanceView mutation.
- `SumeragiEngineValidationResultGate_bug_wrong_phase.cfg`: expected-failure invalid-result wrong phase mutation.
- `SumeragiEngineValidationResultGate_bug_use_invalid_subject_despite_highest.cfg`: expected-failure highest-QC subject loss mutation.
- `SumeragiEngineValidationResultGate_bug_use_highest_without_highest.cfg`: expected-failure false highest-QC subject mutation.
- `SumeragiEngineValidationResultGate_bug_omit_highest_binding.cfg`: expected-failure missing highest-QC binding mutation.
- `SumeragiEngineValidationResultGate_bug_bind_highest_without_highest.cfg`: expected-failure spurious highest-QC binding mutation.
- `SumeragiEngineValidationResultGate_bug_drop_pending_finality.cfg`: expected-failure superseded-callback pending-finality drop mutation.
- `SumeragiEngineValidationResultGate_bug_overwrite_committed.cfg`: expected-failure superseded-callback committed-state overwrite mutation.
- `SumeragiEngineCommittedBlockGate.tla`: pure engine committed-block notification gate model.
- `SumeragiEngineCommittedBlockGate_fast.cfg`: CI-friendly engine committed-block gate check.
- `SumeragiEngineCommittedBlockGate_bug_skip_fresh_record.cfg`: expected-failure missing committed-height record mutation.
- `SumeragiEngineCommittedBlockGate_bug_reject_boundary_activation.cfg`: expected-failure boundary reconfiguration rejection mutation.
- `SumeragiEngineCommittedBlockGate_bug_activate_without_boundary.cfg`: expected-failure plain commit activation mutation.
- `SumeragiEngineCommittedBlockGate_bug_activate_non_boundary.cfg`: expected-failure non-boundary reconfiguration activation mutation.
- `SumeragiEngineCommittedBlockGate_bug_record_duplicate.cfg`: expected-failure duplicate committed-height record mutation.
- `SumeragiEngineCommittedBlockGate_bug_activate_duplicate.cfg`: expected-failure duplicate activation mutation.
- `SumeragiEngineCommittedBlockGate_bug_record_conflict.cfg`: expected-failure conflicting committed-height record mutation.
- `SumeragiEngineCommittedBlockGate_bug_activate_conflict.cfg`: expected-failure conflicting reconfiguration activation mutation.
- `SumeragiEngineCommittedBlockGate_bug_overwrite_conflict.cfg`: expected-failure conflicting committed-height overwrite mutation.
- `SumeragiValidatorSetTransition.tla`: validator-set activation safety model.
- `SumeragiValidatorSetTransition_fast.cfg`: CI-friendly reconfiguration check.
- `SumeragiValidatorSetTransition_bug_premature_activation.cfg`: expected-failure activation-without-boundary-finality mutation.
- `SumeragiValidatorSetTransition_bug_premature_new_cert.cfg`: expected-failure new-set-before-activation mutation.
- `SumeragiValidatorSetTransition_bug_mixed_cert.cfg`: expected-failure mixed-set certificate mutation.
- `SumeragiCertifiedRecovery.tla`: certified commit-QC payload recovery safety model.
- `SumeragiCertifiedRecovery_fast.cfg`: CI-friendly certified-recovery check.
- `SumeragiCertifiedRecovery_bug_commit_without_payload.cfg`: expected-failure commit-without-payload mutation.
- `SumeragiCertifiedRecovery_bug_mismatched_payload.cfg`: expected-failure mismatched-payload mutation.
- `SumeragiCertifiedRecovery_bug_conflicting_finality.cfg`: expected-failure conflicting-finality mutation.
- `SumeragiViewChangeSafety.tla`: view-change/highest-QC/locked-proposal safety model.
- `SumeragiViewChangeSafety_fast.cfg`: CI-friendly view-change safety check.
- `SumeragiViewChangeSafety_bug_stale_new_view.cfg`: expected-failure stale-new-view mutation.
- `SumeragiViewChangeSafety_bug_unsafe_proposal.cfg`: expected-failure unsafe-proposal mutation.
- `SumeragiViewChangeSafety_bug_lock_overwrite.cfg`: expected-failure lock-overwrite mutation.
- `SumeragiViewChangeSafety_bug_highest_regression.cfg`: expected-failure highest-QC regression mutation.
- `SumeragiValidationGate.tla`: asynchronous proposal-validation callback ownership model.
- `SumeragiValidationGate_fast.cfg`: CI-friendly validation-callback safety check.
- `SumeragiValidationGate_bug_unknown_result.cfg`: expected-failure unknown-validation-result mutation.
- `SumeragiValidationGate_bug_completed_replay.cfg`: expected-failure completed-result-replay mutation.
- `SumeragiValidationGate_bug_timeout_inflight.cfg`: expected-failure timeout-retains-in-flight mutation.
- `SumeragiValidationGate_bug_invalid_replay.cfg`: expected-failure duplicate-invalid-result mutation.
- `SumeragiCertificateAdmission.tla`: fail-closed certificate-admission safety model.
- `SumeragiCertificateAdmission_fast.cfg`: CI-friendly certificate-admission safety check.
- `SumeragiCertificateAdmission_bug_wrong_context.cfg`: expected-failure wrong-context certificate mutation.
- `SumeragiCertificateAdmission_bug_stale_prepare_commit.cfg`: expected-failure stale prepare/commit certificate mutation.
- `SumeragiCertificateAdmission_bug_future_height.cfg`: expected-failure future-height certificate mutation.
- `SumeragiCertificateAdmission_bug_committed_height.cfg`: expected-failure committed-height certificate mutation.
- `SumeragiHighestQcSelection.tla`: deterministic highest-QC selection model.
- `SumeragiHighestQcSelection_fast.cfg`: CI-friendly highest-QC selection check.
- `SumeragiHighestQcSelection_bug_height_priority.cfg`: expected-failure height-priority comparator mutation.
- `SumeragiHighestQcSelection_bug_phase_rank.cfg`: expected-failure phase-rank comparator mutation.
- `SumeragiHighestQcSelection_bug_subject_tie.cfg`: expected-failure missing subject tie-break mutation.
- `SumeragiHighestQcSelection_bug_non_new_view.cfg`: expected-failure non-new-view inclusion mutation.
- `SumeragiFrontierRecovery.tla`: focused frontier recovery model.
- `SumeragiFrontierRecovery_fast.cfg`: smaller CI-friendly frontier parameter set.
- `SumeragiFrontierRecovery_deep.cfg`: larger frontier backlog/window/view bound set.
- `SumeragiFrontierRecovery_wide.cfg`: wider frontier bound set used by formal CI.
- `SumeragiFrontierRecovery_bug_stale_owner.cfg`: expected-failure stale-owner mutation.
- `SumeragiFrontierRecovery_bug_vote_queue.cfg`: expected-failure vote-queue mutation.
- `SumeragiFrontierRecovery_bug_payload_recovery.cfg`: expected-failure payload-recovery mutation.
- `SumeragiFrontierRecovery_bug_retransmit_followthrough.cfg`: expected-failure retransmit-follow-through mutation.
- `SumeragiFrontierRecovery_bug_future_promotion.cfg`: expected-failure future-promotion mutation.
- `SumeragiFrontierRecovery_bug_future_reanchor_clear.cfg`: expected-failure reanchor-clear mutation.
- `SumeragiFrontierRecovery_bug_future_evidence_drop.cfg`: expected-failure future-evidence drop mutation.
- `SumeragiFrontierRecovery_bug_promotion_reset.cfg`: expected-failure promotion-reset mutation.
- `SumeragiFrontierRecovery_bug_future_stale_owner.cfg`: expected-failure future stale-owner mutation.
- `SumeragiFrontierRecovery_bug_progress_touch.cfg`: expected-failure pending progress-touch mutation.
- `SumeragiFrontierRecovery_bug_height_only_recovery.cfg`: expected-failure height-only stale recovery mutation.
- `SumeragiFrontierRecovery_tlc_small.cfg`: small TLC cross-check config.
- `.github/workflows/nightly_sumeragi_formal.yml`: scheduled/manual longer-bound
  frontier check using `frontier-nightly`.

## Properties

Invariants:
- `TypeInvariant`
- `CommitImpliesQuorum`
- `CommitImpliesStakeQuorum`
- `CommitImpliesDelivered`
- `DeliverImpliesEvidence`

Fork-safety invariants:
- `TypeInvariant`
- `HonestCommitVotesSingleBranch`
- `CommitCertificateImpliesCountQuorum`
- `CommitCertificateImpliesStakeQuorum`
- `CommitCertificateImpliesHonestSupport`, which requires every modeled commit
  certificate to contain enough honest support after discounting the Byzantine
  budget.
- `NoConflictingCommitCertificates`, which is the direct same-height finality
  property for the two modeled branches.

Quorum-policy invariants:
- `TypeInvariant`
- `CountMatchesStrictSupermajority`
- `CountRejectsOverValidatorCount`
- `StakeMatchesStrictSupermajority`
- `ExactTwoThirdsStakeRejected`
- `StakeRejectsInvalidInputs`
- `StakeRejectsOverTotal`

Commit-root consistency invariants:
- `TypeInvariant`
- `SelectedRootMatchesSpec`
- `SelectedEvidenceMatchesSpecRoot`
- `AcceptedMatchesSpec`
- `MixedRootsCannotSatisfyPermissionedQuorum`
- `MixedRootsCannotSatisfyStakeQuorum`
- `WrongContextCannotSatisfyRootQuorum`
- `ValidationRootMismatchRejected`
- `ValidatedMatchesSpec`

Commit-pipeline recovery-gate invariants:
- `TypeInvariant`
- `LocalCommitQcFormationMatchesSpec`
- `LocalQuorumFormsBeforePeerRecovery`
- `CommitQcObservationIsPreserved`
- `MissingCommitQcRecoveryMatchesSpec`
- `FreshLocalVoteDoesNotRecover`
- `RecoveryRequiresLocalVote`
- `RecoveryRequiresCommitQcAbsent`
- `RecoveryRequiresPayloadLocal`
- `RecoveryRequiresValidPending`
- `RecoveryRequiresTipExtension`
- `QuorumRetransmitMatchesSpec`
- `QuorumRetransmitUsesMissingSignerTargets`
- `CollectorSubsetNeverOverridesQuorumTargets`
- `EmptyVoteSetNeverRebroadcasts`
- `CachedCommitQcSkipsRebroadcast`

Commit-evidence replay-gate invariants:
- `TypeInvariant`
- `ReplayMatchesSpec`
- `InactivePendingNeverReplays`
- `CooldownSuppressesReplay`
- `NoEvidenceNeverReplays`
- `RemoteTargetsRequired`
- `FirstEvidenceReplays`
- `ProgressReplays`
- `StalledPositiveEvidenceRetries`
- `VoteEvidenceUsesVoteReplay`
- `CommitQcUsesCommitCertReplay`
- `PayloadFallbackNeverUsed`
- `ReplayTargetsExcludeLocal`
- `DuplicateExplicitTargetsAreDeduped`

Precommit vote-gate invariants:
- `TypeInvariant`
- `EmittedMatchesSpec`
- `RejectedMatchesSpec`
- `SafeCandidatesAreAccepted`
- `UnsafeCandidatesAreRejected`
- `InvalidValidationNeverEmits`
- `ObserversNeverEmit`
- `DuplicateSameSlotNeverEmits`
- `UnsupersededConflictNeverEmits`
- `OlderConflictCannotUseQuorumCompletion`
- `LockedConflictsNeverEmit`
- `PermittedConflictCasesCanEmit`
- `PermittedLockCasesCanEmit`

Proposal assembly-gate invariants:
- `TypeInvariant`
- `AssembledMatchesSpec`
- `DeferredMatchesSpec`
- `SafeCandidatesAreAssembled`
- `UnsafeCandidatesAreDeferred`
- `ObserversNeverAssemble`
- `ActiveLocalVoteConflictNeverAssembles`
- `PendingVoteVerificationNeverAssembles`
- `MissingHighestQcNeverAssembles`
- `NonExtendingHighestQcNeverAssembles`
- `SplitVoteLockNeverAssembles`
- `CommittedEdgeConflictNeverAssembles`
- `PermittedVoteHistoryCasesAssemble`
- `PermittedLockedParentCasesAssemble`

Engine tick gate invariants:
- `TypeInvariant`
- `EveryTickAdvancesView`
- `EveryTickSignsNewView`
- `EveryTickEmitsAdvanceView`
- `EveryTickEntersProposalPhase`
- `TicksClearInflightValidation`
- `TicksPreservePendingFinality`
- `HighestTicksUseHighestSubject`
- `NoHighestTicksUseZeroSubject`
- `HighestTicksBindHighestQc`
- `NoHighestTicksDoNotBindHighestQc`
- `SignedTicksHaveConsistentOutputs`

Engine proposal-ingress gate invariants:
- `TypeInvariant`
- `AcceptedMatchesSpec`
- `IgnoredMatchesSpec`
- `SafeProposalsValidate`
- `SafeProposalsSignPrepare`
- `SafeProposalsEnterPreparePhase`
- `UnsafeProposalsAreIgnored`
- `WrongPhaseNeverAccepted`
- `WrongRoundNeverAccepted`
- `IncompatibleHighestNeverAccepted`
- `LockedConflictWithoutUnlockNeverAccepted`
- `AcceptedProposalsRequestValidation`
- `AcceptedProposalsSignPrepareVote`
- `AcceptedProposalsEnterPrepare`
- `IgnoredProposalsDoNotEmit`
- `OutputsStayTogether`

Engine prepare-QC gate invariants:
- `TypeInvariant`
- `SignedMatchesSpec`
- `IgnoredMatchesSpec`
- `SafePrepareQcsSign`
- `UnsafePrepareQcsAreIgnored`
- `WrongContextNeverSigns`
- `WrongQuorumPolicyNeverSigns`
- `StaleViewNeverSigns`
- `CommittedHeightNeverSigns`
- `ReplayPrepareNeverSigns`
- `ConflictingPrepareNeverSigns`
- `PendingFinalityNeverSigns`
- `SignedPrepareRecordsLock`
- `SignedPrepareRecordsHighest`
- `IgnoredPrepareDoesNotMutateLock`
- `LockAndHighestFollowSigned`

Engine commit-QC gate invariants:
- `TypeInvariant`
- `CommittedMatchesSpec`
- `FetchedMatchesSpec`
- `IgnoredMatchesSpec`
- `SafeAvailableCommitQcsCommit`
- `SafeMissingPayloadCommitQcsFetch`
- `UnsafeCommitQcsAreIgnored`
- `WrongContextNeverAccepted`
- `WrongQuorumPolicyNeverAccepted`
- `StaleViewNeverAccepted`
- `CommittedHeightNeverAccepted`
- `PendingReplayNeverAccepted`
- `PendingConflictNeverAccepted`
- `NoCommitWithoutPayload`
- `NoFetchWhenPayloadAvailable`
- `AcceptedCommitQcsRecordHighest`
- `IgnoredCommitQcsDoNotRecordHighest`
- `HighestFollowsAcceptedCommitQcs`

Engine payload-availability gate invariants:
- `TypeInvariant`
- `EveryPayloadIsRecordedAvailable`
- `CommittedMatchesSpec`
- `IgnoredMatchesSpec`
- `PayloadOnlyNeverCommits`
- `MismatchedPayloadsNeverCommit`
- `MatchingPayloadCommits`
- `MismatchedPayloadsPreservePending`
- `MatchingPayloadClearsPending`
- `CommitClearsPending`
- `CommitEntersProposalPhase`
- `IgnoredPayloadsDoNotClearPending`

Engine committed-block gate invariants:
- `TypeInvariant`
- `RecordedMatchesSpec`
- `ActivatedMatchesSpec`
- `IgnoredMatchesSpec`
- `FreshCommitNotificationsRecord`
- `FreshBoundaryReconfigurationActivates`
- `PlainCommitNotificationsNeverActivate`
- `NonBoundaryReconfigurationNeverActivates`
- `DuplicateNotificationsAreIdempotent`
- `ConflictingNotificationsAreIgnored`
- `ConflictsNeverOverwrite`
- `ActivationRequiresFreshBoundaryRecord`
- `NoDuplicateOrConflictRecord`

Validator-set transition invariants:
- `TypeInvariant`
- `ActivationRequiresOldBoundaryFinality`
- `NewCertificatesStartAtActivationHeight`
- `NewCertificatesRequireActivation`
- `OldCertificatesStopBeforeActivationHeight`
- `NoMixedValidatorSetCertificates`
- `NoHeightCommittedByMultipleValidatorSets`

Certified recovery invariants:
- `TypeInvariant`
- `PendingFinalityRequiresCommitQc`
- `CommitRequiresCommitQc`
- `NoCommitWithoutPayload`
- `CommitRequiresMatchingPayload`
- `NoMismatchedPayloadAccepted`
- `NoConflictingFinality`

View-change safety invariants:
- `TypeInvariant`
- `CurrentViewNeverRewinds`
- `StaleNewViewCertificatesRejected`
- `HighestQcDominatesAcceptedEvidence`
- `HighestQcNeverRegresses`
- `UnsafeProposalsRejected`
- `ConflictingLockOverwritesRejected`

Validation-gate invariants:
- `TypeInvariant`
- `UnknownValidationDoesNotAdvance`
- `CompletedValidationReplayDoesNotAdvance`
- `LateValidationAfterTimeoutDoesNotAdvance`
- `TimeoutClearsInflight`
- `InvalidValidationAdvancesAtMostOnce`
- `NoStaleInflightAfterViewAdvance`
- `CompletedValidationClearsInflight`

Certificate-admission invariants:
- `TypeInvariant`
- `WrongContextCertificatesIgnored`
- `StalePrepareCommitCertificatesIgnored`
- `FutureHeightCertificatesIgnored`
- `CommittedHeightCertificatesIgnored`
- `LockedCertificateMatchesCurrentView`
- `CommittedHeightHasNoPendingFinality`

Highest-QC selection invariants:
- `TypeInvariant`
- `SelectedAEqualsSpecMax`
- `SelectedBEqualsSpecMax`
- `SelectedOnlyFromNewViewCertificates`
- `EqualObservedSelectsEqualQc`
- `HeightPriorityDominatesView`
- `PhaseRankDominatesSubject`
- `SubjectTieBreakDominatesArrivalOrder`

Temporal property:
- `EventuallyCommit` (`[] (gst => <> committed)`), with post-GST fairness encoded
  operationally in `Next` (timeout/fault preemption guards on enabled
  progress actions). This keeps the model checkable with Apalache 0.52.x, which
  does not support `WF_` fairness operators inside checked temporal properties.

Frontier recovery invariants:
- `TypeInvariant`
- `CommitImpliesVoteQuorum`
- `CommitImpliesPayloadAvailability`
- `VoteBackedNotDroppedAsZeroEvidenceZombie`
- `PostGstVoteBackedFrontierHasProgress`, which rules out a terminal
  post-GST state where `pending /\ voteBacked /\ ~committed` has no recovery,
  commit, retransmit, rotation, or bounded-drop transition.
- `FuturePromotionReadyHasProgress`, which rules out a terminal post-GST
  state where the current pending wrapper has cleared for future evidence but
  the future slot cannot be promoted.
- `StaleRecoveryOwnerHasClearProgress`, which requires stale current frontier
  ownership to expose a clear transition once the relevant subject view has
  rotated.
- `VoteQueueBacklogHasDrainProgress`, which requires a queued-vote backlog on
  a fresh active frontier to expose a real drain transition instead of being
  masked by unrelated progress bookkeeping.
- `MissingPayloadHasRecoveryProgress`, which requires a vote-backed active
  frontier with a drained vote queue and missing payload to expose a payload
  recovery transition.
- `QuorumWindowHasRetransmitProgress`, which requires an expired
  quorum-reschedule window to expose a quorum retransmit transition before
  bounded rotation/drop can follow.
- `RetransmitHasFollowthroughProgress`, which requires a vote-backed frontier
  that already retransmitted quorum evidence to expose the deterministic
  rotation or view-bound clear follow-through.
- `FutureEvidenceHasReanchorProgress`, which requires concrete future frontier
  evidence to expose the current-wrapper clear step before promotion.
- `FutureEvidencePreservedUntilPromotion`, which requires observed future
  frontier evidence to remain represented by the concrete future slot until it
  is promoted.
- `FuturePromotionResetsActiveProgress`, which requires a freshly promoted
  second slot to start with cleared active progress, validation, vote, QC,
  recovery, quorum-window, and view flags.
- `PendingProgressEventsTouchAge`, which requires every modeled pending
  progress event to reset the abstract progress age.
- `StaleRecoveryUnlockIsViewScoped`, which requires any stale recovery unlock
  to have rotated at least the pending block's subject view.

Frontier recovery temporal property:
- `PostGstVoteBackedFrontierEventuallyResolves`: after GST, every unresolved
  active vote-backed pending frontier state eventually clears its pending
  wrapper.
- `RecoveredPayloadEventuallyAdvances`: a vote-backed frontier state that has
  recovered the payload cannot remain pending forever without commit,
  retransmit, reanchor, or rotation.
- `QuorumRetransmitEventuallyLeavesPending`: once quorum retransmit has fired
  for a vote-backed frontier state, the pending wrapper must eventually clear.
- `FutureFrontierEvidenceEventuallyReanchors`: later frontier/new-view evidence
  must be consumed through reanchor and future-slot promotion.
- `FuturePromotionReadyEventuallyPromotes`: a cleared current wrapper with
  promotion-ready future evidence must eventually promote that future slot.
- `PromotedSecondSlotEventuallyClears`: after promotion, the second slot must
  satisfy the same vote-backed pending-clear property as the original active
  slot.

## Assumption map

The fork-safety model is intentionally finite. These are the implementation
surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `honestCommitA`, `honestCommitB`, `byzCommitA`, `byzCommitB` | Commit-vote signer tracking, duplicate/conflicting vote rejection, and double-vote evidence in `crates/iroha_core/src/sumeragi/main_loop/votes.rs` plus coverage such as `conflicting_vote_does_not_override_first` and `conflicting_commit_vote_across_views_is_dropped_for_same_signer_peer` in `main_loop/tests.rs`. |
| `CommitQuorum`, `UseStakeQuorum`, `StakeQuorum` | Strict permissioned and NPoS quorum policy in `crates/iroha_data_model/src/block/consensus.rs` and the live commit-certificate aggregation/validation path. |
| `lockedBranch`, `lockView`, `PrepareQc` | Locked-QC acceptance rules in `crates/iroha_core/src/sumeragi/main_loop/locked_qc.rs` and the pure engine's `proposal_satisfies_lock(...)`. |
| `commitCerts` | Commit-certificate formation and finality conflict rejection in the collector/receiver path; the pure engine bridge coverage includes `conflicting_blocks_cannot_both_commit_at_same_height` and `committed_block_notifications_do_not_overwrite_conflicting_height`. |

The quorum-policy model is intentionally finite. These are the implementation
surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `PermissionedThreshold`, `CountSpecSatisfied` | `QuorumPolicy::permissioned_threshold(...)` and `QuorumPolicy::is_satisfied_by_count(...)` in `crates/iroha_data_model/src/block/consensus.rs`. |
| `CountRejectsOverValidatorCount` | Count quorum must reject signer counts above the active validator count even if they exceed the threshold. Bridge coverage includes `quorum_policy_enforces_strict_supermajority_boundaries`. |
| `StakeSpecSatisfied` | `QuorumPolicy::is_satisfied_by_stake(...)` accepts only signed stake strictly greater than two thirds of total stake. |
| `StakeRejectsInvalidInputs`, `StakeRejectsOverTotal` | NPoS stake quorum rejects missing/negative stake, zero/negative total stake, signed stake above total stake, and checked-multiply overflow. The same bridge test exercises these fail-closed boundaries. |

The commit-pipeline recovery-gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `localQuorumStale`, `localQuorumFresh` | `process_commit_candidates_with_trigger_inner(...)` calls `try_form_qc_from_votes(...)` before peer recovery when cached commit votes meet `min_votes_for_commit`; bridge coverage includes `commit_pipeline_forms_local_commit_qc_before_missing_commit_qc_recovery`. |
| `stalledLocalVote`, `freshLocalVote` | Missing commit-QC recovery is armed only after the pending fast-path timeout for a locally voted pending block; bridge coverage includes `commit_pipeline_arms_missing_commit_qc_recovery_for_stalled_local_vote`. |
| `commitQcAlreadyObserved` | Existing commit-QC evidence must suppress peer recovery and preserve the pending block's commit-QC marker. |
| `missingLocalData`, `invalidPending`, `noLocalVote`, `offTip` | Recovery requires local DA payload availability, valid pending state, local commit-vote emission, and extension of the committed tip. |
| `nearQuorumRetransmit`, `collectorDecoyRetransmit` | `rebroadcast_block_votes(..., target_missing_only = true)` derives quorum missing-signer targets through `quorum_retransmit_targets_for_missing_votes(...)`; bridge coverage includes `commit_pipeline_rebroadcasts_cached_votes_to_quorum_retransmit_targets`. |
| `noVotesRetransmit`, `hasCommitQcRetransmit` | Empty vote logs and already cached commit QCs skip near-quorum rebroadcast. |

The commit-evidence replay-gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `missingPending`, `wrongRound`, `abortedPending` | `maybe_replay_known_block_commit_evidence(...)` returns before replay when no active pending block matches the exact height/view or the pending block is aborted. Bridge coverage includes `known_block_commit_evidence_replay_skips_aborted_pending_tracked`. |
| `cooldownVotes` | `block_sync_rebroadcast_log.allow(...)` suppresses repeated replay during the per-block cooldown. Bridge coverage includes `known_block_commit_evidence_replay_skips_during_cooldown`. |
| `firstVotesRemote`, `firstCommitQcRemote` | The first positive commit evidence snapshot may replay and records the pending block's replay state. Bridge coverage includes `known_block_commit_evidence_replay_skips_payload_fallback_without_roster` and `known_block_commit_qc_replay_targets_snapshot_roster`. |
| `stalledVotesRemote`, `stalledCommitQcRemote` | Stalled positive evidence is allowed to retry once the cooldown expires. Bridge coverage includes `known_block_commit_evidence_replay_retries_stalled_commit_evidence_after_cooldown`. |
| `voteCountProgressRemote`, `commitQcProgressRemote`, `viewProgressRemote` | `PendingBlock::should_replay_commit_evidence(...)` treats higher vote count, newly cached commit QC, or view change as replay progress; unit coverage includes `commit_evidence_replay_advances_on_progress`. |
| `firstNoEvidenceRemote`, `sameZeroNoProgress` | Zero-evidence snapshots must not schedule outbound vote/certificate work or payload fallback. Bridge coverage includes `commit_evidence_replay_cooldown_does_not_fallback_to_payload`. |
| `localOnlyVoteTargets`, `localOnlyCommitQcTargets` | Explicit target sets that collapse to the local peer return `false` without outbound work. Bridge coverage includes `known_block_commit_evidence_replay_returns_false_for_local_only_explicit_targets` and `known_block_commit_qc_replay_returns_false_for_local_only_explicit_targets`. |
| `duplicateVoteTargets` | `rebroadcast_block_votes_to_targets(...)` filters local targets and deduplicates explicit remotes. Bridge coverage includes `known_block_commit_evidence_replay_deduplicates_explicit_vote_targets`. |
| `VoteEvidenceUsesVoteReplay`, `CommitQcUsesCommitCertReplay`, `PayloadFallbackNeverUsed` | Vote replay uses `QcVote`, cached commit-QC replay uses `CommitCert`, and neither path rebuilds `BlockCreated` or `BlockSyncUpdate` payload traffic. Bridge coverage includes `known_block_commit_evidence_replay_uses_explicit_targets`, `known_block_commit_qc_replay_targets_snapshot_roster`, and `commit_evidence_replay_cooldown_does_not_fallback_to_payload`. |

The block-sync recovery-gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `requestedStalePayload`, `staleNoRequest` | `handle_block_sync_update(...)` and `handle_block_created_with_preserve_policy(...)` admit stale-view recovery only for missing-block requests or commit evidence; bridge coverage includes `block_sync_update_accepts_stale_view_when_missing_block_requested` and `block_sync_update_drops_stale_view_without_missing_request`. |
| `staleCommitVotes`, `staleCommitQc` | Vote/QC-backed stale contiguous recovery enters `BlockSyncRecoveryMode::CommitEvidenceRepair` and may become the authoritative owner. Bridge coverage includes `block_sync_update_accepts_stale_view_with_commit_votes` and `block_sync_update_accepts_stale_view_with_commit_qc`. |
| `abortedPayloadOnly`, `abortedCommitQc` | Payload-only sparse updates keep aborted placeholders inactive, while commit-QC evidence may revive them and preserve the observed QC epoch. Bridge coverage includes `block_sync_update_keeps_aborted_next_height_payload_sparse_without_commit_evidence` and `block_sync_update_revives_aborted_next_height_payload_with_commit_qc`. |
| `sparseNextHeight`, `unknownFrontierVoteOnly` | Sparse next-height payload repair and unknown-frontier vote-only updates track missing commit-QC repair instead of silently becoming complete. Bridge coverage includes `block_sync_update_tracks_missing_commit_qc_for_next_height_sparse_payload_recovery` and `block_sync_update_tracks_missing_qc_for_unknown_frontier_vote_only_update`. |
| `payloadOnlyStaleInflight`, `certifiedStaleInflight` | Payload-only exact repair does not clear stale commit inflight or steal owner state, but certified repair may bypass stale inflight and clear it. Bridge coverage includes `block_sync_update_accepts_stale_exact_frontier_payload_repair_with_da` and `block_sync_update_commit_qc_bypasses_stale_commit_inflight_frontier_owner`. |
| `sameHeightRawQuorumConflict`, `sameHeightCertifiedConflict` | Raw block-signature quorum can hydrate a passive retained branch, while certified evidence may supersede a stale same-height frontier owner. Bridge coverage includes `block_sync_update_same_height_conflict_with_block_quorum_stays_passive_without_certified_evidence` and `block_sync_update_commit_qc_supersedes_stale_same_height_frontier_owner`. |
| `cachedCommitQcPayload`, `unvalidatedCommitQc`, `unrequestedFuture` | Cached commit-QC payload recovery remains authoritative, unvalidated sidecar QC cannot advance lock/highest-QC state, and unrequested future-height updates are dropped. Bridge coverage includes `block_sync_payload_with_cached_commit_qc_supersedes_lock_conflicting_stale_frontier_owner`, `block_sync_update_does_not_advance_qc_for_unvalidated_payload`, and `block_sync_update_drops_unrequested_future_height_beyond_active_frontier_lanes`. |

The native AMX attestation-gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `nonNativePlan`, `emptyRoster` | `native_amx_receipt_for_plan(...)` returns `Ok(None)` for non-AMX plans and fails closed when `native_amx_vote_roster()` is empty. |
| `noPrepareVotes`, `prepareBelowQuorum`, `commitWithoutPrepare` | Missing prepare quorum schedules `NativeAmxMessage::PrepareRequest` and does not request commit attestations yet. |
| `prepareQuorumNoCommitVotes`, `prepareQuorumCommitBelowQuorum` | Once prepare quorum exists, missing commit quorum schedules `NativeAmxMessage::CommitRequest`. |
| `fullQuorumSingleLeg`, `fullQuorumMultiLeg`, `oneLegPendingMultiLeg` | A receipt is sealed only when every participant leg has both prepare and commit QCs; any pending leg defers the whole native AMX proposal batch without a partial receipt. |
| `duplicatePrepareSigner`, `duplicateCommitSigner`, `wrongPrepareBody`, `wrongCommitBody`, `outsiderPrepareSigner`, `outsiderCommitSigner` | `NativeAmxSessionCache::insert_vote(...)` and `aggregate_votes_to_qc(...)` reject duplicate exact-body signers, wrong attestation bodies, and signers outside the validator set. Bridge coverage includes `session_cache_rejects_duplicate_signer` and `aggregate_votes_to_qc_rejects_bad_vote_sets`. |
| `unsortedQuorumVotes` | `aggregate_votes_to_qc(...)` projects votes into validator-set order before building the bitmap and BLS aggregate. Bridge coverage includes `aggregate_votes_to_qc_orders_votes_by_validator_set`. |
| `retriedHeightSameSigner`, `differentParticipantSameSigner` | The session cache scopes duplicate checks to exact attestation bodies, so retried heights and distinct participant legs do not collide. Bridge coverage includes `session_cache_allows_same_signer_for_retried_body` and `session_cache_allows_same_signer_for_different_participant_legs`. |

The native AMX journal-replay model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `nativePutReplay`, `singlePutReplay` | `QueuePlanJournalRecordV1` stores the full `RoutingPlan`, and replay returns the same plan variant. Bridge coverage includes `native_amx_queue_journal_replays_plan_after_restart`. |
| `participantOrder`, `participantDedup` | `RoutingPlan::native_amx(...)` sorts and deduplicates participant legs by dataspace and lane before computing the plan digest. Bridge coverage includes `mixed_domain_write_targets_across_dataspaces_build_native_amx_plan`. |
| `digestPreserved`, `gossipPayloadPreserved`, `entrypointPreserved` | Journal records persist `routing_plan`, `gossip_payload`, and `entrypoint`; `plan_digest()` is derived from the stored routing plan. Bridge coverage includes `journal_replays_puts_and_removes` plus native AMX routing tests. |
| `removeExactDigest`, `removeOtherDigest`, `readmitSameHashNewDigest` | `QueuePlanJournal::replay()` keys live entries by `(signed_transaction_hash, plan_digest)`, so removes tombstone only the exact plan digest. |
| `duplicateSameKeyLastWins` | Replayed puts are inserted into a `BTreeMap`, so later puts for the same `(hash, digest)` replace earlier records. |
| `unsupportedVersionIgnored` | Replay ignores records whose `version` is not `QUEUE_PLAN_JOURNAL_VERSION`. |
| `compactionKeepsLive`, `compactionDropsRemoved` | `compact_if_needed()` replays the journal and rewrites only live `Put` frames. |
| `tornPayloadTailPreservesPrior`, `tornLengthTailPreservesPrior` | `QueuePlanJournal::open()` calls `repair_incomplete_tail(...)`, preserving complete prefix frames while truncating incomplete tails. Bridge coverage includes `journal_open_truncates_torn_payload_tail_before_append`. |

The precommit vote-gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `invalidValidation` | `emit_precommit_vote(...)` rejects any pending block whose `ValidationStatus` is not `Valid`; bridge coverage includes `emit_precommit_vote_requires_validated_pending`. |
| `observer`, `notInTopology` | Observer and topology-membership guards in `emit_precommit_vote(...)` prevent non-voting peers from signing local precommits. |
| `duplicateSameSlot` | `local_same_slot_vote(...)` prevents duplicate local precommit votes for the same height/view/epoch. |
| `unsupersededConflict`, `supersededConflict` | Same-height local vote history is enforced by `local_conflicting_slot_vote(...)`, `new_view_qc_supersedes_same_height_vote_conflict(...)`, and stale-vote rotation checks; bridge coverage includes `precommit_vote_rejects_newer_view_after_conflict` and the NEW_VIEW retry regressions in `main_loop/tests.rs`. |
| `candidateCompletesNewerQuorum`, `olderConflictCompletesQuorum` | `candidate_commit_quorum_completes_with_local_vote(...)` can unblock only newer conflicting candidates that complete quorum; bridge coverage includes `precommit_vote_allows_newer_conflict_when_local_vote_completes_quorum` and `precommit_vote_rejects_older_conflict_even_when_local_vote_would_complete_quorum`. |
| `lockedSameHeightConflict`, `missingLockedPayloadOldView`, `missingLockedPayloadNewerView`, `nonExtendingLockedChain`, `extendsLockedChain` | Locked-QC checks in `emit_precommit_vote(...)` and `qc_satisfies_locked_with_lookup(...)` reject same-height locked conflicts, require missing locked payload recovery at the same/older view, allow newer-view override, and require chain extension; bridge coverage includes `precommit_vote_skips_when_block_conflicts_with_locked_chain`, `emit_precommit_vote_requests_missing_locked_payload_before_skipping`, `emit_precommit_vote_allows_newer_view_when_locked_payload_missing`, and `precommit_vote_allows_when_block_extends_locked_chain`. |

The proposal assembly-gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `observer`, `notLeader` | Local proposer eligibility guards in `assemble_and_broadcast_proposal(...)`; bridge coverage includes `observer_assemble_proposal_returns_false`. |
| `activeLocalVoteConflict` | Same-height local vote history blocks fresh proposal assembly before proposal cache or slot-observed state is mutated; bridge coverage includes `assemble_proposal_defers_when_candidate_conflicts_with_local_vote_history`. |
| `staleRetiredPriorVote`, `newViewSupersedesLocalVote` | Stale retired vote history and accepted new-view supersession unblock fresh proposals; bridge coverage includes `assemble_proposal_allows_stale_retired_prior_view_local_vote_history` and the raw vote-lock supersession regressions in `main_loop/tests.rs`. |
| `pendingVoteVerification` | Pending same-height vote verification defers proposal assembly until the conflict surface is known; bridge coverage includes `assemble_proposal_defers_while_same_height_vote_verification_is_pending`. |
| `missingHighestQc` | Missing highest-QC payloads arm exact frontier repair and suppress proposal messages; bridge coverage includes `assemble_proposal_defers_when_highest_qc_block_missing`. |
| `regressedHighestReplacedByLock`, `lockedChainExtends`, `nonExtendingHighestQc` | Highest-QC and locked-chain compatibility in `highest_qc_extends_locked(...)`, locked fallback, and lock-lag range-pull recovery; bridge coverage includes `pacemaker_uses_locked_qc_when_selected_highest_qc_regresses`, `assemble_proposal_reanchors_lock_lag_highest_qc_catchup`, and `highest_qc_extends_locked_rejects_missing_highest`. |
| `splitSameHeightVotesNonViable` | Split same-height vote locks make the fresh branch non-viable and force recovery/defer instead of proposal assembly; bridge coverage includes `fresh_proposal_defers_when_split_same_height_votes_make_new_branch_non_viable`. |
| `committedEdgeHighestConflict` | Highest-QC evidence conflicting with the committed edge is suppressed instead of producing a fresh proposal; bridge coverage includes `assemble_proposal_suppresses_committed_edge_highest_qc_conflict`. |

The engine tick gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `noHighestIdle` | `ConsensusEngine::on_tick(...)` advances view, signs a `NewView` vote with `zero_subject()`, and emits `AdvanceView` when no highest QC exists. Bridge coverage includes `prepare_and_commit_qcs_from_previous_view_are_ignored_after_timeout`. |
| `highestIdle` | `on_tick(...)` uses `qc_subject(highest_qc)` and carries `highest_qc` when local highest-QC state exists. Bridge coverage includes `pending_finality_survives_timeout_and_view_change_noise`. |
| `validationNoHighest`, `validationWithHighest` | Ticks clear the pure engine's `validating` owner so late validation callbacks cannot force an extra view change. Bridge coverage includes `timeout_clears_inflight_validation_before_late_failure_arrives` and `tick_binds_highest_qc_and_clears_inflight_validation`. |
| `pendingFinalityWithHighest` | Ticks leave `pending_finality` intact across view changes while still binding highest-QC evidence into the `NewView` vote. Bridge coverage includes `pending_finality_survives_timeout_and_view_change_noise`. |

The engine NewView-QC gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `safeNoHighest` | `ConsensusEngine::on_new_view_qc(...)` accepts a compatible newer-view certificate without carried highest-QC evidence and emits `AdvanceView`. Bridge coverage includes `stale_new_view_certificate_cannot_update_highest_qc_or_rewind_round` for the accepted advance before the stale replay. |
| `safeImprovingHighest`, `pendingSafeImprovingHighest` | Accepted NewView QCs call `record_highest_qc(...)` when the carried compatible QC improves local state, including while pending finality survives view-change noise. Bridge coverage includes `new_view_certificate_rejects_incompatible_highest_qc` and `pending_finality_survives_timeout_and_view_change_noise`. |
| `safeLowerHighest` | Accepted newer-view certificates with lower carried QC evidence must not regress `highest_qc`. Bridge coverage includes `accepted_new_view_certificate_cannot_downgrade_highest_qc`. |
| `validationSafeNoHighest` | Accepted NewView QCs clear any in-flight proposal validation before late callbacks can mutate the view. Bridge coverage includes `tick_binds_highest_qc_and_clears_inflight_validation` and `invalid_validation_new_view_vote_uses_highest_qc_subject`. |
| `wrongHeight`, `wrongEpoch`, `wrongValidatorSet` | `on_certificate(...)` rejects NewView certificates whose height, epoch, or validator set does not match the engine round before phase-specific handling. Bridge coverage includes `new_view_certificates_with_wrong_epoch_or_validator_set_are_ignored`. |
| `wrongQuorumPolicy` | `on_certificate(...)` rejects certificates whose quorum policy differs from the engine policy before `on_new_view_qc(...)` can mutate state. Bridge coverage is shared with `certificates_with_wrong_view_or_quorum_policy_are_ignored`. |
| `sameView`, `lowerView` | `on_new_view_qc(...)` requires the certificate view to be strictly greater than the current view. Bridge coverage includes `stale_new_view_certificate_cannot_update_highest_qc_or_rewind_round`. |
| `futureHeightHighest`, `futureViewHighest`, `wrongEpochHighest` | `qc_ref_is_compatible_with_round(...)` rejects carried highest-QC evidence from a future height/view or wrong epoch. Bridge coverage includes `new_view_certificate_rejects_incompatible_highest_qc`. |

The engine proposal-ingress gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `safeUnlocked` | `ConsensusEngine::on_proposal(...)` accepts a current-round proposal without a lock and emits both `ValidateBlock` and a prepare `SignVote`. Bridge coverage includes `proposals_are_ignored_outside_proposal_phase` for the accepted first proposal. |
| `safeLockedSubject` | `proposal_satisfies_lock(...)` accepts a proposal whose block hash matches the current locked QC subject. Bridge coverage is shared with `locked_qc_blocks_unsafe_prepare_votes`. |
| `safeConflictHigherQc` | A conflicting proposal can unlock only with a strictly higher compatible QC. Bridge coverage includes `conflicting_proposal_requires_strictly_higher_qc_to_unlock`. |
| `wrongPhase` | `on_proposal(...)` ignores proposals outside `EnginePhase::Proposal`. Bridge coverage includes `proposals_are_ignored_outside_proposal_phase`. |
| `wrongHeight`, `wrongEpoch`, `wrongValidatorSet`, `wrongView` | `on_proposal(...)` requires exact round equality before requesting validation or signing prepare. Bridge coverage includes `proposals_with_wrong_round_context_are_ignored`. |
| `futureHeightHighest`, `futureViewHighest`, `wrongEpochHighest` | `qc_ref_is_compatible_with_round(...)` rejects proposal highest-QC evidence from a future height/view or wrong epoch. Bridge coverage includes `proposal_with_incompatible_highest_qc_cannot_unlock_conflicting_lock`. |
| `lockedConflictNoQc`, `lockedConflictEqualQc`, `lockedConflictLowerQc` | `proposal_satisfies_lock(...)` rejects conflicting locked proposals unless the proposal carries a strictly greater compatible QC. Bridge coverage includes `locked_qc_blocks_unsafe_prepare_votes` and `conflicting_proposal_requires_strictly_higher_qc_to_unlock`. |

The engine prepare-QC gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `safePrepareQc` | `ConsensusEngine::on_certificate(...)` dispatches a current-context `CertPhase::Prepare` certificate into `on_prepare_qc(...)`, which signs one commit vote. Bridge coverage includes `locked_qc_blocks_unsafe_prepare_votes`. |
| `wrongHeight`, `wrongEpoch`, `wrongValidatorSet` | `on_certificate(...)` rejects certificates whose round context does not match the engine round before phase-specific handling. Bridge coverage includes `prepare_qcs_with_wrong_round_context_are_ignored`. |
| `wrongQuorumPolicy` | `on_certificate(...)` rejects certificates whose quorum policy differs from the engine policy. Bridge coverage includes `certificates_with_wrong_view_or_quorum_policy_are_ignored`. |
| `staleView` | Prepare/commit certificates must match the current view after timeout/view advance. Bridge coverage includes `prepare_and_commit_qcs_from_previous_view_are_ignored_after_timeout`. |
| `committedHeight` | `committed.contains_key(...)` blocks certificate handling for finalized heights. Bridge coverage includes `prepare_qc_for_committed_height_is_ignored`. |
| `replaySamePrepareQc`, `conflictingPrepareQc` | The per-round `commit_votes` map suppresses duplicate and conflicting prepare-QC handling after the first commit-vote output. Bridge coverage includes `prepare_qc_replays_and_conflicts_do_not_emit_extra_commit_votes`. |
| `pendingFinality` | `on_prepare_qc(...)` suppresses commit-vote output while a commit QC is waiting for exact payload recovery. Bridge coverage includes `prepare_qc_during_pending_finality_does_not_emit_commit_vote`. |
| `locked`, `highest` | Accepted prepare QCs record the prepare QC as both `locked_qc` and `highest_qc`; bridge coverage includes `locked_qc_blocks_unsafe_prepare_votes` and the replay/conflict test above. |

The engine commit-QC gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `safePayloadAvailable` | `on_commit_qc(...)` commits immediately when `has_payload(...)` is true. Bridge coverage includes `payload_availability_without_commit_qc_never_finalizes`. |
| `safePayloadMissing` | `on_commit_qc(...)` records pending finality and emits `FetchPayload` when the payload is missing. Bridge coverage includes `commit_qc_waits_for_payload_before_finality`. |
| `wrongHeight`, `wrongEpoch`, `wrongValidatorSet` | `on_certificate(...)` rejects certificates whose round context does not match the engine round before commit-QC handling. Bridge coverage includes `commit_qcs_with_wrong_round_context_are_ignored`. |
| `wrongQuorumPolicy` | `on_certificate(...)` rejects certificates whose quorum policy differs from the engine policy. Bridge coverage includes `certificates_with_wrong_view_or_quorum_policy_are_ignored`. |
| `staleView` | Prepare/commit certificates must match the current view after timeout/view advance. Bridge coverage includes `prepare_and_commit_qcs_from_previous_view_are_ignored_after_timeout`. |
| `committedHeight` | `committed.contains_key(...)` blocks certificate handling for finalized heights. Bridge coverage includes `committed_commit_qc_replay_does_not_emit_duplicate_finality` and `conflicting_blocks_cannot_both_commit_at_same_height`. |
| `pendingReplaySameCommitQc`, `pendingConflictingCommitQc` | `pending_finality` suppresses duplicate fetches and conflicting pending-finality subjects until exact payload recovery resolves the current QC. Bridge coverage includes `pending_commit_qc_replays_and_conflicts_do_not_refetch_payload`. |
| `highest` | Accepted commit QCs update `highest_qc` whether they finalize immediately or request payload recovery. Bridge coverage includes `payload_availability_without_commit_qc_never_finalizes` and `commit_qc_waits_for_payload_before_finality`. |

The engine payload-availability gate model is intentionally finite. These are
the implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `noPendingPayload` | `ConsensusEngine::on_payload_available(...)` records local payload availability but emits no finality without a pending commit QC. Bridge coverage includes `payload_availability_without_commit_qc_never_finalizes`. |
| `matchingPendingPayload` | A payload notification matching the pending commit-QC subject removes the pending certificate and calls `commit_subject(...)`. Bridge coverage includes `commit_qc_waits_for_payload_before_finality`. |
| `payloadHashMismatch` | Same block hash with the wrong payload hash is ignored while preserving pending finality. Bridge coverage includes `pending_finality_ignores_payload_hash_mismatch_until_exact_payload_arrives`. |
| `parentMismatch` | Same block hash and payload hash with a different parent is ignored because the full `BlockSubject` must match the pending certificate. Bridge coverage includes `pending_finality_rejects_payload_hash_and_subject_replays_without_dropping_qc`. |
| `unknownBlockHash` | Payload availability for an unrelated block hash cannot satisfy the pending QC and must not drop it. Bridge coverage includes `pending_commit_qc_replays_and_conflicts_do_not_refetch_payload`. |

The engine committed-block gate model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `freshPlain` | `ConsensusEngine::on_committed_block(...)` records a freshly finalized height without emitting validator-set activation when no reconfiguration is present. Bridge coverage includes `committed_block_notifications_do_not_overwrite_conflicting_height`. |
| `freshBoundaryReconfiguration` | A reconfiguration activates only when its `activation_height` equals the committed block height plus one. Bridge coverage includes `reconfiguration_activates_only_after_old_set_finality`. |
| `freshNonBoundaryReconfiguration` | Non-boundary reconfiguration notifications are recorded but do not activate. Bridge coverage includes `reconfiguration_with_non_boundary_activation_is_not_activated`. |
| `duplicatePlain`, `duplicateBoundaryReconfiguration` | Duplicate same-height notifications are no-ops and cannot re-emit activation. Bridge coverage includes `duplicate_committed_block_notification_does_not_reactivate_reconfiguration`. |
| `conflictingPlain`, `conflictingBoundaryReconfiguration`, `conflictingNonBoundaryReconfiguration` | A committed height is immutable once recorded, so conflicting same-height notifications cannot overwrite state or activate reconfiguration. Bridge coverage includes `conflicting_committed_block_notification_cannot_activate_reconfiguration`. |

The validator-set transition model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `ActivationHeight`, `BoundaryHeight`, `staged`, `activated` | Epoch-boundary validator-set activation in the pure engine's `CommittedBlock { reconfiguration }` handling and the live pending-roster activation path. |
| `committedOld`, `committedNew` | Commit certificates are accepted only for the validator set active at that height; bridge coverage includes `reconfiguration_activates_only_after_old_set_finality` in `crates/iroha_core/src/sumeragi/engine.rs` and pending-roster activation tests in `main_loop/tests.rs`. |
| `committedMixed` | Mixed-set certificates are invalid: certificate signers are interpreted against exactly one validator-set id/hash in `crates/iroha_data_model/src/block/consensus.rs` and Sumeragi QC validation. |

The certified recovery model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `qcsObserved`, `pendingSubject` | Commit-QC observation and pending-finality state in the pure engine, including `ConsensusInput::CommitQc` / pending-finality handling in `crates/iroha_core/src/sumeragi/engine.rs`. |
| `fetchRequested`, `matchingPayloads`, `mismatchedPayloads` | Certified block fetch request/response validation. Responses must match height, view, block hash, commit-QC subject, payload hash, and checkpoint before materializing local payload state. |
| `rejectedMismatches` | Mismatched payload/hash/subject responses are rejected while keeping the pending QC available for a later exact response. Bridge coverage includes `pending_finality_rejects_payload_hash_and_subject_replays_without_dropping_qc`. |
| `committedSubjects` | State application/finality is gated on both the commit QC and exact matching payload; conflicting same-height finality is rejected by the pure engine and live certified-fetch validation path. |

The view-change safety model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `currentView`, `maxAcceptedView` | Accepted new-view certificates move the pure engine round forward and stale certificates are ignored by `on_new_view_qc(...)`. Bridge coverage includes `stale_new_view_certificate_cannot_update_highest_qc_or_rewind_round`. |
| `highestRank`, `acceptedQcRanks` | Highest-QC monotonicity maps to `record_highest_qc(...)` and deterministic `select_highest_qc(...)` ordering in `crates/iroha_core/src/sumeragi/engine.rs`. |
| `lockedBranch`, `lockRank` | Locked-QC state maps to `locked_qc` and `proposal_satisfies_lock(...)`; bridge coverage includes `locked_qc_blocks_unsafe_prepare_votes`. |
| `unsafeLockOverwrite` | Conflicting prepare-QC replay/overwrite rejection maps to the pure engine's per-round `commit_votes` guard; bridge coverage includes `prepare_qc_replays_and_conflicts_do_not_emit_extra_commit_votes`. |

The validation-gate model is intentionally finite. These are the implementation
surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `validating`, `validationView` | The pure engine's `validating: Option<BlockSubject>` and current `RoundId` checks in `on_validation_result(...)`. |
| `UnknownValidationFailure` | Rejection of validation callbacks whose block hash does not match the in-flight proposal; bridge coverage includes `validation_results_for_unknown_or_completed_proposals_do_not_force_view_change`. |
| `CompletedValidationReplay` | Replayed success/failure callbacks after a proposal's validation state is consumed; the same bridge test proves these do not force a view change. |
| `TimeoutClearsOrRetainsInflight`, `LateValidationAfterTimeout` | Timeout clears the in-flight validation before a late failure arrives; bridge coverage includes `timeout_clears_inflight_validation_before_late_failure_arrives`. |
| `CurrentValidationFails`, `invalidAdvanceSubjects` | An invalid current proposal advances the view once and clears ownership so replayed failures are ignored; bridge coverage includes `invalid_validation_result_for_current_proposal_advances_view_once`. |

The pure engine validation-result model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `validCurrent`, `invalidNoHighest`, `invalidWithHighest` | The pure engine's `on_validation_result(...)` accepts only a result for the current `RoundId` and exact `validating.block_hash`. Bridge coverage includes `validation_results_for_unknown_or_completed_proposals_do_not_force_view_change` and `invalid_validation_result_for_current_proposal_advances_view_once`. |
| `validCurrent` | A successful validation callback clears the in-flight owner and emits no consensus outputs while the engine stays in prepare phase. |
| `invalidNoHighest`, `invalidWithHighest` | A failed current validation clears ownership, emits `NewView` plus `AdvanceView`, and advances to proposal phase. Bridge coverage includes `invalid_validation_new_view_vote_uses_highest_qc_subject`. |
| `supersededByCommit`, `supersededByCommittedBlock` | Commit QCs and committed-block notifications clear stale validation ownership before late invalid callbacks can mutate pending finality or committed state. Bridge coverage includes `commit_qc_supersedes_late_invalid_validation_result`, `conflicting_commit_qc_supersedes_late_invalid_validation_result`, and `committed_block_notification_supersedes_late_invalid_validation_result`. |

The certificate-admission model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `WrongContextCertificate` | `on_certificate(...)` rejects certificates whose height, epoch, validator-set id, or quorum policy do not match the local consensus context. Bridge coverage includes `certificates_with_wrong_view_or_quorum_policy_are_ignored` and `new_view_certificates_with_wrong_epoch_or_validator_set_are_ignored`. |
| `StalePrepareCommitCertificate` | Prepare/commit certificates must match the current view after timeout/view advance. Bridge coverage includes `prepare_and_commit_qcs_from_previous_view_are_ignored_after_timeout`. |
| `FutureHeightCertificate` | Certificate height must match the current height before it can mutate phase, lock, or pending-finality state. Bridge coverage includes `future_round_certificates_do_not_move_local_phase`. |
| `CommittedHeightCertificate` | Already committed heights are immutable through `committed.contains_key(...)` admission checks and `on_committed_block(...)` conflict guards. Bridge coverage includes `conflicting_blocks_cannot_both_commit_at_same_height` and `committed_block_notifications_do_not_overwrite_conflicting_height`. |

The highest-QC selection model is intentionally finite. These are the
implementation surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `ObservedNewViewQcs` | `select_highest_qc(...)` filters to `CertPhase::NewView` certificates before considering embedded `highest_qc` values. |
| `SpecGreater` | `qc_ref_cmp(...)` orders QCs by height, view, phase rank, and subject hash bytes. Bridge coverage includes `new_view_certificate_selects_highest_qc_deterministically`. |
| `EqualObservedSelectsEqualQc` | Deterministic aggregation: the selected highest QC is independent of certificate arrival/order. The bridge test checks both input orders for height, phase-rank, and subject tie-break cases. |
| `SelectedOnlyFromNewViewCertificates` | Prepare/commit certificates do not contribute highest-QC evidence to a new-view aggregation; the bridge test asserts a prepare/commit-only input selects no QC. |

The frontier model is intentionally finite. These are the implementation
surfaces it abstracts:

| Model concept | Implementation surface |
| --- | --- |
| `frontierSlot`, `pending`, `contiguous`, `payloadState` | `PendingBlock` handling and local payload checks in `crates/iroha_core/src/sumeragi/main_loop/reschedule.rs`, plus BlockCreated/frontier ownership materialization in `proposal_handlers.rs`. |
| `commitVotes`, `queuedVotes` | Commit-vote counting and vote ingress gating exercised by `reschedule_defers_vote_backed_quorum_timeout_while_vote_queue_backlogged` and `reschedule_ignores_quorum_timeout_vote_queue_backlog` in `crates/iroha_core/src/sumeragi/main_loop/tests.rs`. |
| `recoveryOwner` | Active/stale frontier owner state in `frontier_slot_has_active_owner_state_for_view(...)`, stale-owner yield in `maybe_yield_stale_frontier_owner_for_fresh_proposal(...)`, and supersede cleanup in `drop_superseded_contiguous_frontier_owner_state(...)`. |
| `quorumRescheduleArmed`, `quorumWindowAge` | Vote-backed quorum reschedule pacing in `reschedule_stale_pending_blocks_with_now(...)`; regression coverage includes `reschedule_skips_vote_backed_retransmit_while_frontier_quorum_timeout_window_owned`. |
| `payloadRecovered` | Exact frontier body repair and stale RBC repair admission in `request_frontier_owner_body_repair(...)`, `handle_frontier_body_gap_with_topology(...)`, and `stale_frontier_rbc_repair_is_actionable(...)`. |
| `quorumRetransmitted`, `rotated` | Quorum retransmit target selection, `rebroadcast_pending_block_updates(...)`, and deterministic view-change calls in `reschedule_stale_pending_blocks_with_now(...)`. |
| `subjectView`, `progressAge`, `lastProgressKind`, `validationState`, `localVoteEmitted`, `commitQcObserved` | Pending-block progress age/touch accounting. Validation, local commit-vote emission, and commit-QC observation map to `PendingBlock::touch_progress(...)`, `PendingBlock::note_local_commit_vote_emitted(...)`, and `PendingBlock::note_commit_qc_observed(...)`. |
| `recoveryLastRotationView`, `staleRecoveryUnlocked` | Same-height stale frontier recovery unlocks must be scoped by the vote/view that actually rotated. This maps to `stale_same_height_recovery_age(...)` and the stale-owner quorum-timeout guards in the Sumeragi main loop. |
| `futurePresent`, `futureContiguous`, `futureCommitVotes`, `futureQueuedVotes`, `futurePayloadState`, `futureRecoveryOwner` | One concrete future frontier slot. `FutureFrontierEvidence` is derived from the slot instead of stored as an independent Boolean. |
| `futureEvidenceObserved` | A late or initially present future-evidence obligation. Once observed, the future slot must remain concrete evidence until promotion. |
| `futurePromotionReady`, `futurePromoted`, `promotionFresh` | The two-step future reanchor path: clear the stale/current pending wrapper, then promote the future slot into the active slot with active progress flags reset. This maps to future new-view / higher-frontier quorum handling in `on_pacemaker_propose_ready(...)`, covered by `pacemaker_reanchors_frontier_when_future_new_view_quorum_exists`, `pacemaker_reanchors_future_new_view_quorum_while_vote_queue_backlogged`, and `pacemaker_reanchors_future_new_view_quorum_over_stale_frontier_owner`. |

## Running

From repository root:

```bash
bash scripts/formal/sumeragi_apalache.sh fast
bash scripts/formal/sumeragi_apalache.sh deep
bash scripts/formal/sumeragi_apalache.sh fork-fast
bash scripts/formal/sumeragi_apalache.sh fork-npos
bash scripts/formal/sumeragi_apalache.sh quorum-fast
bash scripts/formal/sumeragi_apalache.sh rbc-fast
bash scripts/formal/sumeragi_apalache.sh qc-signers-fast
bash scripts/formal/sumeragi_apalache.sh commit-roots-fast
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-fast
bash scripts/formal/sumeragi_apalache.sh commit-evidence-replay-fast
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-fast
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-fast
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-fast
bash scripts/formal/sumeragi_apalache.sh precommit-fast
bash scripts/formal/sumeragi_apalache.sh proposal-fast
bash scripts/formal/sumeragi_apalache.sh engine-tick-fast
bash scripts/formal/sumeragi_apalache.sh engine-new-view-fast
bash scripts/formal/sumeragi_apalache.sh engine-proposal-fast
bash scripts/formal/sumeragi_apalache.sh engine-prepare-fast
bash scripts/formal/sumeragi_apalache.sh engine-commit-fast
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-fast
bash scripts/formal/sumeragi_apalache.sh engine-payload-fast
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-fast
bash scripts/formal/sumeragi_apalache.sh reconfig-fast
bash scripts/formal/sumeragi_apalache.sh recovery-fast
bash scripts/formal/sumeragi_apalache.sh view-change-fast
bash scripts/formal/sumeragi_apalache.sh validation-fast
bash scripts/formal/sumeragi_apalache.sh admission-fast
bash scripts/formal/sumeragi_apalache.sh highest-fast
bash scripts/formal/sumeragi_apalache.sh frontier-fast
bash scripts/formal/sumeragi_apalache.sh frontier-deep
bash scripts/formal/sumeragi_apalache.sh frontier-wide
bash scripts/formal/sumeragi_tlc.sh frontier-small
```

The runner sets an explicit Apalache `--length` for each mode:

| Mode | Length | Intended use |
| --- | ---: | --- |
| `fast` | 10 | CI commit-path check |
| `deep` | 10 | Larger commit-path check |
| `fork-fast` | 9 | CI permissioned fork-safety check |
| `fork-npos` | 9 | CI NPoS stake-quorum fork-safety check |
| `quorum-fast` | 2 | CI quorum-policy arithmetic check |
| `rbc-fast` | 2 | CI RBC deliver-quorum gate check |
| `qc-signers-fast` | 2 | CI QC signer-bitmap admission check |
| `commit-roots-fast` | 2 | CI commit-root consistency check |
| `commit-pipeline-recovery-fast` | 2 | CI commit-pipeline recovery gate check |
| `commit-evidence-replay-fast` | 2 | CI known-block commit-evidence replay gate check |
| `block-sync-recovery-fast` | 2 | CI block-sync recovery admission gate check |
| `native-amx-attestation-fast` | 2 | CI native AMX attestation gate check |
| `native-amx-journal-fast` | 1 | CI native AMX queue-journal replay check |
| `precommit-fast` | 2 | CI precommit vote-emission gate check |
| `proposal-fast` | 2 | CI proposal assembly gate check |
| `engine-tick-fast` | 2 | CI pure engine tick gate check |
| `engine-new-view-fast` | 2 | CI pure engine NewView-QC gate check |
| `engine-proposal-fast` | 2 | CI pure engine proposal-ingress gate check |
| `engine-prepare-fast` | 2 | CI pure engine prepare-QC gate check |
| `engine-commit-fast` | 2 | CI pure engine commit-QC gate check |
| `engine-committed-block-fast` | 2 | CI pure engine committed-block gate check |
| `engine-payload-fast` | 2 | CI pure engine payload-availability gate check |
| `engine-validation-result-fast` | 2 | CI pure engine validation-result gate check |
| `reconfig-fast` | 7 | CI validator-set transition safety check |
| `recovery-fast` | 7 | CI certified payload recovery safety check |
| `view-change-fast` | 6 | CI view-change and lock-safety check |
| `validation-fast` | 6 | CI validation-callback ownership check |
| `admission-fast` | 6 | CI certificate-admission guard check |
| `highest-fast` | 6 | CI deterministic highest-QC selection check |
| `frontier-fast` | 7 | CI frontier check |
| `frontier-deep` | 8 | Larger frontier check |
| `frontier-wide` | 7 | Wider PR formal CI frontier check |
| `frontier-nightly` | 10 | Manual/scheduled wider-bound frontier check |

`APALACHE_LENGTH=<n>` overrides the per-mode default when locally exploring a
counterexample or widening a bounded proof.

`scripts/formal/sumeragi_tlc.sh frontier-small` runs a small exhaustive TLC
cross-check using the same module and TLC-friendly weak-fairness specification.
The TLC config disables generic deadlock rejection because resolved terminal
states, such as a legitimate zero-evidence drop, are valid endpoints; invariants
and temporal properties remain checked.

## Operating Process

Use the expected-failure configs as mutation tests when a formal model changes.
A useful model change should either keep every existing mutation red or add a
new expected-failure config before strengthening the spec.

If a new Taira hang report involves more than one concrete future frontier slot,
do not stretch this two-slot proof by adding more Boolean shortcuts. Add a
three-slot or parameterized follow-up model, then map the new transition back to
focused Rust regression tests.

If a counterexample only relies on abstract evidence predicates, first add or
tighten a Rust bridge test that exercises the corresponding Sumeragi state
transition. Runtime consensus code should change only after the bridge test
shows the abstraction mismatch is real.

The docs metadata job intentionally emits stale `source_hash` warnings for
translated Sumeragi formal READMEs until their bodies are refreshed. PR and
nightly CI upload a JSON metadata report so the translation refresh can be
tracked without pretending stale translations are current.

### Reproducible local setup (no Docker required)

Install the pinned local Apalache toolchain used by this repository:

```bash
bash scripts/formal/install_apalache.sh 0.52.2
```

The runner auto-detects this install at:
`target/apalache/toolchains/v0.52.2/bin/apalache-mc`.
After installation, `ci/check_sumeragi_formal.sh` should work without extra env vars:

```bash
bash ci/check_sumeragi_formal.sh
```

The expected-failure mutations are part of normal formal CI through
`ci/check_sumeragi_formal.sh`. They should fail under Apalache and are useful
when changing the model:

```bash
bash ci/check_sumeragi_formal_expected_failures.sh
```

Individual mutation modes are also accepted by the runner:

```bash
bash scripts/formal/sumeragi_apalache.sh frontier-bug-stale-owner
bash scripts/formal/sumeragi_apalache.sh frontier-bug-vote-queue
bash scripts/formal/sumeragi_apalache.sh frontier-bug-payload-recovery
bash scripts/formal/sumeragi_apalache.sh frontier-bug-retransmit-followthrough
bash scripts/formal/sumeragi_apalache.sh frontier-bug-future-promotion
bash scripts/formal/sumeragi_apalache.sh frontier-bug-future-reanchor-clear
bash scripts/formal/sumeragi_apalache.sh frontier-bug-future-evidence-drop
bash scripts/formal/sumeragi_apalache.sh frontier-bug-promotion-reset
bash scripts/formal/sumeragi_apalache.sh frontier-bug-future-stale-owner
bash scripts/formal/sumeragi_apalache.sh frontier-bug-progress-touch
bash scripts/formal/sumeragi_apalache.sh frontier-bug-height-only-recovery
bash scripts/formal/sumeragi_apalache.sh fork-bug-double-sign
bash scripts/formal/sumeragi_apalache.sh quorum-bug-count-under-threshold
bash scripts/formal/sumeragi_apalache.sh quorum-bug-count-over-validators
bash scripts/formal/sumeragi_apalache.sh quorum-bug-stake-exact-two-thirds
bash scripts/formal/sumeragi_apalache.sh quorum-bug-stake-over-total
bash scripts/formal/sumeragi_apalache.sh quorum-bug-stake-invalid-input
bash scripts/formal/sumeragi_apalache.sh quorum-bug-stake-overflow
bash scripts/formal/sumeragi_apalache.sh rbc-bug-duplicate-ready
bash scripts/formal/sumeragi_apalache.sh rbc-bug-under-quorum-deliver
bash scripts/formal/sumeragi_apalache.sh rbc-bug-wrong-commit-formula
bash scripts/formal/sumeragi_apalache.sh rbc-bug-force-one-ignored
bash scripts/formal/sumeragi_apalache.sh qc-signers-bug-count-observers
bash scripts/formal/sumeragi_apalache.sh qc-signers-bug-ignore-bitmap-length
bash scripts/formal/sumeragi_apalache.sh qc-signers-bug-ignore-out-of-bounds
bash scripts/formal/sumeragi_apalache.sh qc-signers-bug-under-quorum-accept
bash scripts/formal/sumeragi_apalache.sh commit-roots-bug-mix-root-signers
bash scripts/formal/sumeragi_apalache.sh commit-roots-bug-count-wrong-context
bash scripts/formal/sumeragi_apalache.sh commit-roots-bug-tie-high-root
bash scripts/formal/sumeragi_apalache.sh commit-roots-bug-stake-ignores-weight
bash scripts/formal/sumeragi_apalache.sh commit-roots-bug-under-quorum-accept
bash scripts/formal/sumeragi_apalache.sh commit-roots-bug-validate-mismatched-roots
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-bug-skip-local-qc-formation
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-bug-recover-despite-local-quorum
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-bug-request-recovery-before-timeout
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-bug-request-recovery-without-local-vote
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-bug-request-recovery-with-commit-qc
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-bug-request-recovery-with-missing-data
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-bug-request-recovery-invalid-pending
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-bug-request-recovery-off-tip
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-bug-skip-missing-qc-request
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-bug-drop-commit-qc-marker
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-bug-skip-quorum-retransmit
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-bug-use-collector-targets
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-bug-rebroadcast-without-votes
bash scripts/formal/sumeragi_apalache.sh commit-pipeline-recovery-bug-rebroadcast-after-qc
bash scripts/formal/sumeragi_apalache.sh commit-evidence-replay-bug-replay-inactive
bash scripts/formal/sumeragi_apalache.sh commit-evidence-replay-bug-ignore-cooldown
bash scripts/formal/sumeragi_apalache.sh commit-evidence-replay-bug-replay-without-targets
bash scripts/formal/sumeragi_apalache.sh commit-evidence-replay-bug-skip-first-evidence
bash scripts/formal/sumeragi_apalache.sh commit-evidence-replay-bug-skip-progress
bash scripts/formal/sumeragi_apalache.sh commit-evidence-replay-bug-skip-stalled-retry
bash scripts/formal/sumeragi_apalache.sh commit-evidence-replay-bug-replay-no-evidence
bash scripts/formal/sumeragi_apalache.sh commit-evidence-replay-bug-votes-use-payload-fallback
bash scripts/formal/sumeragi_apalache.sh commit-evidence-replay-bug-commit-qc-uses-votes
bash scripts/formal/sumeragi_apalache.sh commit-evidence-replay-bug-drop-commit-qc-replay
bash scripts/formal/sumeragi_apalache.sh commit-evidence-replay-bug-use-local-targets
bash scripts/formal/sumeragi_apalache.sh commit-evidence-replay-bug-use-duplicate-targets
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-accept-stale-without-request
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-drop-requested-stale
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-accept-future-unrequested
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-revive-aborted-without-commit-qc
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-keep-aborted-with-commit-qc
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-skip-vote-backed-owner
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-steal-owner-with-payload-only
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-skip-certified-owner
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-activate-uncertified-conflict
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-drop-commit-qc-marker
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-skip-missing-commit-qc-request
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-keep-missing-request
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-clear-inflight-for-payload-only
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-keep-inflight-for-certified
bash scripts/formal/sumeragi_apalache.sh block-sync-recovery-bug-promote-unvalidated-qc
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-seal-non-native-plan
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-seal-empty-roster
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-skip-prepare-request
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-skip-commit-request
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-request-commit-before-prepare
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-retry-prepare-after-quorum
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-seal-with-prepare-only
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-seal-with-commit-only
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-seal-partial-multi-leg
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-accept-duplicate-prepare
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-accept-duplicate-commit
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-accept-wrong-prepare-body
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-accept-wrong-commit-body
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-accept-outsider-signer
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-use-arrival-order-bitmap
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-collapse-retry-bodies
bash scripts/formal/sumeragi_apalache.sh native-amx-attestation-bug-collapse-participant-legs
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-drop-native-plan
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-collapse-native-to-single
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-single-plan-as-native
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-drop-participants
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-reorder-participants
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-keep-duplicate-participant
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-recompute-digest-wrong
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-drop-gossip-payload
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-drop-entrypoint
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-remove-by-hash-only
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-ignore-exact-remove
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-replay-unsupported-version
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-first-put-wins
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-compaction-drops-live
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-compaction-keeps-removed
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-keep-torn-tail
bash scripts/formal/sumeragi_apalache.sh native-amx-journal-bug-drop-prior-on-tail-repair
bash scripts/formal/sumeragi_apalache.sh precommit-bug-invalid-validation
bash scripts/formal/sumeragi_apalache.sh precommit-bug-observer
bash scripts/formal/sumeragi_apalache.sh precommit-bug-duplicate
bash scripts/formal/sumeragi_apalache.sh precommit-bug-unsuperseded-conflict
bash scripts/formal/sumeragi_apalache.sh precommit-bug-older-quorum-completion
bash scripts/formal/sumeragi_apalache.sh precommit-bug-locked-conflict
bash scripts/formal/sumeragi_apalache.sh precommit-bug-missing-locked-payload
bash scripts/formal/sumeragi_apalache.sh precommit-bug-non-extending-lock
bash scripts/formal/sumeragi_apalache.sh precommit-bug-reject-safe
bash scripts/formal/sumeragi_apalache.sh proposal-bug-observer
bash scripts/formal/sumeragi_apalache.sh proposal-bug-active-vote-conflict
bash scripts/formal/sumeragi_apalache.sh proposal-bug-pending-vote-verification
bash scripts/formal/sumeragi_apalache.sh proposal-bug-missing-highest-qc
bash scripts/formal/sumeragi_apalache.sh proposal-bug-non-extending-highest
bash scripts/formal/sumeragi_apalache.sh proposal-bug-split-vote-lock
bash scripts/formal/sumeragi_apalache.sh proposal-bug-committed-edge-conflict
bash scripts/formal/sumeragi_apalache.sh proposal-bug-reject-safe
bash scripts/formal/sumeragi_apalache.sh proposal-bug-reject-stale-retired
bash scripts/formal/sumeragi_apalache.sh proposal-bug-reject-locked-fallback
bash scripts/formal/sumeragi_apalache.sh engine-tick-bug-skip-round-advance
bash scripts/formal/sumeragi_apalache.sh engine-tick-bug-skip-new-view-vote
bash scripts/formal/sumeragi_apalache.sh engine-tick-bug-skip-advance-output
bash scripts/formal/sumeragi_apalache.sh engine-tick-bug-wrong-phase
bash scripts/formal/sumeragi_apalache.sh engine-tick-bug-keep-validation
bash scripts/formal/sumeragi_apalache.sh engine-tick-bug-drop-pending-finality
bash scripts/formal/sumeragi_apalache.sh engine-tick-bug-use-zero-despite-highest
bash scripts/formal/sumeragi_apalache.sh engine-tick-bug-use-highest-without-highest
bash scripts/formal/sumeragi_apalache.sh engine-tick-bug-omit-highest-binding
bash scripts/formal/sumeragi_apalache.sh engine-tick-bug-bind-highest-without-highest
bash scripts/formal/sumeragi_apalache.sh engine-new-view-bug-accept-wrong-context
bash scripts/formal/sumeragi_apalache.sh engine-new-view-bug-accept-wrong-quorum
bash scripts/formal/sumeragi_apalache.sh engine-new-view-bug-accept-stale-view
bash scripts/formal/sumeragi_apalache.sh engine-new-view-bug-accept-incompatible-highest
bash scripts/formal/sumeragi_apalache.sh engine-new-view-bug-reject-safe-no-highest
bash scripts/formal/sumeragi_apalache.sh engine-new-view-bug-reject-safe-improving-highest
bash scripts/formal/sumeragi_apalache.sh engine-new-view-bug-reject-safe-lower-highest
bash scripts/formal/sumeragi_apalache.sh engine-new-view-bug-skip-advance-output
bash scripts/formal/sumeragi_apalache.sh engine-new-view-bug-wrong-phase
bash scripts/formal/sumeragi_apalache.sh engine-new-view-bug-keep-validation
bash scripts/formal/sumeragi_apalache.sh engine-new-view-bug-drop-pending-finality
bash scripts/formal/sumeragi_apalache.sh engine-new-view-bug-overwrite-lower-highest
bash scripts/formal/sumeragi_apalache.sh engine-new-view-bug-skip-highest-record
bash scripts/formal/sumeragi_apalache.sh engine-proposal-bug-wrong-phase
bash scripts/formal/sumeragi_apalache.sh engine-proposal-bug-wrong-round
bash scripts/formal/sumeragi_apalache.sh engine-proposal-bug-incompatible-highest
bash scripts/formal/sumeragi_apalache.sh engine-proposal-bug-locked-conflict-no-qc
bash scripts/formal/sumeragi_apalache.sh engine-proposal-bug-locked-conflict-equal-qc
bash scripts/formal/sumeragi_apalache.sh engine-proposal-bug-locked-conflict-lower-qc
bash scripts/formal/sumeragi_apalache.sh engine-proposal-bug-reject-unlocked
bash scripts/formal/sumeragi_apalache.sh engine-proposal-bug-reject-locked-subject
bash scripts/formal/sumeragi_apalache.sh engine-proposal-bug-reject-higher-qc
bash scripts/formal/sumeragi_apalache.sh engine-proposal-bug-skip-validation
bash scripts/formal/sumeragi_apalache.sh engine-proposal-bug-skip-prepare-vote
bash scripts/formal/sumeragi_apalache.sh engine-proposal-bug-skip-prepare-phase
bash scripts/formal/sumeragi_apalache.sh engine-prepare-bug-wrong-context
bash scripts/formal/sumeragi_apalache.sh engine-prepare-bug-wrong-quorum-policy
bash scripts/formal/sumeragi_apalache.sh engine-prepare-bug-stale-view
bash scripts/formal/sumeragi_apalache.sh engine-prepare-bug-committed-height
bash scripts/formal/sumeragi_apalache.sh engine-prepare-bug-replay-prepare
bash scripts/formal/sumeragi_apalache.sh engine-prepare-bug-conflicting-prepare
bash scripts/formal/sumeragi_apalache.sh engine-prepare-bug-pending-finality
bash scripts/formal/sumeragi_apalache.sh engine-prepare-bug-reject-safe
bash scripts/formal/sumeragi_apalache.sh engine-prepare-bug-missing-lock-record
bash scripts/formal/sumeragi_apalache.sh engine-commit-bug-wrong-context
bash scripts/formal/sumeragi_apalache.sh engine-commit-bug-wrong-quorum-policy
bash scripts/formal/sumeragi_apalache.sh engine-commit-bug-stale-view
bash scripts/formal/sumeragi_apalache.sh engine-commit-bug-committed-height
bash scripts/formal/sumeragi_apalache.sh engine-commit-bug-pending-replay
bash scripts/formal/sumeragi_apalache.sh engine-commit-bug-pending-conflict
bash scripts/formal/sumeragi_apalache.sh engine-commit-bug-commit-without-payload
bash scripts/formal/sumeragi_apalache.sh engine-commit-bug-fetch-despite-payload
bash scripts/formal/sumeragi_apalache.sh engine-commit-bug-reject-available
bash scripts/formal/sumeragi_apalache.sh engine-commit-bug-reject-missing-payload
bash scripts/formal/sumeragi_apalache.sh engine-commit-bug-missing-highest-record
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-bug-skip-fresh-record
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-bug-reject-boundary-activation
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-bug-activate-without-boundary
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-bug-activate-non-boundary
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-bug-record-duplicate
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-bug-activate-duplicate
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-bug-record-conflict
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-bug-activate-conflict
bash scripts/formal/sumeragi_apalache.sh engine-committed-block-bug-overwrite-conflict
bash scripts/formal/sumeragi_apalache.sh engine-payload-bug-skip-available-record
bash scripts/formal/sumeragi_apalache.sh engine-payload-bug-commit-without-pending
bash scripts/formal/sumeragi_apalache.sh engine-payload-bug-commit-mismatched-payload
bash scripts/formal/sumeragi_apalache.sh engine-payload-bug-drop-pending-on-mismatch
bash scripts/formal/sumeragi_apalache.sh engine-payload-bug-reject-matching-payload
bash scripts/formal/sumeragi_apalache.sh engine-payload-bug-keep-pending-after-commit
bash scripts/formal/sumeragi_apalache.sh engine-payload-bug-wrong-phase-after-commit
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-accept-wrong-round
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-accept-wrong-block-hash
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-accept-no-inflight
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-accept-superseded
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-reject-current-valid
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-reject-current-invalid
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-keep-validation
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-valid-emits-output
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-skip-round-advance
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-skip-new-view-vote
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-skip-advance-output
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-wrong-phase
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-use-invalid-subject-despite-highest
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-use-highest-without-highest
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-omit-highest-binding
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-bind-highest-without-highest
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-drop-pending-finality
bash scripts/formal/sumeragi_apalache.sh engine-validation-result-bug-overwrite-committed
bash scripts/formal/sumeragi_apalache.sh reconfig-bug-premature-activation
bash scripts/formal/sumeragi_apalache.sh reconfig-bug-premature-new-cert
bash scripts/formal/sumeragi_apalache.sh reconfig-bug-mixed-cert
bash scripts/formal/sumeragi_apalache.sh recovery-bug-commit-without-payload
bash scripts/formal/sumeragi_apalache.sh recovery-bug-mismatched-payload
bash scripts/formal/sumeragi_apalache.sh recovery-bug-conflicting-finality
bash scripts/formal/sumeragi_apalache.sh view-change-bug-stale-new-view
bash scripts/formal/sumeragi_apalache.sh view-change-bug-unsafe-proposal
bash scripts/formal/sumeragi_apalache.sh view-change-bug-lock-overwrite
bash scripts/formal/sumeragi_apalache.sh view-change-bug-highest-regression
bash scripts/formal/sumeragi_apalache.sh validation-bug-unknown-result
bash scripts/formal/sumeragi_apalache.sh validation-bug-completed-replay
bash scripts/formal/sumeragi_apalache.sh validation-bug-timeout-inflight
bash scripts/formal/sumeragi_apalache.sh validation-bug-invalid-replay
bash scripts/formal/sumeragi_apalache.sh admission-bug-wrong-context
bash scripts/formal/sumeragi_apalache.sh admission-bug-stale-prepare-commit
bash scripts/formal/sumeragi_apalache.sh admission-bug-future-height
bash scripts/formal/sumeragi_apalache.sh admission-bug-committed-height
bash scripts/formal/sumeragi_apalache.sh highest-bug-height-priority
bash scripts/formal/sumeragi_apalache.sh highest-bug-phase-rank
bash scripts/formal/sumeragi_apalache.sh highest-bug-subject-tie
bash scripts/formal/sumeragi_apalache.sh highest-bug-non-new-view
```

If Apalache is not in `PATH`, you can:

- set `APALACHE_BIN` to the executable path, or
- use the Docker fallback (enabled by default when `docker` is available):
  - image: `APALACHE_DOCKER_IMAGE` (default `ghcr.io/apalache-mc/apalache:0.52.2`)
  - requires a running Docker daemon
  - disable fallback with `APALACHE_ALLOW_DOCKER=0`.

Examples:

```bash
APALACHE_BIN=/opt/apalache/bin/apalache-mc bash scripts/formal/sumeragi_apalache.sh fast
APALACHE_DOCKER_IMAGE=ghcr.io/apalache-mc/apalache:0.52.2 bash scripts/formal/sumeragi_apalache.sh frontier-deep
```

## Notes

- This model complements (does not replace) executable Rust model tests in
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_model_tests.rs`
  and
  `crates/iroha_core/src/sumeragi/main_loop/tests/state_machine_fairness_model_tests.rs`.
- The checks are bounded by constant values in the `.cfg` files.
- PR CI runs these checks in `.github/workflows/pr.yml` via
  `ci/check_sumeragi_formal.sh`.
- Scheduled/manual CI runs the same formal baseline plus the longer
  `frontier-nightly` bound in `.github/workflows/nightly_sumeragi_formal.yml`.
- English documentation is authoritative for the current frontier formal slice.
  Translated `docs/formal/sumeragi/README.*.md` files are intentionally not
  refreshed here and may remain source-current stale until a separate
  translation refresh.
