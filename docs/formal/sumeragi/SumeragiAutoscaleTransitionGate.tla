---- MODULE SumeragiAutoscaleTransitionGate ----
EXTENDS FiniteSets, Integers

(***************************************************************************
A bounded model for the autoscale commit gate and elastic-lane lifecycle.

The original slice checked the exact `autoscale_transition_committed_at(...)`
call-site contract. That direct truth-table remains below. The state machine
also models the safety boundary around an automatically created or retired
lane: optimistic catalog binding, a recoverable physical-geometry journal,
fresh incarnations, activation heights, and fail-closed stale-artifact
admission.

Scale-in is an explicit two-phase protocol. A consensus carrier first commits
a drain request that closes routing above an exact proposal height. A lane
drain certificate can then be accepted only after every committed pre-close
item, including a commit delayed in the network, is present in the canonical
merge frontier. A later global carrier commits that certificate. Catalog
retirement requires a still later carrier, so neither an uncarried certificate
nor a certificate introduced by the retirement carrier can authorize removal.

Each lane incarnation pins the validator committee that was authoritative when
the incarnation was created. The current global roster may churn independently,
but a drain request keeps using the pinned committee. Therefore every drain
quorum intersects any pinned-committee quorum that certified pre-close work,
and at least one locked signer prevents a lower-frontier drain certificate
until that historical work reaches the canonical merge frontier.

`Bug` is used by the expected-failure configurations. `"none"` is the
production specification.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

Lanes == {0, 1, 2}
BaselineLanes == {0}
ElasticLanes == {1, 2}
InitialLanes == {0, 1}
MinLanes == 1
MaxLanes == 3
MaxHeight == 4
MaxLaneTip == 1

Validators == {0, 1, 2, 3, 4, 5}
PrimaryCommittee == {0, 1, 2}
AlternateCommittee == {3, 4, 5}
Committees == {PrimaryCommittee, AlternateCommittee}
EmptyCommittee == {}
CommitteeValues == Committees \cup {EmptyCommittee}
QuorumSize == 2

PrimaryQuorums == {{0, 1}, {0, 2}, {1, 2}, {0, 1, 2}}
AlternateQuorums == {{3, 4}, {3, 5}, {4, 5}, {3, 4, 5}}
CandidateSignerSets ==
  PrimaryQuorums \cup AlternateQuorums \cup {{}, {0}, {3}}

CommitteeQuorums(committee) ==
  CASE committee = PrimaryCommittee -> PrimaryQuorums
    [] committee = AlternateCommittee -> AlternateQuorums
    [] OTHER -> {}

ValidCertificate == "valid"
MalformedCertificate == "malformed"
ForgedCertificate == "forged"
UnderQuorumCertificate == "under_quorum"
CertificateKinds == {
  ValidCertificate,
  MalformedCertificate,
  ForgedCertificate,
  UnderQuorumCertificate
}

VARIABLES
  \* Original direct helper gate sentinel.
  \* @type: Int;
  checked,
  \* Last committed global carrier height.
  \* @type: Int;
  height,
  \* Authoritative consensus catalog and last consensus-published catalog.
  \* @type: Set(Int);
  catalog,
  \* @type: Set(Int);
  committedCatalog,
  \* Physically provisioned Kura/WSV lane geometry.
  \* @type: Set(Int);
  physical,
  \* Per-lane monotonic generation and active incarnation (0 = inactive).
  \* @type: Int -> Int;
  generation,
  \* @type: Int -> Int;
  retiredGeneration,
  \* @type: Int -> Int;
  incarnation,
  \* First global proposal height allowed for the active incarnation.
  \* @type: Int -> Int;
  activation,
  \* @type: Int -> Int;
  transitionHeight,
  \* 0 = none, l + 1 = create l, -(l + 1) = retire l.
  \* @type: Int;
  pending,
  \* @type: Set(Int);
  pendingBase,
  \* 0 = clean, 1 = physical prepare, 2 = catalog published.
  \* @type: Int;
  journalPhase,
  \* @type: Bool;
  pressure,
  \* @type: Bool;
  idle,
  \* Delivered lane-commit frontier, total committed frontier (including a
  \* delayed commit), and canonical merge frontier.
  \* @type: Int -> Int;
  laneTip,
  \* @type: Int -> Int;
  totalCommittedTip,
  \* @type: Int -> Int;
  mergedTip,
  \* A pre-close commit exists but has not yet reached the merge path.
  \* @type: Set(Int);
  delayedPreClose,
  \* Immutable committee binding for the latest incarnation, the incarnation
  \* named by that binding, and the first binding retained for every bounded
  \* historical incarnation.
  \* @type: Int -> Set(Int);
  pinnedCommittee,
  \* @type: Int -> Int;
  pinnedCommitteeIncarnation,
  \* @type: Int -> (Int -> Set(Int));
  committeeHistory,
  \* Independently churning global roster; it never rewrites lane pins.
  \* @type: Set(Int);
  currentRoster,
  \* Certified pre-close work whose effect is not yet in the merge frontier,
  \* plus the pinned-committee quorum that certified it.
  \* @type: Set(Int);
  certifiedBacklog,
  \* @type: Int -> Set(Int);
  commitSigners,
  \* -1 = routing open; otherwise the largest admissible proposal height.
  \* @type: Int -> Int;
  closeHeight,
  \* Incarnation bound by the committed drain request.
  \* @type: Int -> Int;
  drainIncarnation,
  \* Committee frozen by the close request and signers on an accepted drain
  \* certificate.
  \* @type: Int -> Set(Int);
  drainCommittee,
  \* @type: Int -> Set(Int);
  certificateSigners,
  \* Locally assembled and validated certificate awaiting a global carrier.
  \* @type: Set(Int);
  certificateReady,
  \* @type: Int -> Int;
  certificateTip,
  \* @type: Int -> Int;
  certificateIncarnation,
  \* Global carrier that committed the certificate, or -1 when absent.
  \* @type: Int -> Int;
  commitmentCarrier,
  \* @type: Int -> Int;
  commitmentTip,
  \* Global carrier that removed the lane, or -1 while not retired.
  \* @type: Int -> Int;
  retirementCarrier,
  \* @type: Bool;
  staleArtifactAccepted,
  \* @type: Bool;
  postCloseProposalAccepted,
  \* @type: Bool;
  invalidDrainCertificateAccepted,
  \* @type: Bool;
  nonConsensusCatalogWrite,
  \* @type: Bool;
  baselineDigestIntact

vars ==
  <<checked, height, catalog, committedCatalog, physical, generation,
    retiredGeneration, incarnation, activation, transitionHeight, pending,
    pendingBase, journalPhase, pressure, idle, laneTip, totalCommittedTip,
    mergedTip, delayedPreClose, pinnedCommittee,
    pinnedCommitteeIncarnation, committeeHistory, currentRoster,
    certifiedBacklog, commitSigners, closeHeight, drainIncarnation,
    drainCommittee, certificateSigners, certificateReady, certificateTip,
    certificateIncarnation,
    commitmentCarrier, commitmentTip, retirementCarrier,
    staleArtifactAccepted, postCloseProposalAccepted,
    invalidDrainCertificateAccepted, nonConsensusCatalogWrite,
    baselineDigestIntact>>

CommitteeState ==
  <<pinnedCommittee, pinnedCommitteeIncarnation, committeeHistory,
    currentRoster, certifiedBacklog, commitSigners, drainCommittee,
    certificateSigners>>

(***************************************************************************
Direct transition-helper truth table retained for source-level mutation gates.
***************************************************************************)

Cases == {
  "enabled_matching_success",
  "enabled_matching_failure",
  "disabled_matching_success",
  "enabled_previous_success",
  "enabled_next_success",
  "disabled_previous_failure"
}

Enabled(c) ==
  c \in {
    "enabled_matching_success",
    "enabled_matching_failure",
    "enabled_previous_success",
    "enabled_next_success"
  }

CommitSuccess(c) ==
  c \in {
    "enabled_matching_success",
    "disabled_matching_success",
    "enabled_previous_success",
    "enabled_next_success"
  }

CommittedHeight(c) == 10

LastTransitionHeight(c) ==
  CASE c \in {
         "enabled_matching_success",
         "enabled_matching_failure",
         "disabled_matching_success"
       } -> 10
    [] c \in {"enabled_previous_success", "disabled_previous_failure"} -> 9
    [] c = "enabled_next_success" -> 11
    [] OTHER -> 0

SpecHelperResult(c) ==
  Enabled(c) /\ LastTransitionHeight(c) = CommittedHeight(c)

ActualHelperResult(c) ==
  CASE Bug = "skip_matching_transition"
       /\ Enabled(c)
       /\ LastTransitionHeight(c) = CommittedHeight(c) -> FALSE
    [] Bug = "ignore_enabled"
       /\ ~Enabled(c)
       /\ LastTransitionHeight(c) = CommittedHeight(c) -> TRUE
    [] Bug = "ignore_height"
       /\ Enabled(c) -> TRUE
    [] Bug = "off_by_one_previous"
       /\ Enabled(c)
       /\ LastTransitionHeight(c) + 1 = CommittedHeight(c) -> TRUE
    [] Bug = "off_by_one_next"
       /\ Enabled(c)
       /\ LastTransitionHeight(c) = CommittedHeight(c) + 1 -> TRUE
    [] OTHER -> SpecHelperResult(c)

SpecQueueReconfigured(c) ==
  CommitSuccess(c) /\ SpecHelperResult(c)

ActualQueueReconfigured(c) ==
  CASE Bug = "skip_success_reconfigure"
       /\ CommitSuccess(c)
       /\ ActualHelperResult(c) -> FALSE
    [] Bug = "reconfigure_failed_commit"
       /\ ~CommitSuccess(c)
       /\ ActualHelperResult(c) -> TRUE
    [] Bug = "reconfigure_without_transition"
       /\ CommitSuccess(c)
       /\ ~ActualHelperResult(c) -> TRUE
    [] OTHER -> CommitSuccess(c) /\ ActualHelperResult(c)

SpecReportedHeight(c) ==
  IF SpecQueueReconfigured(c) THEN CommittedHeight(c) ELSE -1

ActualReportedHeight(c) ==
  IF ActualQueueReconfigured(c) THEN
    IF Bug = "wrong_reported_height" THEN CommittedHeight(c) + 1 ELSE CommittedHeight(c)
  ELSE -1

SpecCase(c) ==
  <<SpecHelperResult(c), SpecQueueReconfigured(c), SpecReportedHeight(c)>>

ActualCase(c) ==
  <<ActualHelperResult(c), ActualQueueReconfigured(c), ActualReportedHeight(c)>>

(***************************************************************************
Elastic lifecycle state machine.
***************************************************************************)

WorkReflected(l) ==
  /\ l \notin delayedPreClose
  /\ l \notin certifiedBacklog
  /\ laneTip[l] = totalCommittedTip[l]
  /\ mergedTip[l] = totalCommittedTip[l]

HasNoHistoricalSigningConflict(l, signers) ==
  l \notin certifiedBacklog \/ commitSigners[l] \cap signers = {}

SpecCertificateAccepted(l, kind, signers) ==
  /\ kind = ValidCertificate
  /\ drainIncarnation[l] = incarnation[l]
  /\ drainIncarnation[l] > 0
  /\ drainCommittee[l] = pinnedCommittee[l]
  /\ signers \in CommitteeQuorums(drainCommittee[l])
  /\ HasNoHistoricalSigningConflict(l, signers)
  /\ WorkReflected(l)

ActualCertificateAccepted(l, kind, signers) ==
  CASE SpecCertificateAccepted(l, kind, signers) -> TRUE
    [] Bug = "drain_uses_current_roster"
       /\ kind = ValidCertificate
       /\ l \in certifiedBacklog
       /\ ~WorkReflected(l)
       /\ drainIncarnation[l] = incarnation[l]
       /\ drainIncarnation[l] > 0
       /\ drainCommittee[l] = currentRoster
       /\ signers \in CommitteeQuorums(drainCommittee[l])
       /\ HasNoHistoricalSigningConflict(l, signers) -> TRUE
    [] kind = ValidCertificate
       /\ ~WorkReflected(l)
       /\ Bug \in {
            "certify_unmerged_work",
            "retire_undrained"
          } -> TRUE
    [] kind = ValidCertificate
       /\ l \in delayedPreClose
       /\ Bug = "retire_loses_delayed_work" -> TRUE
    [] kind = MalformedCertificate
       /\ Bug = "accept_malformed_certificate" -> TRUE
    [] kind = ForgedCertificate
       /\ Bug = "accept_forged_certificate" -> TRUE
    [] kind = UnderQuorumCertificate
       /\ Bug = "accept_under_quorum_certificate" -> TRUE
    [] OTHER -> FALSE

Init ==
  /\ checked = 0
  /\ height = 0
  /\ catalog = InitialLanes
  /\ committedCatalog = InitialLanes
  /\ physical = InitialLanes
  /\ generation = [l \in Lanes |-> IF l \in InitialLanes THEN 1 ELSE 0]
  /\ retiredGeneration = [l \in Lanes |-> 0]
  /\ incarnation = [l \in Lanes |-> IF l \in InitialLanes THEN 1 ELSE 0]
  /\ activation = [l \in Lanes |-> IF l \in InitialLanes THEN 0 ELSE -1]
  /\ transitionHeight = [l \in Lanes |-> IF l \in InitialLanes THEN 0 ELSE -1]
  /\ pending = 0
  /\ pendingBase = InitialLanes
  /\ journalPhase = 0
  /\ pressure = FALSE
  /\ idle = FALSE
  /\ laneTip = [l \in Lanes |-> 0]
  /\ totalCommittedTip = [l \in Lanes |-> 0]
  /\ mergedTip = [l \in Lanes |-> 0]
  /\ delayedPreClose = {}
  /\ pinnedCommittee =
       [l \in Lanes |-> IF l \in InitialLanes THEN PrimaryCommittee ELSE {}]
  /\ pinnedCommitteeIncarnation =
       [l \in Lanes |-> IF l \in InitialLanes THEN 1 ELSE 0]
  /\ committeeHistory =
       [l \in Lanes |->
          [g \in 0..(MaxHeight + 1) |->
             IF l \in InitialLanes /\ g = 1 THEN PrimaryCommittee ELSE {}]]
  /\ currentRoster = PrimaryCommittee
  /\ certifiedBacklog = {}
  /\ commitSigners = [l \in Lanes |-> {}]
  /\ closeHeight = [l \in Lanes |-> -1]
  /\ drainIncarnation = [l \in Lanes |-> 0]
  /\ drainCommittee = [l \in Lanes |-> {}]
  /\ certificateSigners = [l \in Lanes |-> {}]
  /\ certificateReady = {}
  /\ certificateTip = [l \in Lanes |-> 0]
  /\ certificateIncarnation = [l \in Lanes |-> 0]
  /\ commitmentCarrier = [l \in Lanes |-> -1]
  /\ commitmentTip = [l \in Lanes |-> 0]
  /\ retirementCarrier = [l \in Lanes |-> -1]
  /\ staleArtifactAccepted = FALSE
  /\ postCloseProposalAccepted = FALSE
  /\ invalidDrainCertificateAccepted = FALSE
  /\ nonConsensusCatalogWrite = FALSE
  /\ baselineDigestIntact = TRUE

ObservePressure ==
  /\ ~pressure
  /\ pressure' = TRUE
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, physical,
                 generation, retiredGeneration, incarnation, activation,
                 transitionHeight, pending, pendingBase, journalPhase, idle,
                 laneTip, totalCommittedTip, mergedTip, delayedPreClose,
                 closeHeight, drainIncarnation, certificateReady,
                 certificateTip, certificateIncarnation, commitmentCarrier,
                 commitmentTip, retirementCarrier, staleArtifactAccepted,
                 postCloseProposalAccepted, invalidDrainCertificateAccepted,
                 nonConsensusCatalogWrite, baselineDigestIntact,
                 CommitteeState>>

ObserveIdle ==
  /\ ~idle
  /\ idle' = TRUE
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, physical,
                 generation, retiredGeneration, incarnation, activation,
                 transitionHeight, pending, pendingBase, journalPhase,
                 pressure, laneTip, totalCommittedTip, mergedTip,
                 delayedPreClose, closeHeight, drainIncarnation,
                 certificateReady, certificateTip, certificateIncarnation,
                 commitmentCarrier, commitmentTip, retirementCarrier,
                 staleArtifactAccepted, postCloseProposalAccepted,
                 invalidDrainCertificateAccepted, nonConsensusCatalogWrite,
                 baselineDigestIntact, CommitteeState>>

AttemptProposal(l, proposalHeight) ==
  /\ l \in ElasticLanes
  /\ proposalHeight \in 0..(MaxHeight + 1)
  /\ LET valid ==
           /\ l \in catalog
           /\ proposalHeight >= activation[l]
           /\ (closeHeight[l] = -1 \/ proposalHeight <= closeHeight[l])
           /\ l \notin certificateReady
           /\ commitmentCarrier[l] = -1
           /\ totalCommittedTip[l] < MaxLaneTip
         acceptedPostClose ==
           /\ Bug = "accept_post_close_proposal"
           /\ l \in catalog
           /\ closeHeight[l] >= 0
           /\ proposalHeight > closeHeight[l]
           /\ totalCommittedTip[l] < MaxLaneTip
     IN
       /\ laneTip' =
            IF valid \/ acceptedPostClose
            THEN [laneTip EXCEPT ![l] = @ + 1]
            ELSE laneTip
       /\ totalCommittedTip' =
            IF valid \/ acceptedPostClose
            THEN [totalCommittedTip EXCEPT ![l] = @ + 1]
            ELSE totalCommittedTip
       /\ postCloseProposalAccepted' =
            (postCloseProposalAccepted \/ acceptedPostClose)
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, physical,
                 generation, retiredGeneration, incarnation, activation,
                 transitionHeight, pending, pendingBase, journalPhase,
                 pressure, idle, mergedTip, delayedPreClose, closeHeight,
                 drainIncarnation, certificateReady, certificateTip,
                 certificateIncarnation, commitmentCarrier, commitmentTip,
                 retirementCarrier, staleArtifactAccepted,
                 invalidDrainCertificateAccepted, nonConsensusCatalogWrite,
                 baselineDigestIntact, CommitteeState>>

StageDelayedPreClose(l, signers) ==
  /\ l \in (catalog \cap ElasticLanes)
  /\ closeHeight[l] = -1
  /\ l \notin delayedPreClose
  /\ l \notin certifiedBacklog
  /\ signers \in CommitteeQuorums(pinnedCommittee[l])
  /\ totalCommittedTip[l] < MaxLaneTip
  /\ totalCommittedTip' = [totalCommittedTip EXCEPT ![l] = @ + 1]
  /\ delayedPreClose' = delayedPreClose \cup {l}
  /\ certifiedBacklog' = certifiedBacklog \cup {l}
  /\ commitSigners' = [commitSigners EXCEPT ![l] = signers]
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, physical,
                 generation, retiredGeneration, incarnation, activation,
                 transitionHeight, pending, pendingBase, journalPhase,
                 pressure, idle, laneTip, mergedTip, closeHeight,
                 drainIncarnation, certificateReady, certificateTip,
                 certificateIncarnation, commitmentCarrier, commitmentTip,
                 retirementCarrier, staleArtifactAccepted,
                 postCloseProposalAccepted, invalidDrainCertificateAccepted,
                 nonConsensusCatalogWrite, baselineDigestIntact,
                 pinnedCommittee, pinnedCommitteeIncarnation,
                 committeeHistory, currentRoster, drainCommittee,
                 certificateSigners>>

DeliverDelayedPreClose(l) ==
  /\ l \in delayedPreClose
  /\ l \notin certificateReady
  /\ commitmentCarrier[l] = -1
  /\ laneTip' = [laneTip EXCEPT ![l] = @ + 1]
  /\ delayedPreClose' = delayedPreClose \ {l}
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, physical,
                 generation, retiredGeneration, incarnation, activation,
                 transitionHeight, pending, pendingBase, journalPhase,
                 pressure, idle, totalCommittedTip, mergedTip, closeHeight,
                 drainIncarnation, certificateReady, certificateTip,
                 certificateIncarnation, commitmentCarrier, commitmentTip,
                 retirementCarrier, staleArtifactAccepted,
                 postCloseProposalAccepted, invalidDrainCertificateAccepted,
                 nonConsensusCatalogWrite, baselineDigestIntact,
                 CommitteeState>>

MergeLaneWork(l) ==
  /\ l \in catalog
  /\ mergedTip[l] < laneTip[l]
  /\ mergedTip' = [mergedTip EXCEPT ![l] = @ + 1]
  /\ certifiedBacklog' =
       IF mergedTip[l] + 1 = totalCommittedTip[l]
       THEN certifiedBacklog \ {l}
       ELSE certifiedBacklog
  /\ commitSigners' =
       IF mergedTip[l] + 1 = totalCommittedTip[l]
       THEN [commitSigners EXCEPT ![l] = {}]
       ELSE commitSigners
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, physical,
                 generation, retiredGeneration, incarnation, activation,
                 transitionHeight, pending, pendingBase, journalPhase,
                 pressure, idle, laneTip, totalCommittedTip, delayedPreClose,
                 closeHeight, drainIncarnation, certificateReady,
                 certificateTip, certificateIncarnation, commitmentCarrier,
                 commitmentTip, retirementCarrier, staleArtifactAccepted,
                 postCloseProposalAccepted, invalidDrainCertificateAccepted,
                 nonConsensusCatalogWrite, baselineDigestIntact,
                 pinnedCommittee, pinnedCommitteeIncarnation,
                 committeeHistory, currentRoster, drainCommittee,
                 certificateSigners>>

RequestDrain(l) ==
  /\ pending = 0
  /\ journalPhase = 0
  /\ idle
  /\ l \in (catalog \cap ElasticLanes)
  /\ Cardinality(catalog) > MinLanes
  /\ closeHeight[l] = -1
  /\ height < MaxHeight
  /\ height' = height + 1
  /\ closeHeight' = [closeHeight EXCEPT ![l] = height']
  /\ drainIncarnation' = [drainIncarnation EXCEPT ![l] = incarnation[l]]
  /\ drainCommittee' =
       [drainCommittee EXCEPT ![l] =
          IF Bug = "drain_uses_current_roster"
          THEN currentRoster
          ELSE pinnedCommittee[l]]
  /\ idle' = FALSE
  /\ UNCHANGED <<checked, catalog, committedCatalog, physical, generation,
                 retiredGeneration, incarnation, activation,
                 transitionHeight, pending, pendingBase, journalPhase,
                 pressure, laneTip, totalCommittedTip, mergedTip,
                 delayedPreClose, certificateReady, certificateTip,
                 certificateIncarnation, commitmentCarrier, commitmentTip,
                 retirementCarrier, staleArtifactAccepted,
                 postCloseProposalAccepted, invalidDrainCertificateAccepted,
                 nonConsensusCatalogWrite, baselineDigestIntact,
                 pinnedCommittee, pinnedCommitteeIncarnation,
                 committeeHistory, currentRoster, certifiedBacklog,
                 commitSigners, certificateSigners>>

AttemptDrainCertificate(l, kind, signers) ==
  /\ l \in (catalog \cap ElasticLanes)
  /\ kind \in CertificateKinds
  /\ signers \in CandidateSignerSets
  /\ closeHeight[l] >= 0
  /\ l \notin certificateReady
  /\ commitmentCarrier[l] = -1
  /\ LET accepted == ActualCertificateAccepted(l, kind, signers)
         valid == SpecCertificateAccepted(l, kind, signers)
     IN
       /\ certificateReady' =
            IF accepted THEN certificateReady \cup {l} ELSE certificateReady
       /\ certificateTip' =
            IF accepted
            THEN [certificateTip EXCEPT ![l] = mergedTip[l]]
            ELSE certificateTip
       /\ certificateIncarnation' =
            IF accepted
            THEN [certificateIncarnation EXCEPT ![l] = drainIncarnation[l]]
            ELSE certificateIncarnation
       /\ certificateSigners' =
            IF accepted
            THEN [certificateSigners EXCEPT ![l] = signers]
            ELSE certificateSigners
       /\ invalidDrainCertificateAccepted' =
            (invalidDrainCertificateAccepted \/ (accepted /\ ~valid))
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, physical,
                 generation, retiredGeneration, incarnation, activation,
                 transitionHeight, pending, pendingBase, journalPhase,
                 pressure, idle, laneTip, totalCommittedTip, mergedTip,
                 delayedPreClose, closeHeight, drainIncarnation,
                 commitmentCarrier, commitmentTip, retirementCarrier,
                 staleArtifactAccepted, postCloseProposalAccepted,
                 nonConsensusCatalogWrite, baselineDigestIntact,
                 pinnedCommittee, pinnedCommitteeIncarnation,
                 committeeHistory, currentRoster, certifiedBacklog,
                 commitSigners, drainCommittee>>

CarryDrainCertificate(l) ==
  /\ l \in certificateReady
  /\ commitmentCarrier[l] = -1
  /\ height < MaxHeight
  /\ height' = height + 1
  /\ commitmentCarrier' = [commitmentCarrier EXCEPT ![l] = height']
  /\ commitmentTip' = [commitmentTip EXCEPT ![l] = certificateTip[l]]
  /\ UNCHANGED <<checked, catalog, committedCatalog, physical, generation,
                 retiredGeneration, incarnation, activation,
                 transitionHeight, pending, pendingBase, journalPhase,
                 pressure, idle, laneTip, totalCommittedTip, mergedTip,
                 delayedPreClose, closeHeight, drainIncarnation,
                 certificateReady, certificateTip, certificateIncarnation,
                 retirementCarrier, staleArtifactAccepted,
                 postCloseProposalAccepted, invalidDrainCertificateAccepted,
                 nonConsensusCatalogWrite, baselineDigestIntact,
                 CommitteeState>>

StageCreate(l) ==
  /\ pending = 0
  /\ journalPhase = 0
  /\ pressure
  /\ l \in ElasticLanes \ catalog
  /\ Cardinality(catalog) < MaxLanes
  /\ pending' = l + 1
  /\ pendingBase' = catalog
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, physical,
                 generation, retiredGeneration, incarnation, activation,
                 transitionHeight, journalPhase, pressure, idle, laneTip,
                 totalCommittedTip, mergedTip, delayedPreClose, closeHeight,
                 drainIncarnation, certificateReady, certificateTip,
                 certificateIncarnation, commitmentCarrier, commitmentTip,
                 retirementCarrier, staleArtifactAccepted,
                 postCloseProposalAccepted, invalidDrainCertificateAccepted,
                 nonConsensusCatalogWrite, baselineDigestIntact,
                 CommitteeState>>

StageRetire(l) ==
  /\ pending = 0
  /\ journalPhase = 0
  /\ l \in (catalog \cap ElasticLanes)
  /\ Cardinality(catalog) > MinLanes
  /\ (commitmentCarrier[l] >= 0 \/
      (Bug = "retire_without_carried_certificate" /\ idle))
  /\ pending' = -(l + 1)
  /\ pendingBase' = catalog
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, physical,
                 generation, retiredGeneration, incarnation, activation,
                 transitionHeight, journalPhase, pressure, idle, laneTip,
                 totalCommittedTip, mergedTip, delayedPreClose, closeHeight,
                 drainIncarnation, certificateReady, certificateTip,
                 certificateIncarnation, commitmentCarrier, commitmentTip,
                 retirementCarrier, staleArtifactAccepted,
                 postCloseProposalAccepted, invalidDrainCertificateAccepted,
                 nonConsensusCatalogWrite, baselineDigestIntact,
                 CommitteeState>>

PrepareCreate(l) ==
  /\ pending = l + 1
  /\ journalPhase = 0
  /\ physical = catalog
  /\ physical' = physical \cup {l}
  /\ journalPhase' = 1
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, generation,
                 retiredGeneration, incarnation, activation,
                 transitionHeight, pending, pendingBase, pressure, idle,
                 laneTip, totalCommittedTip, mergedTip, delayedPreClose,
                 closeHeight, drainIncarnation, certificateReady,
                 certificateTip, certificateIncarnation, commitmentCarrier,
                 commitmentTip, retirementCarrier, staleArtifactAccepted,
                 postCloseProposalAccepted, invalidDrainCertificateAccepted,
                 nonConsensusCatalogWrite, baselineDigestIntact,
                 CommitteeState>>

PrepareRetire(l) ==
  /\ pending = -(l + 1)
  /\ journalPhase = 0
  /\ physical = catalog
  /\ physical' = physical \ {l}
  /\ journalPhase' = 1
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, generation,
                 retiredGeneration, incarnation, activation,
                 transitionHeight, pending, pendingBase, pressure, idle,
                 laneTip, totalCommittedTip, mergedTip, delayedPreClose,
                 closeHeight, drainIncarnation, certificateReady,
                 certificateTip, certificateIncarnation, commitmentCarrier,
                 commitmentTip, retirementCarrier, staleArtifactAccepted,
                 postCloseProposalAccepted, invalidDrainCertificateAccepted,
                 nonConsensusCatalogWrite, baselineDigestIntact,
                 CommitteeState>>

CommitCreate(l) ==
  /\ pending = l + 1
  /\ journalPhase = 1
  /\ height < MaxHeight
  /\ (pendingBase = catalog \/ Bug = "stale_catalog_commit")
  /\ catalog' = catalog \cup {l}
  /\ committedCatalog' = catalog'
  /\ height' = height + 1
  /\ generation' =
       [generation EXCEPT ![l] =
          IF Bug = "reuse_incarnation" /\ retiredGeneration[l] > 0
          THEN @
          ELSE @ + 1]
  /\ incarnation' = [incarnation EXCEPT ![l] = generation'[l]]
  /\ activation' =
       [activation EXCEPT ![l] = IF Bug = "activate_early" THEN height ELSE height']
  /\ transitionHeight' = [transitionHeight EXCEPT ![l] = height']
  /\ laneTip' = [laneTip EXCEPT ![l] = 0]
  /\ totalCommittedTip' = [totalCommittedTip EXCEPT ![l] = 0]
  /\ mergedTip' = [mergedTip EXCEPT ![l] = 0]
  /\ delayedPreClose' = delayedPreClose \ {l}
  /\ pinnedCommittee' = [pinnedCommittee EXCEPT ![l] = currentRoster]
  /\ pinnedCommitteeIncarnation' =
       [pinnedCommitteeIncarnation EXCEPT ![l] = generation'[l]]
  /\ committeeHistory' =
       IF committeeHistory[l][generation'[l]] = {}
       THEN [committeeHistory EXCEPT ![l][generation'[l]] = currentRoster]
       ELSE committeeHistory
  /\ certifiedBacklog' = certifiedBacklog \ {l}
  /\ commitSigners' = [commitSigners EXCEPT ![l] = {}]
  /\ closeHeight' = [closeHeight EXCEPT ![l] = -1]
  /\ drainIncarnation' = [drainIncarnation EXCEPT ![l] = 0]
  /\ drainCommittee' = [drainCommittee EXCEPT ![l] = {}]
  /\ certificateSigners' = [certificateSigners EXCEPT ![l] = {}]
  /\ certificateReady' = certificateReady \ {l}
  /\ certificateTip' = [certificateTip EXCEPT ![l] = 0]
  /\ certificateIncarnation' = [certificateIncarnation EXCEPT ![l] = 0]
  /\ commitmentCarrier' = [commitmentCarrier EXCEPT ![l] = -1]
  /\ commitmentTip' = [commitmentTip EXCEPT ![l] = 0]
  /\ retirementCarrier' = [retirementCarrier EXCEPT ![l] = -1]
  /\ pending' = 0
  /\ pendingBase' = catalog'
  /\ journalPhase' = 2
  /\ pressure' = FALSE
  /\ UNCHANGED <<checked, physical, retiredGeneration, idle,
                 staleArtifactAccepted, postCloseProposalAccepted,
                 invalidDrainCertificateAccepted, nonConsensusCatalogWrite,
                 baselineDigestIntact, currentRoster>>

CommitRetire(l) ==
  /\ pending = -(l + 1)
  /\ journalPhase = 1
  /\ pendingBase = catalog
  /\ LET hasCommitment == commitmentCarrier[l] >= 0
         sameCarrier ==
           /\ Bug = "retire_same_carrier"
           /\ hasCommitment
           /\ height = commitmentCarrier[l]
         earlyRetirement ==
           /\ Bug = "retire_without_carried_certificate"
           /\ ~hasCommitment
     IN
       /\ (hasCommitment \/ earlyRetirement)
       /\ (sameCarrier \/ height < MaxHeight)
       /\ height' = IF sameCarrier THEN height ELSE height + 1
       /\ catalog' = catalog \ {l}
       /\ committedCatalog' = catalog'
       /\ retiredGeneration' = [retiredGeneration EXCEPT ![l] = generation[l]]
       /\ incarnation' = [incarnation EXCEPT ![l] = 0]
       /\ activation' = [activation EXCEPT ![l] = -1]
       /\ transitionHeight' = [transitionHeight EXCEPT ![l] = height']
       /\ retirementCarrier' = [retirementCarrier EXCEPT ![l] = height']
  /\ pending' = 0
  /\ pendingBase' = catalog'
  /\ journalPhase' = 2
  /\ idle' = FALSE
  /\ UNCHANGED <<checked, physical, generation, pressure, laneTip,
                 totalCommittedTip, mergedTip, delayedPreClose, closeHeight,
                 drainIncarnation, certificateReady, certificateTip,
                 certificateIncarnation, commitmentCarrier, commitmentTip,
                 staleArtifactAccepted, postCloseProposalAccepted,
                 invalidDrainCertificateAccepted, nonConsensusCatalogWrite,
                 baselineDigestIntact, CommitteeState>>

FinalizeJournal ==
  /\ journalPhase = 2
  /\ physical = catalog
  /\ journalPhase' = 0
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, physical,
                 generation, retiredGeneration, incarnation, activation,
                 transitionHeight, pending, pendingBase, pressure, idle,
                 laneTip, totalCommittedTip, mergedTip, delayedPreClose,
                 closeHeight, drainIncarnation, certificateReady,
                 certificateTip, certificateIncarnation, commitmentCarrier,
                 commitmentTip, retirementCarrier, staleArtifactAccepted,
                 postCloseProposalAccepted, invalidDrainCertificateAccepted,
                 nonConsensusCatalogWrite, baselineDigestIntact,
                 CommitteeState>>

CrashRecover ==
  /\ journalPhase \in {1, 2}
  /\ physical' = catalog
  /\ pending' = 0
  /\ pendingBase' = catalog
  /\ journalPhase' = 0
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, generation,
                 retiredGeneration, incarnation, activation,
                 transitionHeight, pressure, idle, laneTip,
                 totalCommittedTip, mergedTip, delayedPreClose, closeHeight,
                 drainIncarnation, certificateReady, certificateTip,
                 certificateIncarnation, commitmentCarrier, commitmentTip,
                 retirementCarrier, staleArtifactAccepted,
                 postCloseProposalAccepted, invalidDrainCertificateAccepted,
                 nonConsensusCatalogWrite, baselineDigestIntact,
                 CommitteeState>>

AttemptArtifact(l, claimedIncarnation, proposalHeight) ==
  /\ l \in Lanes
  /\ claimedIncarnation \in 0..(MaxHeight + 1)
  /\ proposalHeight \in 0..(MaxHeight + 1)
  /\ LET valid ==
           /\ l \in catalog
           /\ incarnation[l] = claimedIncarnation
           /\ claimedIncarnation > 0
           /\ proposalHeight >= activation[l]
           /\ (closeHeight[l] = -1 \/ proposalHeight <= closeHeight[l])
     IN staleArtifactAccepted' =
          (staleArtifactAccepted \/ (~valid /\ Bug = "accept_stale_artifact"))
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, physical,
                 generation, retiredGeneration, incarnation, activation,
                 transitionHeight, pending, pendingBase, journalPhase,
                 pressure, idle, laneTip, totalCommittedTip, mergedTip,
                 delayedPreClose, closeHeight, drainIncarnation,
                 certificateReady, certificateTip, certificateIncarnation,
                 commitmentCarrier, commitmentTip, retirementCarrier,
                 postCloseProposalAccepted, invalidDrainCertificateAccepted,
                 nonConsensusCatalogWrite, baselineDigestIntact,
                 CommitteeState>>

InjectedRestartDrift ==
  /\ Bug = "restart_geometry_drift"
  /\ journalPhase = 0
  /\ physical' = physical \cup (ElasticLanes \ catalog)
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, generation,
                 retiredGeneration, incarnation, activation,
                 transitionHeight, pending, pendingBase, journalPhase,
                 pressure, idle, laneTip, totalCommittedTip, mergedTip,
                 delayedPreClose, closeHeight, drainIncarnation,
                 certificateReady, certificateTip, certificateIncarnation,
                 commitmentCarrier, commitmentTip, retirementCarrier,
                 staleArtifactAccepted, postCloseProposalAccepted,
                 invalidDrainCertificateAccepted, nonConsensusCatalogWrite,
                 baselineDigestIntact, CommitteeState>>

InjectedNonConsensusCatalogWrite(l) ==
  /\ Bug = "non_consensus_catalog_write"
  /\ l \in ElasticLanes \ catalog
  /\ catalog' = catalog \cup {l}
  /\ nonConsensusCatalogWrite' = TRUE
  /\ UNCHANGED <<checked, height, committedCatalog, physical, generation,
                 retiredGeneration, incarnation, activation,
                 transitionHeight, pending, pendingBase, journalPhase,
                 pressure, idle, laneTip, totalCommittedTip, mergedTip,
                 delayedPreClose, closeHeight, drainIncarnation,
                 certificateReady, certificateTip, certificateIncarnation,
                 commitmentCarrier, commitmentTip, retirementCarrier,
                 staleArtifactAccepted, postCloseProposalAccepted,
                 invalidDrainCertificateAccepted, baselineDigestIntact,
                 CommitteeState>>

InjectedBaselineMutation ==
  /\ Bug = "mutate_baseline"
  /\ baselineDigestIntact' = FALSE
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, physical,
                 generation, retiredGeneration, incarnation, activation,
                 transitionHeight, pending, pendingBase, journalPhase,
                 pressure, idle, laneTip, totalCommittedTip, mergedTip,
                 delayedPreClose, closeHeight, drainIncarnation,
                 certificateReady, certificateTip, certificateIncarnation,
                 commitmentCarrier, commitmentTip, retirementCarrier,
                 staleArtifactAccepted, postCloseProposalAccepted,
                 invalidDrainCertificateAccepted, nonConsensusCatalogWrite,
                 CommitteeState>>

RotateCurrentRoster ==
  /\ currentRoster' =
       IF currentRoster = PrimaryCommittee
       THEN AlternateCommittee
       ELSE PrimaryCommittee
  /\ UNCHANGED <<checked, height, catalog, committedCatalog, physical,
                 generation, retiredGeneration, incarnation, activation,
                 transitionHeight, pending, pendingBase, journalPhase,
                 pressure, idle, laneTip, totalCommittedTip, mergedTip,
                 delayedPreClose, closeHeight, drainIncarnation,
                 certificateReady, certificateTip, certificateIncarnation,
                 commitmentCarrier, commitmentTip, retirementCarrier,
                 staleArtifactAccepted, postCloseProposalAccepted,
                 invalidDrainCertificateAccepted, nonConsensusCatalogWrite,
                 baselineDigestIntact, pinnedCommittee,
                 pinnedCommitteeIncarnation, committeeHistory,
                 certifiedBacklog, commitSigners, drainCommittee,
                 certificateSigners>>

Next ==
  \/ ObservePressure
  \/ ObserveIdle
  \/ RotateCurrentRoster
  \/ \E l \in ElasticLanes, h \in 0..(MaxHeight + 1):
       AttemptProposal(l, h)
  \/ \E l \in ElasticLanes,
        signers \in (PrimaryQuorums \cup AlternateQuorums):
       StageDelayedPreClose(l, signers)
  \/ \E l \in ElasticLanes: DeliverDelayedPreClose(l)
  \/ \E l \in ElasticLanes: MergeLaneWork(l)
  \/ \E l \in ElasticLanes: RequestDrain(l)
  \/ \E l \in ElasticLanes, kind \in CertificateKinds,
        signers \in CandidateSignerSets:
       AttemptDrainCertificate(l, kind, signers)
  \/ \E l \in ElasticLanes: CarryDrainCertificate(l)
  \/ \E l \in ElasticLanes: StageCreate(l)
  \/ \E l \in ElasticLanes: StageRetire(l)
  \/ \E l \in ElasticLanes: PrepareCreate(l)
  \/ \E l \in ElasticLanes: PrepareRetire(l)
  \/ \E l \in ElasticLanes: CommitCreate(l)
  \/ \E l \in ElasticLanes: CommitRetire(l)
  \/ FinalizeJournal
  \/ CrashRecover
  \/ \E l \in Lanes, i \in 0..(MaxHeight + 1), h \in 0..(MaxHeight + 1):
       AttemptArtifact(l, i, h)
  \/ InjectedRestartDrift
  \/ \E l \in ElasticLanes: InjectedNonConsensusCatalogWrite(l)
  \/ InjectedBaselineMutation

(***************************************************************************
Safety properties.
***************************************************************************)

TypeInvariant ==
  /\ checked = 0
  /\ height \in 0..MaxHeight
  /\ catalog \subseteq Lanes
  /\ committedCatalog \subseteq Lanes
  /\ physical \subseteq Lanes
  /\ generation \in [Lanes -> 0..(MaxHeight + 1)]
  /\ retiredGeneration \in [Lanes -> 0..MaxHeight]
  /\ incarnation \in [Lanes -> 0..(MaxHeight + 1)]
  /\ activation \in [Lanes -> -1..MaxHeight]
  /\ transitionHeight \in [Lanes -> -1..MaxHeight]
  /\ pending \in {-3, -2, 0, 2, 3}
  /\ pendingBase \subseteq Lanes
  /\ journalPhase \in {0, 1, 2}
  /\ pressure \in BOOLEAN
  /\ idle \in BOOLEAN
  /\ laneTip \in [Lanes -> 0..MaxLaneTip]
  /\ totalCommittedTip \in [Lanes -> 0..MaxLaneTip]
  /\ mergedTip \in [Lanes -> 0..MaxLaneTip]
  /\ delayedPreClose \subseteq Lanes
  /\ pinnedCommittee \in [Lanes -> CommitteeValues]
  /\ pinnedCommitteeIncarnation \in [Lanes -> 0..(MaxHeight + 1)]
  /\ committeeHistory \in
       [Lanes -> [0..(MaxHeight + 1) -> CommitteeValues]]
  /\ currentRoster \in Committees
  /\ certifiedBacklog \subseteq Lanes
  /\ commitSigners \in [Lanes -> SUBSET Validators]
  /\ closeHeight \in [Lanes -> -1..MaxHeight]
  /\ drainIncarnation \in [Lanes -> 0..(MaxHeight + 1)]
  /\ drainCommittee \in [Lanes -> CommitteeValues]
  /\ certificateSigners \in [Lanes -> SUBSET Validators]
  /\ certificateReady \subseteq Lanes
  /\ certificateTip \in [Lanes -> 0..MaxLaneTip]
  /\ certificateIncarnation \in [Lanes -> 0..(MaxHeight + 1)]
  /\ commitmentCarrier \in [Lanes -> -1..MaxHeight]
  /\ commitmentTip \in [Lanes -> 0..MaxLaneTip]
  /\ retirementCarrier \in [Lanes -> -1..MaxHeight]
  /\ staleArtifactAccepted \in BOOLEAN
  /\ postCloseProposalAccepted \in BOOLEAN
  /\ invalidDrainCertificateAccepted \in BOOLEAN
  /\ nonConsensusCatalogWrite \in BOOLEAN
  /\ baselineDigestIntact \in BOOLEAN

TransitionMatchesSpec ==
  \A c \in Cases: ActualCase(c) = SpecCase(c)

BaselinePreserved ==
  /\ BaselineLanes \subseteq catalog
  /\ baselineDigestIntact

CapacityBounds ==
  /\ Cardinality(catalog) >= MinLanes
  /\ Cardinality(catalog) <= MaxLanes

ActiveIncarnationDiscipline ==
  \A l \in catalog:
    /\ incarnation[l] = generation[l]
    /\ incarnation[l] > 0
    /\ generation[l] > retiredGeneration[l]
    /\ activation[l] >= 0
    /\ activation[l] <= height
    /\ activation[l] = transitionHeight[l]

InactiveIncarnationDiscipline ==
  \A l \in (Lanes \ catalog):
    /\ incarnation[l] = 0
    /\ activation[l] = -1
    /\ transitionHeight[l] <= height

PinnedCommitteeImmutablePerIncarnation ==
  \A l \in Lanes:
    /\ committeeHistory[l][0] = {}
    /\ \A g \in (generation[l] + 1)..(MaxHeight + 1):
         committeeHistory[l][g] = {}
    /\ IF generation[l] = 0
       THEN /\ pinnedCommittee[l] = {}
            /\ pinnedCommitteeIncarnation[l] = 0
       ELSE /\ pinnedCommittee[l] \in Committees
            /\ pinnedCommitteeIncarnation[l] = generation[l]
            /\ pinnedCommittee[l] = committeeHistory[l][generation[l]]

CertifiedBacklogUsesPinnedQuorum ==
  \A l \in certifiedBacklog:
    /\ commitSigners[l] \in CommitteeQuorums(pinnedCommittee[l])
    /\ mergedTip[l] < totalCommittedTip[l]

NoCommitSignersOutsideCertifiedBacklog ==
  \A l \in (Lanes \ certifiedBacklog):
    commitSigners[l] = {}

CleanGeometryMatchesCatalog ==
  journalPhase = 0 => physical = catalog

PublishedGeometryMatchesCatalog ==
  journalPhase = 2 => physical = catalog

OnlyConsensusPublishesCatalog ==
  /\ ~nonConsensusCatalogWrite
  /\ catalog = committedCatalog

StaleArtifactsFailClosed == ~staleArtifactAccepted

WorkFrontiersAreMonotonic ==
  \A l \in Lanes:
    /\ mergedTip[l] <= laneTip[l]
    /\ laneTip[l] <= totalCommittedTip[l]
    /\ totalCommittedTip[l] =
         laneTip[l] + IF l \in delayedPreClose THEN 1 ELSE 0

DrainBindingsAreExact ==
  \A l \in ElasticLanes:
    /\ (closeHeight[l] >= 0 =>
          /\ drainIncarnation[l] > 0
          /\ closeHeight[l] <= height)
    /\ (l \in certificateReady =>
          /\ closeHeight[l] >= 0
          /\ certificateIncarnation[l] = drainIncarnation[l])
    /\ (commitmentCarrier[l] >= 0 =>
          /\ l \in certificateReady
          /\ closeHeight[l] < commitmentCarrier[l]
          /\ commitmentCarrier[l] <= height
          /\ commitmentTip[l] = certificateTip[l])

DrainCommitteeBoundToPinnedIncarnation ==
  \A l \in ElasticLanes:
    closeHeight[l] >= 0 =>
      /\ drainIncarnation[l] = pinnedCommitteeIncarnation[l]
      /\ drainCommittee[l] = pinnedCommittee[l]

HistoricalCertifiedBacklogQuorumOverlap ==
  \A l \in ElasticLanes:
    closeHeight[l] >= 0 /\ l \in certifiedBacklog =>
      \A signers \in CommitteeQuorums(drainCommittee[l]):
        commitSigners[l] \cap signers # {}

AcceptedDrainSignersArePinned ==
  \A l \in certificateReady:
    certificateSigners[l] \in CommitteeQuorums(pinnedCommittee[l])

PinnedCommitteeDrainSafety ==
  /\ DrainCommitteeBoundToPinnedIncarnation
  /\ HistoricalCertifiedBacklogQuorumOverlap
  /\ AcceptedDrainSignersArePinned

CertificatesCoverAllPreCloseWork ==
  \A l \in certificateReady:
    /\ l \notin delayedPreClose
    /\ l \notin certifiedBacklog
    /\ laneTip[l] = totalCommittedTip[l]
    /\ mergedTip[l] = totalCommittedTip[l]
    /\ certificateTip[l] = totalCommittedTip[l]

InvalidDrainCertificatesFailClosed ==
  ~invalidDrainCertificateAccepted

NoPostCloseProposals ==
  ~postCloseProposalAccepted

NoEarlyRetirement ==
  \A l \in ElasticLanes:
    retirementCarrier[l] >= 0 => commitmentCarrier[l] >= 0

NoSameCarrierRetirement ==
  \A l \in ElasticLanes:
    retirementCarrier[l] >= 0 =>
      retirementCarrier[l] > commitmentCarrier[l]

NoLostDelayedWork ==
  \A l \in ElasticLanes:
    retirementCarrier[l] >= 0 =>
      /\ l \notin delayedPreClose
      /\ l \notin certifiedBacklog
      /\ mergedTip[l] = totalCommittedTip[l]
      /\ commitmentTip[l] = totalCommittedTip[l]

RetirementRequiresDrain ==
  /\ CertificatesCoverAllPreCloseWork
  /\ InvalidDrainCertificatesFailClosed
  /\ NoEarlyRetirement
  /\ NoSameCarrierRetirement
  /\ NoLostDelayedWork

AutoscaleTransitionExactness ==
  /\ TransitionMatchesSpec
  /\ BaselinePreserved
  /\ CapacityBounds
  /\ ActiveIncarnationDiscipline
  /\ InactiveIncarnationDiscipline
  /\ PinnedCommitteeImmutablePerIncarnation
  /\ CertifiedBacklogUsesPinnedQuorum
  /\ NoCommitSignersOutsideCertifiedBacklog
  /\ CleanGeometryMatchesCatalog
  /\ PublishedGeometryMatchesCatalog
  /\ OnlyConsensusPublishesCatalog
  /\ StaleArtifactsFailClosed
  /\ WorkFrontiersAreMonotonic
  /\ DrainBindingsAreExact
  /\ PinnedCommitteeDrainSafety
  /\ NoPostCloseProposals
  /\ RetirementRequiresDrain

AutoscaleTransitionCorrectnessEnvelope ==
  /\ TypeInvariant
  /\ AutoscaleTransitionExactness

SafetyFast == AutoscaleTransitionCorrectnessEnvelope

(***************************************************************************
Expected-failure invariants for the original source-level mutants.
***************************************************************************)

BugSkipMatchingTransition ==
  ActualCase("enabled_matching_success") = SpecCase("enabled_matching_success")

BugIgnoreEnabled ==
  ActualCase("disabled_matching_success") = SpecCase("disabled_matching_success")

BugIgnoreHeight ==
  ActualCase("enabled_previous_success") = SpecCase("enabled_previous_success")

BugOffByOnePrevious ==
  ActualCase("enabled_previous_success") = SpecCase("enabled_previous_success")

BugOffByOneNext ==
  ActualCase("enabled_next_success") = SpecCase("enabled_next_success")

BugSkipSuccessReconfigure ==
  ActualCase("enabled_matching_success") = SpecCase("enabled_matching_success")

BugReconfigureFailedCommit ==
  ActualCase("enabled_matching_failure") = SpecCase("enabled_matching_failure")

BugReconfigureWithoutTransition ==
  ActualCase("disabled_matching_success") = SpecCase("disabled_matching_success")

BugWrongReportedHeight ==
  ActualCase("enabled_matching_success") = SpecCase("enabled_matching_success")

====
