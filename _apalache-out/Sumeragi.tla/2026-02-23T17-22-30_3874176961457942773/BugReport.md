<!-- Thank you for filing a report! Please ensure you have filled out all -->
<!-- sections, as it help us to address the problem effectively. -->

<!-- NOTE: Please try to ensure the bug can be produced on the latest release of -->
<!-- Apalache. See https://github.com/apalache-mc/apalache/releases -->

## Impact

<!-- Whether this is blocking your work or whether you are able to proceed using -->
<!-- workarounds or alternative approaches. -->

## Input specification

```
---- MODULE Sumeragi ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for Sumeragi commit-path safety/liveness checks.

This spec intentionally models only the commit-critical state:
- voting phases and quorum counters,
- weighted NPoS stake quorum,
- RBC header/chunk/ready/deliver causality,
- view-change progression and GST flip,
- weak fairness assumptions over honest progress actions.
***************************************************************************)

CONSTANTS
  \* @type: Int;
  N,
  \* @type: Int;
  F,
  \* @type: Int;
  CommitQuorum,
  \* @type: Int;
  ViewQuorum,
  \* @type: Int;
  StakeQuorum,
  \* @type: Int;
  StakePerHonestVote,
  \* @type: Int;
  StakePerByzVote,
  \* @type: Int;
  MaxView,
  \* @type: Int;
  MaxChunks

VARIABLES
  \* @type: Str;
  phase,
  \* @type: Int;
  view,
  \* @type: Int;
  prepareVotes,
  \* @type: Int;
  commitVotesHonest,
  \* @type: Int;
  commitVotesByz,
  \* @type: Int;
  stakeSigned,
  \* @type: Int;
  newViewVotes,
  \* @type: Str;
  rbcState,
  \* @type: Int;
  chunkCount,
  \* @type: Int;
  readyVotes,
  \* @type: Bool;
  headerSeen,
  \* @type: Bool;
  digestValid,
  \* @type: Bool;
  committed,
  \* @type: Bool;
  gst

vars == <<
  phase,
  view,
  prepareVotes,
  commitVotesHonest,
  commitVotesByz,
  stakeSigned,
  newViewVotes,
  rbcState,
  chunkCount,
  readyVotes,
  headerSeen,
  digestValid,
  committed,
  gst
>>

Phases == {"Propose", "Prepare", "CommitVote", "NewView", "Committed"}
RbcStates == {
  "Idle",
  "Init",
  "Chunking",
  "ChunksComplete",
  "ReadyPartial",
  "ReadyQuorum",
  "Delivered",
  "Corrupted",
  "Withheld"
}

CanCommit(vh, vb, stake, rbc) ==
  /\ vh + vb >= CommitQuorum
  /\ stake >= StakeQuorum
  /\ rbc = "Delivered"

TypeInvariant ==
  /\ phase \in Phases
  /\ view \in 0..MaxView
  /\ prepareVotes \in 0..N
  /\ commitVotesHonest \in 0..N
  /\ commitVotesByz \in 0..N
  /\ stakeSigned \in Nat
  /\ newViewVotes \in 0..N
  /\ rbcState \in RbcStates
  /\ chunkCount \in 0..MaxChunks
  /\ readyVotes \in 0..N
  /\ headerSeen \in BOOLEAN
  /\ digestValid \in BOOLEAN
  /\ committed \in BOOLEAN
  /\ gst \in BOOLEAN

Init ==
  /\ phase = "Propose"
  /\ view = 0
  /\ prepareVotes = 0
  /\ commitVotesHonest = 0
  /\ commitVotesByz = 0
  /\ stakeSigned = 0
  /\ newViewVotes = 0
  /\ rbcState = "Idle"
  /\ chunkCount = 0
  /\ readyVotes = 0
  /\ headerSeen = FALSE
  /\ digestValid = FALSE
  /\ committed = FALSE
  /\ gst = FALSE

HonestProposeEnabled ==
  phase \in {"Propose", "NewView", "Committed"}

HonestPrepareVoteEnabled ==
  /\ phase = "Prepare"
  /\ prepareVotes < N - F

HonestCommitVoteEnabled ==
  /\ phase = "CommitVote"
  /\ commitVotesHonest < N - F

HonestNewViewVoteEnabled ==
  /\ phase = "NewView"
  /\ newViewVotes < N - F

RbcInitEnabled ==
  rbcState \in {"Idle", "Withheld", "Corrupted"}

RbcChunkGoodEnabled ==
  /\ rbcState \in {"Init", "Chunking", "Withheld"}
  /\ headerSeen
  /\ (rbcState # "Withheld" \/ gst)

RbcReadyGoodEnabled ==
  /\ rbcState \in {"ChunksComplete", "ReadyPartial", "ReadyQuorum"}
  /\ headerSeen
  /\ digestValid
  /\ readyVotes < N

RbcDeliverGoodEnabled ==
  /\ rbcState = "ReadyQuorum"
  /\ readyVotes >= CommitQuorum
  /\ headerSeen
  /\ digestValid

PostGstProgressEnabled ==
  \/ HonestProposeEnabled
  \/ HonestPrepareVoteEnabled
  \/ HonestCommitVoteEnabled
  \/ HonestNewViewVoteEnabled
  \/ RbcInitEnabled
  \/ RbcChunkGoodEnabled
  \/ RbcReadyGoodEnabled
  \/ RbcDeliverGoodEnabled

TimeoutTickEnabled ==
  ~gst \/ ~PostGstProgressEnabled

HonestPropose ==
  /\ HonestProposeEnabled
  /\ phase' = "Prepare"
  /\ prepareVotes' = 0
  /\ newViewVotes' = 0
  /\ rbcState' = IF rbcState = "Idle" THEN "Init" ELSE rbcState
  /\ headerSeen' = IF rbcState = "Idle" THEN TRUE ELSE headerSeen
  /\ digestValid' = IF rbcState = "Idle" THEN TRUE ELSE digestValid
  /\ chunkCount' = IF rbcState = "Idle" THEN 0 ELSE chunkCount
  /\ readyVotes' = IF rbcState = "Idle" THEN 0 ELSE readyVotes
  /\ UNCHANGED <<
      view,
      commitVotesHonest,
      commitVotesByz,
      stakeSigned,
      committed,
      gst
     >>

HonestPrepareVote ==
  /\ HonestPrepareVoteEnabled
  /\ prepareVotes' = prepareVotes + 1
  /\ phase' = IF prepareVotes' >= CommitQuorum THEN "CommitVote" ELSE "Prepare"
  /\ UNCHANGED <<
      view,
      commitVotesHonest,
      commitVotesByz,
      stakeSigned,
      newViewVotes,
      rbcState,
      chunkCount,
      readyVotes,
      headerSeen,
      digestValid,
      committed,
      gst
     >>

HonestCommitVote ==
  /\ HonestCommitVoteEnabled
  /\ commitVotesHonest' = commitVotesHonest + 1
  /\ stakeSigned' = stakeSigned + StakePerHonestVote
  /\ phase' =
      IF CanCommit(
            commitVotesHonest + 1,
            commitVotesByz,
            stakeSigned + StakePerHonestVote,
            rbcState
         )
      THEN "Committed"
      ELSE "CommitVote"
  /\ committed' =
      (committed \/ CanCommit(
                        commitVotesHonest + 1,
                        commitVotesByz,
                        stakeSigned + StakePerHonestVote,
                        rbcState
                    ))
  /\ UNCHANGED <<
      view,
      prepareVotes,
      commitVotesByz,
      newViewVotes,
      rbcState,
      chunkCount,
      readyVotes,
      headerSeen,
      digestValid,
      gst
     >>

ByzantineEquivocateCommit ==
  /\ phase = "CommitVote"
  /\ commitVotesByz < F
  /\ commitVotesByz' = commitVotesByz + 1
  /\ stakeSigned' = stakeSigned + StakePerByzVote
  /\ phase' =
      IF CanCommit(
            commitVotesHonest,
            commitVotesByz + 1,
            stakeSigned + StakePerByzVote,
            rbcState
         )
      THEN "Committed"
      ELSE "CommitVote"
  /\ committed' =
      (committed \/ CanCommit(
                        commitVotesHonest,
                        commitVotesByz + 1,
                        stakeSigned + StakePerByzVote,
                        rbcState
                    ))
  /\ UNCHANGED <<
      view,
      prepareVotes,
      commitVotesHonest,
      newViewVotes,
      rbcState,
      chunkCount,
      readyVotes,
      headerSeen,
      digestValid,
      gst
     >>

TimeoutTick ==
  /\ TimeoutTickEnabled
  /\ phase' = "NewView"
  /\ view' = IF view < MaxView THEN view + 1 ELSE MaxView
  /\ newViewVotes' = 0
  /\ prepareVotes' = 0
  /\ commitVotesHonest' = 0
  /\ commitVotesByz' = 0
  /\ stakeSigned' = 0
  /\ UNCHANGED <<
      rbcState,
      chunkCount,
      readyVotes,
      headerSeen,
      digestValid,
      committed,
      gst
     >>

HonestNewViewVote ==
  /\ HonestNewViewVoteEnabled
  /\ newViewVotes' = newViewVotes + 1
  /\ phase' = IF newViewVotes' >= ViewQuorum THEN "Propose" ELSE "NewView"
  /\ UNCHANGED <<
      view,
      prepareVotes,
      commitVotesHonest,
      commitVotesByz,
      stakeSigned,
      rbcState,
      chunkCount,
      readyVotes,
      headerSeen,
      digestValid,
      committed,
      gst
     >>

RbcInit ==
  /\ RbcInitEnabled
  /\ rbcState' = "Init"
  /\ chunkCount' = 0
  /\ readyVotes' = 0
  /\ headerSeen' = TRUE
  /\ digestValid' = TRUE
  /\ UNCHANGED <<
      phase,
      view,
      prepareVotes,
      commitVotesHonest,
      commitVotesByz,
      stakeSigned,
      newViewVotes,
      committed,
      gst
     >>

RbcChunkGood ==
  /\ RbcChunkGoodEnabled
  /\ chunkCount' = IF chunkCount < MaxChunks THEN chunkCount + 1 ELSE chunkCount
  /\ rbcState' = IF chunkCount' >= MaxChunks THEN "ChunksComplete" ELSE "Chunking"
  /\ digestValid' = TRUE
  /\ UNCHANGED <<
      phase,
      view,
      prepareVotes,
      commitVotesHonest,
      commitVotesByz,
      stakeSigned,
      newViewVotes,
      readyVotes,
      headerSeen,
      committed,
      gst
     >>

RbcReadyGood ==
  /\ RbcReadyGoodEnabled
  /\ readyVotes' = readyVotes + 1
  /\ rbcState' = IF readyVotes' >= CommitQuorum THEN "ReadyQuorum" ELSE "ReadyPartial"
  /\ UNCHANGED <<
      phase,
      view,
      prepareVotes,
      commitVotesHonest,
      commitVotesByz,
      stakeSigned,
      newViewVotes,
      chunkCount,
      headerSeen,
      digestValid,
      committed,
      gst
     >>

RbcDeliverGood ==
  /\ RbcDeliverGoodEnabled
  /\ rbcState' = "Delivered"
  /\ phase' =
      IF CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered")
      THEN "Committed"
      ELSE phase
  /\ committed' =
      (committed \/ CanCommit(commitVotesHonest, commitVotesByz, stakeSigned, "Delivered"))
  /\ UNCHANGED <<
      view,
      prepareVotes,
      commitVotesHonest,
      commitVotesByz,
      stakeSigned,
      newViewVotes,
      chunkCount,
      readyVotes,
      headerSeen,
      digestValid,
      gst
     >>

ByzantineFault ==
  /\ rbcState' = IF rbcState \in {"Init", "Chunking", "ChunksComplete", "ReadyPartial", "ReadyQuorum"}
                 THEN "Corrupted"
                 ELSE rbcState
  /\ digestValid' = IF rbcState' = "Corrupted" THEN FALSE ELSE digestValid
  /\ UNCHANGED <<
      phase,
      view,
      prepareVotes,
      commitVotesHonest,
      commitVotesByz,
      stakeSigned,
      newViewVotes,
      chunkCount,
      readyVotes,
      headerSeen,
      committed,
      gst
     >>

GstElapsed ==
  /\ ~gst
  /\ gst' = TRUE
  /\ UNCHANGED <<
      phase,
      view,
      prepareVotes,
      commitVotesHonest,
      commitVotesByz,
      stakeSigned,
      newViewVotes,
      rbcState,
      chunkCount,
      readyVotes,
      headerSeen,
      digestValid,
      committed
     >>

Next ==
  \/ HonestPropose
  \/ HonestPrepareVote
  \/ HonestCommitVote
  \/ ByzantineEquivocateCommit
  \/ TimeoutTick
  \/ HonestNewViewVote
  \/ RbcInit
  \/ RbcChunkGood
  \/ RbcReadyGood
  \/ RbcDeliverGood
  \/ ByzantineFault
  \/ GstElapsed

Fairness ==
  /\ WF_vars(HonestPropose)
  /\ WF_vars(HonestPrepareVote)
  /\ WF_vars(HonestCommitVote)
  /\ WF_vars(HonestNewViewVote)
  /\ WF_vars(RbcInit)
  /\ WF_vars(RbcChunkGood)
  /\ WF_vars(RbcReadyGood)
  /\ WF_vars(RbcDeliverGood)

Spec ==
  /\ Init
  /\ [][Next]_vars
  /\ Fairness

CommitImpliesQuorum ==
  committed => commitVotesHonest + commitVotesByz >= CommitQuorum

CommitImpliesStakeQuorum ==
  committed => stakeSigned >= StakeQuorum

CommitImpliesDelivered ==
  committed => rbcState = "Delivered"

DeliverImpliesEvidence ==
  rbcState = "Delivered" =>
    /\ readyVotes >= CommitQuorum
    /\ chunkCount >= MaxChunks
    /\ headerSeen
    /\ digestValid

EventuallyCommit ==
  Fairness => [] (gst => <> committed)

====
````

## The command line parameters used to run the tool

```
--config=/Users/mtakemiya/dev/iroha/docs/formal/sumeragi/Sumeragi_fast.cfg --run-dir=/Users/mtakemiya/dev/iroha/target/apalache/sumeragi-fast
```

## Expected behavior

<!-- What did you expect to see? -->

## Log files

<details>

```
2026-02-23T17:22:30,525 [main] INFO  a.f.a.t.Tool\$ - # APALACHE version: 0.52.2 | build: 9103560
2026-02-23T17:22:30,554 [main] INFO  a.f.a.i.p.o.OptionGroup\$ -   > Sumeragi_fast.cfg: Loading TLC configuration
2026-02-23T17:22:30,601 [main] INFO  a.f.a.i.p.o.OptionGroup\$ -   > Using init predicate(s) Init from the TLC config
2026-02-23T17:22:30,615 [main] INFO  a.f.a.i.p.o.OptionGroup\$ -   > Using next predicate(s) Next from the TLC config
2026-02-23T17:22:30,615 [main] INFO  a.f.a.i.p.o.OptionGroup\$ -   > Using temporal predicate(s) EventuallyCommit from the TLC config
2026-02-23T17:22:30,629 [main] INFO  a.f.a.i.p.o.OptionGroup\$ -   > Using inv predicate(s) TypeInvariant, CommitImpliesQuorum, CommitImpliesStakeQuorum, CommitImpliesDelivered, DeliverImpliesEvidence from the TLC config
2026-02-23T17:22:30,630 [main] INFO  a.f.a.t.t.o.CheckCmd - Tuning: search.outputTraces=false
2026-02-23T17:22:30,764 [main] INFO  a.f.a.i.p.PassChainExecutor - PASS #0: SanyParser
2026-02-23T17:22:31,009 [main] DEBUG a.f.a.i.p.PassChainExecutor - PASS #0: SanyParser [OK]
2026-02-23T17:22:31,016 [main] INFO  a.f.a.i.p.PassChainExecutor - PASS #1: TypeCheckerSnowcat
2026-02-23T17:22:31,022 [main] INFO  a.f.a.t.p.t.EtcTypeCheckerPassImpl -  > Running Snowcat .::.
2026-02-23T17:22:31,195 [main] INFO  a.f.a.t.p.t.EtcTypeCheckerPassImpl -  > Your types are purrfect!
2026-02-23T17:22:31,196 [main] INFO  a.f.a.t.p.t.EtcTypeCheckerPassImpl -  > All expressions are typed
2026-02-23T17:22:31,211 [main] DEBUG a.f.a.i.p.PassChainExecutor - PASS #1: TypeCheckerSnowcat [OK]
2026-02-23T17:22:31,226 [main] INFO  a.f.a.i.p.PassChainExecutor - PASS #2: ConfigurationPass
2026-02-23T17:22:31,337 [main] INFO  a.f.a.t.p.p.ConfigurationPassImpl -   > Sumeragi_fast.cfg: found INVARIANTS: TypeInvariant, CommitImpliesQuorum, CommitImpliesStakeQuorum, CommitImpliesDelivered, DeliverImpliesEvidence
2026-02-23T17:22:31,339 [main] INFO  a.f.a.t.p.p.ConfigurationPassImpl -   > Sumeragi_fast.cfg: found PROPERTIES: EventuallyCommit
2026-02-23T17:22:31,347 [main] INFO  a.f.a.t.p.p.ConfigurationPassImpl -   > Set the initialization predicate to Init
2026-02-23T17:22:31,352 [main] INFO  a.f.a.t.p.p.ConfigurationPassImpl -   > Set the transition predicate to Next
2026-02-23T17:22:31,366 [main] INFO  a.f.a.t.p.p.ConfigurationPassImpl -   > Set the constant initialization predicate to CInit
2026-02-23T17:22:31,379 [main] INFO  a.f.a.t.p.p.ConfigurationPassImpl -   > Set an invariant to TypeInvariant
2026-02-23T17:22:31,389 [main] INFO  a.f.a.t.p.p.ConfigurationPassImpl -   > Set an invariant to CommitImpliesQuorum
2026-02-23T17:22:31,390 [main] INFO  a.f.a.t.p.p.ConfigurationPassImpl -   > Set an invariant to CommitImpliesStakeQuorum
2026-02-23T17:22:31,402 [main] INFO  a.f.a.t.p.p.ConfigurationPassImpl -   > Set an invariant to CommitImpliesDelivered
2026-02-23T17:22:31,403 [main] INFO  a.f.a.t.p.p.ConfigurationPassImpl -   > Set an invariant to DeliverImpliesEvidence
2026-02-23T17:22:31,418 [main] INFO  a.f.a.t.p.p.ConfigurationPassImpl -   > Set a temporal property to EventuallyCommit
2026-02-23T17:22:31,430 [main] DEBUG a.f.a.i.p.PassChainExecutor - PASS #2: ConfigurationPass [OK]
2026-02-23T17:22:31,437 [main] INFO  a.f.a.i.p.PassChainExecutor - PASS #3: DesugarerPass
2026-02-23T17:22:31,437 [main] INFO  a.f.a.t.p.p.DesugarerPassImpl -   > Desugaring...
2026-02-23T17:22:31,458 [main] DEBUG a.f.a.i.p.PassChainExecutor - PASS #3: DesugarerPass [OK]
2026-02-23T17:22:31,470 [main] INFO  a.f.a.i.p.PassChainExecutor - PASS #4: InlinePass
2026-02-23T17:22:31,485 [main] INFO  a.f.a.t.p.p.InlinePassImpl - Leaving only relevant operators: CInit, CInitPrimed, CommitImpliesDelivered, CommitImpliesQuorum, CommitImpliesStakeQuorum, DeliverImpliesEvidence, EventuallyCommit, Init, InitPrimed, Next, TypeInvariant
2026-02-23T17:22:31,540 [main] DEBUG a.f.a.i.p.PassChainExecutor - PASS #4: InlinePass [OK]
2026-02-23T17:22:31,546 [main] INFO  a.f.a.i.p.PassChainExecutor - PASS #5: TemporalPass
2026-02-23T17:22:31,560 [main] INFO  a.f.a.t.p.p.TemporalPassImpl -   > Rewriting temporal operators...
2026-02-23T17:22:31,579 [main] INFO  a.f.a.t.p.p.TemporalPassImpl -   > Found 1 temporal properties
2026-02-23T17:22:31,583 [main] INFO  a.f.a.t.p.p.TemporalPassImpl -   > Adding logic for loop finding
2026-02-23T17:22:31,617 [main] ERROR a.f.a.t.Tool\$ - Unhandled exception
scala.NotImplementedError: Handling fairness is not supported yet!
	at at.forsyte.apalache.tla.pp.temporal.TableauEncoder.encodeSyntaxTreeInPredicates(TableauEncoder.scala:358)
	at at.forsyte.apalache.tla.pp.temporal.TableauEncoder.\$anonfun\$encodeSyntaxTreeInPredicates\$1(TableauEncoder.scala:192)
	at scala.collection.immutable.List.map(List.scala:236)
	at scala.collection.immutable.List.map(List.scala:79)
	at at.forsyte.apalache.tla.pp.temporal.TableauEncoder.encodeSyntaxTreeInPredicates(TableauEncoder.scala:191)
	at at.forsyte.apalache.tla.pp.temporal.TableauEncoder.\$anonfun\$encodeSyntaxTreeInPredicates\$1(TableauEncoder.scala:192)
	at scala.collection.immutable.List.map(List.scala:236)
	at scala.collection.immutable.List.map(List.scala:79)
	at at.forsyte.apalache.tla.pp.temporal.TableauEncoder.encodeSyntaxTreeInPredicates(TableauEncoder.scala:191)
	at at.forsyte.apalache.tla.pp.temporal.TableauEncoder.singleTemporalToInvariant(TableauEncoder.scala:406)
	at at.forsyte.apalache.tla.pp.temporal.TableauEncoder.\$anonfun\$temporalsToInvariants\$1(TableauEncoder.scala:438)
	at scala.collection.immutable.List.map(List.scala:236)
	at scala.collection.immutable.List.map(List.scala:79)
	at at.forsyte.apalache.tla.pp.temporal.TableauEncoder.temporalsToInvariants(TableauEncoder.scala:438)
	at at.forsyte.apalache.tla.passes.pp.TemporalPassImpl.temporalToInvariants(TemporalPassImpl.scala:97)
	at at.forsyte.apalache.tla.passes.pp.TemporalPassImpl.execute(TemporalPassImpl.scala:40)
	at at.forsyte.apalache.infra.passes.PassChainExecutor.exec(PassChainExecutor.scala:71)
	at at.forsyte.apalache.infra.passes.PassChainExecutor.\$anonfun\$runPassOnModule\$3(PassChainExecutor.scala:60)
	at scala.util.Either.flatMap(Either.scala:360)
	at at.forsyte.apalache.infra.passes.PassChainExecutor.\$anonfun\$runPassOnModule\$1(PassChainExecutor.scala:58)
	at scala.collection.LinearSeqOps.foldLeft(LinearSeq.scala:183)
	at scala.collection.LinearSeqOps.foldLeft\$(LinearSeq.scala:179)
	at scala.collection.immutable.List.foldLeft(List.scala:79)
	at at.forsyte.apalache.infra.passes.PassChainExecutor.runOnPasses(PassChainExecutor.scala:51)
	at at.forsyte.apalache.infra.passes.PassChainExecutor.run(PassChainExecutor.scala:42)
	at at.forsyte.apalache.tla.tooling.opt.CheckCmd.run(CheckCmd.scala:137)
	at at.forsyte.apalache.tla.Tool\$.runCommand(Tool.scala:139)
	at at.forsyte.apalache.tla.Tool\$.run(Tool.scala:119)
	at at.forsyte.apalache.tla.Tool\$.main(Tool.scala:40)
	at at.forsyte.apalache.tla.Tool.main(Tool.scala)
```
</details>

## System information

- Apalache version: `0.52.2 build 9103560`
- OS: `Mac OS X`
- JDK version: `21.0.10`

## Triage checklist (for maintainers)

<!-- This section is for maintainers -->

- [ ] Reproduce the bug on the main development branch.
- [ ] Add the issue to the apalache GitHub project.
- [ ] If the bug is high impact, ensure someone available is assigned to fix it.

