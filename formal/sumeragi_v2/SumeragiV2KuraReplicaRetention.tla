---- MODULE SumeragiV2KuraReplicaRetention ----
EXTENDS Naturals, FiniteSets

(***************************************************************************
Bounded executable safety model for authenticated Kura body retention and
eviction.  One canonical body is retained under an exact signed-finality and
complete-wire identity.  Its deterministic keeper set contains exactly
max(f + 1, configured replica floor) CommitQC signers, where
f = validator count - height-context quorum count.

Remote adverts are useful only while every selected keeper has a fresh,
signed claim for the exact finality and wire whose authenticated semantic
sender and direct transport via peer both equal the advertised keeper.  A
selected local keeper always pins the body.  Restart begins with an empty
volatile advert registry, and a
prepared eviction repeats the full authority check immediately before its
durable compaction stage is published.

The independent refresh cursor models the configured bounded historical-
advert owner: it scans an exact window in order, retains one source across
backpressure, and admits only a fixed number of probes per owner turn.  The
source ledger binds that transition slice to the process-lifetime Rust owner.

This finite model is counterexample-search evidence.  The source ledger binds
the implemented transition slices to exact Rust items, but does not turn
model exploration into a deductive refinement proof.
***************************************************************************)

CONSTANTS
  \* @type: Str;
  Mode,
  \* @type: Int;
  ValidatorCount,
  \* @type: Int;
  QuorumCount,
  \* @type: Int;
  RequiredReplicaFloor,
  \* @type: Set(Int);
  CommitQcSigners,
  \* @type: Set(Int);
  DeterministicKeepers,
  \* @type: Int;
  LocalKeeper,
  \* @type: Int;
  FaultyKeeper,
  \* @type: Int;
  NonSignerKeeper,
  \* @type: Int;
  RegistryTtl,
  \* @type: Int;
  ClockLimit,
  \* @type: Int;
  RegistryKeyCount,
  \* @type: Int;
  RegistryCapacity,
  \* @type: Int;
  RefreshHeightCount,
  \* @type: Int;
  RefreshWindow

RetentionModes ==
  {"Fixed", "ForgedSignature", "WrongFinalityIdentity",
   "WrongWireIdentity", "RelayedAdvert", "KeeperCardinalityDrift",
   "NonSignerKeeper",
   "LocalKeeperEviction", "PartialRemoteFreshness",
   "StaleAdvertSurvivesTtl", "RestartRetainsRegistry",
   "RegistryCapacityOverflow",
   "OversizedRefreshWindow", "RefreshCursorSkip",
   "SkipFinalPreStageRecheck"}

KeeperUniverse == 1..ValidatorCount

RegistryKeyUniverse == 1..RegistryKeyCount

Max2(left, right) == IF left >= right THEN left ELSE right

Min2(left, right) == IF left <= right THEN left ELSE right

FaultBound == ValidatorCount - QuorumCount

RequiredKeeperCount == Max2(FaultBound + 1, RequiredReplicaFloor)

RetentionConfiguration ==
  /\ Mode \in RetentionModes
  /\ ValidatorCount \in Nat \ {0}
  /\ QuorumCount \in 1..ValidatorCount
  /\ RequiredReplicaFloor \in 1..ValidatorCount
  /\ CommitQcSigners \subseteq KeeperUniverse
  /\ Cardinality(CommitQcSigners) >= RequiredKeeperCount
  /\ DeterministicKeepers \subseteq CommitQcSigners
  /\ Cardinality(DeterministicKeepers) = RequiredKeeperCount
  /\ LocalKeeper \in KeeperUniverse
  /\ FaultyKeeper \in DeterministicKeepers
  /\ NonSignerKeeper \in KeeperUniverse \ CommitQcSigners
  /\ RegistryTtl \in Nat \ {0}
  /\ ClockLimit > RegistryTtl
  /\ RegistryKeyCount \in Nat \ {0}
  /\ RegistryCapacity \in 1..RegistryKeyCount
  /\ RegistryKeyCount > RegistryCapacity
  /\ RefreshHeightCount \in Nat \ {0}
  /\ RefreshWindow \in 1..RefreshHeightCount

RefreshedPrefix(cursor) ==
  IF cursor = 1 THEN {} ELSE 1..(cursor - 1)

RefreshBatch(cursor, width) ==
  {height \in 1..RefreshHeightCount :
     /\ height >= cursor
     /\ height < cursor + width}

VARIABLES
  \* @type: Bool;
  authorityReady,
  \* @type: Set(Int);
  selectedKeepers,
  \* @type: Set(Int);
  registryKeepers,
  \* @type: Set(Int);
  signedAdverts,
  \* @type: (Int -> Int);
  advertSenders,
  \* @type: (Int -> Int);
  advertVias,
  \* @type: Set(Int);
  finalityBoundAdverts,
  \* @type: Set(Int);
  wireBoundAdverts,
  \* @type: (Int -> Int);
  observedAt,
  \* @type: Int;
  clock,
  \* @type: Int;
  restartCount,
  \* @type: Bool;
  restartClearedRegistry,
  \* @type: Set(Int);
  registryKeys,
  \* @type: Bool;
  bodyAvailable,
  \* @type: Bool;
  evictionPrepared,
  \* @type: Bool;
  evictionStagePublished,
  \* @type: Bool;
  publishedWithAllSelectedFresh,
  \* @type: Bool;
  publishedWithLocalUnselected,
  \* @type: Bool;
  publishedWithExactAdverts,
  \* @type: Bool;
  publishedWithFreshAuthority,
  \* @type: Bool;
  finalPreStageRechecked,
  \* @type: Int;
  refreshCursor,
  \* @type: Set(Int);
  refreshedHeights,
  \* @type: Set(Int);
  lastRefreshBatch

vars ==
  <<authorityReady, selectedKeepers, registryKeepers, signedAdverts,
    advertSenders, advertVias, finalityBoundAdverts, wireBoundAdverts,
    observedAt, clock, restartCount, restartClearedRegistry, registryKeys,
    bodyAvailable,
    evictionPrepared, evictionStagePublished,
    publishedWithAllSelectedFresh, publishedWithLocalUnselected,
    publishedWithExactAdverts, publishedWithFreshAuthority,
    finalPreStageRechecked, refreshCursor, refreshedHeights,
    lastRefreshBatch>>

AdvertFresh(keeper) ==
  /\ keeper \in registryKeepers
  /\ clock >= observedAt[keeper]
  /\ clock - observedAt[keeper] <= RegistryTtl

FreshRegistry ==
  {keeper \in registryKeepers : AdvertFresh(keeper)}

AllSelectedFresh ==
  /\ selectedKeepers # {}
  /\ selectedKeepers \subseteq FreshRegistry

AllSelectedExactAdverts ==
  /\ selectedKeepers \subseteq signedAdverts
  /\ \A keeper \in selectedKeepers :
       /\ advertSenders[keeper] = keeper
       /\ advertVias[keeper] = keeper
  /\ selectedKeepers \subseteq finalityBoundAdverts
  /\ selectedKeepers \subseteq wireBoundAdverts

ExactDeterministicKeeperAuthority ==
  /\ selectedKeepers = DeterministicKeepers
  /\ Cardinality(selectedKeepers) = RequiredKeeperCount
  /\ selectedKeepers \subseteq CommitQcSigners

EvictionAuthorityFresh ==
  /\ authorityReady
  /\ ExactDeterministicKeeperAuthority
  /\ LocalKeeper \notin selectedKeepers
  /\ AllSelectedFresh
  /\ AllSelectedExactAdverts

Init ==
  /\ RetentionConfiguration
  /\ authorityReady = FALSE
  /\ selectedKeepers = {}
  /\ registryKeepers = {}
  /\ signedAdverts = {}
  /\ advertSenders = [keeper \in KeeperUniverse |-> keeper]
  /\ advertVias = [keeper \in KeeperUniverse |-> keeper]
  /\ finalityBoundAdverts = {}
  /\ wireBoundAdverts = {}
  /\ observedAt = [keeper \in KeeperUniverse |-> 0]
  /\ clock = 0
  /\ restartCount = 0
  /\ restartClearedRegistry = TRUE
  /\ registryKeys = {}
  /\ bodyAvailable = TRUE
  /\ evictionPrepared = FALSE
  /\ evictionStagePublished = FALSE
  /\ publishedWithAllSelectedFresh = FALSE
  /\ publishedWithLocalUnselected = FALSE
  /\ publishedWithExactAdverts = FALSE
  /\ publishedWithFreshAuthority = FALSE
  /\ finalPreStageRechecked = FALSE
  /\ refreshCursor = 1
  /\ refreshedHeights = {}
  /\ lastRefreshBatch = {}

InstallAuthenticatedAuthority ==
  /\ ~authorityReady
  /\ authorityReady' = TRUE
  /\ selectedKeepers' =
       IF Mode = "KeeperCardinalityDrift"
       THEN {keeper \in DeterministicKeepers : keeper # FaultyKeeper}
       ELSE IF Mode = "NonSignerKeeper"
            THEN {keeper \in DeterministicKeepers : keeper # FaultyKeeper}
                   \cup {NonSignerKeeper}
            ELSE DeterministicKeepers
  /\ UNCHANGED <<registryKeepers, signedAdverts,
                 advertSenders, advertVias, finalityBoundAdverts,
                 wireBoundAdverts, observedAt, clock, restartCount,
                 restartClearedRegistry, registryKeys, bodyAvailable,
                 evictionPrepared,
                 evictionStagePublished, publishedWithAllSelectedFresh,
                 publishedWithLocalUnselected, publishedWithExactAdverts,
                 publishedWithFreshAuthority, finalPreStageRechecked,
                 refreshCursor, refreshedHeights, lastRefreshBatch>>

AdmitAdvert(keeper) ==
  /\ authorityReady
  /\ keeper \in selectedKeepers
  /\ bodyAvailable
  /\ registryKeepers' = registryKeepers \cup {keeper}
  /\ signedAdverts' =
       IF Mode = "ForgedSignature" /\ keeper = FaultyKeeper
       THEN signedAdverts
       ELSE signedAdverts \cup {keeper}
  /\ advertSenders' = [advertSenders EXCEPT ![keeper] = keeper]
  /\ advertVias' =
       [advertVias EXCEPT
          ![keeper] = IF Mode = "RelayedAdvert" /\ keeper = FaultyKeeper
                     THEN NonSignerKeeper
                     ELSE keeper]
  /\ finalityBoundAdverts' =
       IF Mode = "WrongFinalityIdentity" /\ keeper = FaultyKeeper
       THEN finalityBoundAdverts
       ELSE finalityBoundAdverts \cup {keeper}
  /\ wireBoundAdverts' =
       IF Mode = "WrongWireIdentity" /\ keeper = FaultyKeeper
       THEN wireBoundAdverts
       ELSE wireBoundAdverts \cup {keeper}
  /\ observedAt' = [observedAt EXCEPT ![keeper] = clock]
  /\ UNCHANGED <<authorityReady, selectedKeepers, clock, restartCount,
                 restartClearedRegistry, registryKeys, bodyAvailable,
                 evictionPrepared,
                 evictionStagePublished, publishedWithAllSelectedFresh,
                 publishedWithLocalUnselected, publishedWithExactAdverts,
                 publishedWithFreshAuthority, finalPreStageRechecked,
                 refreshCursor, refreshedHeights, lastRefreshBatch>>

Tick ==
  /\ clock < ClockLimit
  /\ LET nextClock == clock + 1
         survivors ==
           {keeper \in registryKeepers :
              nextClock - observedAt[keeper] <= RegistryTtl}
     IN /\ clock' = nextClock
        /\ registryKeepers' =
             IF Mode = "StaleAdvertSurvivesTtl"
             THEN registryKeepers
             ELSE survivors
  /\ UNCHANGED <<authorityReady, selectedKeepers, signedAdverts,
                 advertSenders, advertVias, finalityBoundAdverts,
                 wireBoundAdverts, observedAt, restartCount,
                 restartClearedRegistry, registryKeys, bodyAvailable,
                 evictionPrepared, evictionStagePublished,
                 publishedWithAllSelectedFresh,
                 publishedWithLocalUnselected, publishedWithExactAdverts,
                 publishedWithFreshAuthority, finalPreStageRechecked,
                 refreshCursor, refreshedHeights, lastRefreshBatch>>

CrashAndRestart ==
  /\ restartCount = 0
  /\ registryKeepers # {}
  /\ restartCount' = 1
  /\ restartClearedRegistry' = (Mode # "RestartRetainsRegistry")
  /\ registryKeepers' =
       IF Mode = "RestartRetainsRegistry" THEN registryKeepers ELSE {}
  /\ signedAdverts' =
       IF Mode = "RestartRetainsRegistry" THEN signedAdverts ELSE {}
  /\ advertSenders' =
       IF Mode = "RestartRetainsRegistry"
       THEN advertSenders
       ELSE [keeper \in KeeperUniverse |-> keeper]
  /\ advertVias' =
       IF Mode = "RestartRetainsRegistry"
       THEN advertVias
       ELSE [keeper \in KeeperUniverse |-> keeper]
  /\ finalityBoundAdverts' =
       IF Mode = "RestartRetainsRegistry" THEN finalityBoundAdverts ELSE {}
  /\ wireBoundAdverts' =
       IF Mode = "RestartRetainsRegistry" THEN wireBoundAdverts ELSE {}
  /\ observedAt' =
       IF Mode = "RestartRetainsRegistry"
       THEN observedAt
       ELSE [keeper \in KeeperUniverse |-> 0]
  /\ registryKeys' =
       IF Mode = "RestartRetainsRegistry" THEN registryKeys ELSE {}
  /\ evictionPrepared' = FALSE
  /\ UNCHANGED <<authorityReady, selectedKeepers, clock, bodyAvailable,
                 evictionStagePublished, publishedWithAllSelectedFresh,
                 publishedWithLocalUnselected, publishedWithExactAdverts,
                 publishedWithFreshAuthority, finalPreStageRechecked,
                 refreshCursor, refreshedHeights, lastRefreshBatch>>

AdmitRegistryKey(key) ==
  /\ key \in RegistryKeyUniverse
  /\ key \notin registryKeys
  /\ IF Mode = "RegistryCapacityOverflow"
     THEN TRUE
     ELSE Cardinality(registryKeys) < RegistryCapacity
  /\ registryKeys' = registryKeys \cup {key}
  /\ UNCHANGED <<authorityReady, selectedKeepers, registryKeepers,
                 signedAdverts, advertSenders, advertVias,
                 finalityBoundAdverts, wireBoundAdverts, observedAt, clock,
                 restartCount, restartClearedRegistry, bodyAvailable,
                 evictionPrepared, evictionStagePublished,
                 publishedWithAllSelectedFresh,
                 publishedWithLocalUnselected, publishedWithExactAdverts,
                 publishedWithFreshAuthority, finalPreStageRechecked,
                 refreshCursor, refreshedHeights, lastRefreshBatch>>

RefreshAdvertWindow ==
  /\ refreshCursor <= RefreshHeightCount
  /\ LET batch ==
           IF Mode = "OversizedRefreshWindow"
           THEN RefreshBatch(refreshCursor, RefreshWindow + 1)
           ELSE RefreshBatch(refreshCursor, RefreshWindow)
         exactNext == refreshCursor + Cardinality(batch)
         nextCursor ==
           IF Mode = "RefreshCursorSkip"
           THEN Min2(RefreshHeightCount + 1, exactNext + 1)
           ELSE exactNext
     IN /\ lastRefreshBatch' = batch
        /\ refreshedHeights' = refreshedHeights \cup batch
        /\ refreshCursor' = nextCursor
  /\ UNCHANGED <<authorityReady, selectedKeepers, registryKeepers,
                 signedAdverts, advertSenders, advertVias,
                 finalityBoundAdverts, wireBoundAdverts, observedAt, clock,
                 restartCount, restartClearedRegistry, registryKeys,
                 bodyAvailable, evictionPrepared, evictionStagePublished,
                 publishedWithAllSelectedFresh,
                 publishedWithLocalUnselected, publishedWithExactAdverts,
                 publishedWithFreshAuthority, finalPreStageRechecked>>

PrepareEviction ==
  /\ authorityReady
  /\ bodyAvailable
  /\ ~evictionPrepared
  /\ IF Mode = "LocalKeeperEviction"
     THEN AllSelectedFresh /\ AllSelectedExactAdverts
     ELSE IF Mode = "PartialRemoteFreshness"
          THEN /\ Cardinality(selectedKeepers \cap FreshRegistry) > 0
               /\ ~AllSelectedFresh
               /\ selectedKeepers \cap FreshRegistry
                    \subseteq signedAdverts
               /\ selectedKeepers \cap FreshRegistry
                    \subseteq finalityBoundAdverts
               /\ selectedKeepers \cap FreshRegistry
                    \subseteq wireBoundAdverts
          ELSE EvictionAuthorityFresh
  /\ evictionPrepared' = TRUE
  /\ UNCHANGED <<authorityReady, selectedKeepers, registryKeepers,
                 signedAdverts, advertSenders, advertVias,
                 finalityBoundAdverts, wireBoundAdverts, observedAt, clock,
                 restartCount, restartClearedRegistry, registryKeys,
                 bodyAvailable, evictionStagePublished,
                 publishedWithAllSelectedFresh,
                 publishedWithLocalUnselected, publishedWithExactAdverts,
                 publishedWithFreshAuthority, finalPreStageRechecked,
                 refreshCursor, refreshedHeights, lastRefreshBatch>>

PublishEvictionStage ==
  /\ evictionPrepared
  /\ bodyAvailable
  /\ ~evictionStagePublished
  /\ IF Mode \in
          {"LocalKeeperEviction", "PartialRemoteFreshness",
           "SkipFinalPreStageRecheck"}
     THEN TRUE
     ELSE EvictionAuthorityFresh
  /\ evictionStagePublished' = TRUE
  /\ bodyAvailable' = FALSE
  /\ publishedWithAllSelectedFresh' = AllSelectedFresh
  /\ publishedWithLocalUnselected' =
       (LocalKeeper \notin selectedKeepers)
  /\ publishedWithExactAdverts' = AllSelectedExactAdverts
  /\ publishedWithFreshAuthority' = EvictionAuthorityFresh
  /\ finalPreStageRechecked' = (Mode # "SkipFinalPreStageRecheck")
  /\ UNCHANGED <<authorityReady, selectedKeepers, registryKeepers,
                 signedAdverts, advertSenders, advertVias,
                 finalityBoundAdverts, wireBoundAdverts, observedAt, clock,
                 restartCount, restartClearedRegistry, registryKeys,
                 evictionPrepared, refreshCursor, refreshedHeights,
                 lastRefreshBatch>>

Next ==
  \/ InstallAuthenticatedAuthority
  \/ \E keeper \in KeeperUniverse : AdmitAdvert(keeper)
  \/ \E key \in RegistryKeyUniverse : AdmitRegistryKey(key)
  \/ Tick
  \/ CrashAndRestart
  \/ RefreshAdvertWindow
  \/ PrepareEviction
  \/ PublishEvictionStage

KuraReplicaRetentionTypeInvariant ==
  /\ RetentionConfiguration
  /\ authorityReady \in BOOLEAN
  /\ selectedKeepers \subseteq KeeperUniverse
  /\ registryKeepers \subseteq KeeperUniverse
  /\ signedAdverts \subseteq KeeperUniverse
  /\ advertSenders \in [KeeperUniverse -> KeeperUniverse]
  /\ advertVias \in [KeeperUniverse -> KeeperUniverse]
  /\ finalityBoundAdverts \subseteq KeeperUniverse
  /\ wireBoundAdverts \subseteq KeeperUniverse
  /\ observedAt \in [KeeperUniverse -> Nat]
  /\ clock \in 0..ClockLimit
  /\ restartCount \in 0..1
  /\ restartClearedRegistry \in BOOLEAN
  /\ registryKeys \subseteq RegistryKeyUniverse
  /\ bodyAvailable \in BOOLEAN
  /\ evictionPrepared \in BOOLEAN
  /\ evictionStagePublished \in BOOLEAN
  /\ publishedWithAllSelectedFresh \in BOOLEAN
  /\ publishedWithLocalUnselected \in BOOLEAN
  /\ publishedWithExactAdverts \in BOOLEAN
  /\ publishedWithFreshAuthority \in BOOLEAN
  /\ finalPreStageRechecked \in BOOLEAN
  /\ refreshCursor \in 1..(RefreshHeightCount + 1)
  /\ refreshedHeights \subseteq 1..RefreshHeightCount
  /\ lastRefreshBatch \subseteq 1..RefreshHeightCount

KRAdmittedAdvertsSigned ==
  registryKeepers \subseteq signedAdverts

KRAdmittedAdvertsDirectAuthenticated ==
  \A keeper \in registryKeepers :
    /\ advertSenders[keeper] = keeper
    /\ advertVias[keeper] = keeper

KRAdmittedAdvertsBindExactFinality ==
  registryKeepers \subseteq finalityBoundAdverts

KRAdmittedAdvertsBindExactWire ==
  registryKeepers \subseteq wireBoundAdverts

KRDeterministicFPlusOneKeepers ==
  authorityReady => ExactDeterministicKeeperAuthority

KRLocalSelectedKeeperPinsBody ==
  bodyAvailable \/ publishedWithLocalUnselected

KREvictionRequiresAllSelectedRemoteFresh ==
  bodyAvailable \/ publishedWithAllSelectedFresh

KRExpiredAdvertsCannotAuthorize ==
  registryKeepers \subseteq FreshRegistry

KRRestartClearsAdvertRegistry ==
  restartCount = 0 \/ restartClearedRegistry

KRRegistryCapacityBounded ==
  Cardinality(registryKeys) <= RegistryCapacity

KRRefreshWindowBounded ==
  Cardinality(lastRefreshBatch) <= RefreshWindow

KRRefreshCursorExact ==
  refreshedHeights = RefreshedPrefix(refreshCursor)

KRFinalPreStageRecheck ==
  bodyAvailable \/
    /\ finalPreStageRechecked
    /\ publishedWithFreshAuthority

KuraReplicaRetentionProductionRefinementObligation ==
  /\ KuraReplicaRetentionTypeInvariant
  /\ KRAdmittedAdvertsSigned
  /\ KRAdmittedAdvertsDirectAuthenticated
  /\ KRAdmittedAdvertsBindExactFinality
  /\ KRAdmittedAdvertsBindExactWire
  /\ KRDeterministicFPlusOneKeepers
  /\ KRLocalSelectedKeeperPinsBody
  /\ KREvictionRequiresAllSelectedRemoteFresh
  /\ KRExpiredAdvertsCannotAuthorize
  /\ KRRestartClearsAdvertRegistry
  /\ KRRegistryCapacityBounded
  /\ KRRefreshWindowBounded
  /\ KRRefreshCursorExact
  /\ KRFinalPreStageRecheck

====
