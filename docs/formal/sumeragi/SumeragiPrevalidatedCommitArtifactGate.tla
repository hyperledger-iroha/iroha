---- MODULE SumeragiPrevalidatedCommitArtifactGate ----

EXTENDS Integers

(***************************************************************************
A bounded abstract model for prevalidated commit artifact trust gates.

`trusted_prevalidated_commit_artifact(...)` may reuse a prevalidated artifact
only when the artifact matches the candidate block height/view/hash and a
present COMMIT QC matches the same subject plus parent/post-state roots.
`prevalidated_roots_match_witness(...)` then requires a present execution
witness whose parent and post-state roots reproduce the trusted artifact.
***************************************************************************)

CONSTANT
  \* @type: Int;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Cases == {
  "valid",
  "no_artifact",
  "artifact_hash_mismatch",
  "artifact_height_mismatch",
  "artifact_view_mismatch",
  "no_qc",
  "qc_hash_mismatch",
  "qc_height_mismatch",
  "qc_view_mismatch",
  "qc_prepare_phase",
  "parent_root_mismatch",
  "post_root_mismatch",
  "no_witness",
  "witness_parent_mismatch",
  "witness_post_mismatch"
}

BlockHash == 1
BlockHeight == 10
BlockView == 2
ArtifactParentRoot == 101
ArtifactPostRoot == 202

ArtifactPresent(c) == c # "no_artifact"

ArtifactHash(c) ==
  IF c = "artifact_hash_mismatch" THEN 99 ELSE BlockHash

ArtifactHeight(c) ==
  IF c = "artifact_height_mismatch" THEN 11 ELSE BlockHeight

ArtifactView(c) ==
  IF c = "artifact_view_mismatch" THEN 3 ELSE BlockView

ArtifactParent(c) == ArtifactParentRoot

ArtifactPost(c) == ArtifactPostRoot

QcPresent(c) == c # "no_qc"

QcHash(c) ==
  IF c = "qc_hash_mismatch" THEN 99 ELSE BlockHash

QcHeight(c) ==
  IF c = "qc_height_mismatch" THEN 11 ELSE BlockHeight

QcView(c) ==
  IF c = "qc_view_mismatch" THEN 3 ELSE BlockView

QcPhaseCommit(c) == c # "qc_prepare_phase"

QcParent(c) ==
  IF c = "parent_root_mismatch" THEN 303 ELSE ArtifactParentRoot

QcPost(c) ==
  IF c = "post_root_mismatch" THEN 404 ELSE ArtifactPostRoot

WitnessPresent(c) == c # "no_witness"

WitnessParent(c) ==
  IF c = "witness_parent_mismatch" THEN 303 ELSE ArtifactParent(c)

WitnessPost(c) ==
  IF c = "witness_post_mismatch" THEN 404 ELSE ArtifactPost(c)

SpecTrustedArtifact(c) ==
  /\ ArtifactPresent(c)
  /\ ArtifactHash(c) = BlockHash
  /\ ArtifactHeight(c) = BlockHeight
  /\ ArtifactView(c) = BlockView
  /\ QcPresent(c)
  /\ QcHash(c) = BlockHash
  /\ QcHeight(c) = BlockHeight
  /\ QcView(c) = BlockView
  /\ QcPhaseCommit(c)
  /\ QcParent(c) = ArtifactParent(c)
  /\ QcPost(c) = ArtifactPost(c)

ActualTrustedArtifact(c) ==
  /\ (ArtifactPresent(c) \/ Bug = 1)
  /\ (ArtifactHash(c) = BlockHash \/ Bug = 2)
  /\ (ArtifactHeight(c) = BlockHeight \/ Bug = 3)
  /\ (ArtifactView(c) = BlockView \/ Bug = 4)
  /\ (QcPresent(c) \/ Bug = 5)
  /\ (QcHash(c) = BlockHash \/ Bug = 6)
  /\ (QcHeight(c) = BlockHeight \/ Bug = 7)
  /\ (QcView(c) = BlockView \/ Bug = 8)
  /\ (QcPhaseCommit(c) \/ Bug = 9)
  /\ (QcParent(c) = ArtifactParent(c) \/ Bug = 10)
  /\ (QcPost(c) = ArtifactPost(c) \/ Bug = 11)

SpecRootsMatchWitness(c) ==
  /\ WitnessPresent(c)
  /\ WitnessParent(c) = ArtifactParent(c)
  /\ WitnessPost(c) = ArtifactPost(c)

ActualRootsMatchWitness(c) ==
  /\ (WitnessPresent(c) \/ Bug = 12)
  /\ (WitnessParent(c) = ArtifactParent(c) \/ Bug = 13)
  /\ (WitnessPost(c) = ArtifactPost(c) \/ Bug = 14)

SpecCommitAccepts(c) ==
  IF SpecTrustedArtifact(c) THEN SpecRootsMatchWitness(c) ELSE TRUE

ActualCommitAccepts(c) ==
  IF ActualTrustedArtifact(c) THEN ActualRootsMatchWitness(c) ELSE TRUE

\* @type: Str => <<Bool, Bool, Bool>>;
SpecCase(c) ==
  <<SpecTrustedArtifact(c), SpecRootsMatchWitness(c), SpecCommitAccepts(c)>>

\* @type: Str => <<Bool, Bool, Bool>>;
ActualCase(c) ==
  <<ActualTrustedArtifact(c), ActualRootsMatchWitness(c), ActualCommitAccepts(c)>>

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  checked = 0

TrustedArtifactExact ==
  \A c \in Cases:
    ActualTrustedArtifact(c) = SpecTrustedArtifact(c)

WitnessRootsExact ==
  \A c \in Cases:
    ActualRootsMatchWitness(c) = SpecRootsMatchWitness(c)

CommitAcceptsExact ==
  \A c \in Cases:
    ActualCommitAccepts(c) = SpecCommitAccepts(c)

PrevalidatedCommitArtifactExactness ==
  /\ TrustedArtifactExact
  /\ WitnessRootsExact
  /\ CommitAcceptsExact

CaseTupleExact ==
  \A c \in Cases: ActualCase(c) = SpecCase(c)

SafetyFast ==
  /\ PrevalidatedCommitArtifactExactness
  /\ CaseTupleExact

BugRequiresArtifact ==
  ActualCase("no_artifact") = SpecCase("no_artifact")

BugArtifactBlockHash ==
  ActualCase("artifact_hash_mismatch") = SpecCase("artifact_hash_mismatch")

BugArtifactHeight ==
  ActualCase("artifact_height_mismatch") = SpecCase("artifact_height_mismatch")

BugArtifactView ==
  ActualCase("artifact_view_mismatch") = SpecCase("artifact_view_mismatch")

BugRequiresCommitQc ==
  ActualCase("no_qc") = SpecCase("no_qc")

BugCommitQcSubject ==
  ActualCase("qc_hash_mismatch") = SpecCase("qc_hash_mismatch")

BugCommitQcHeight ==
  ActualCase("qc_height_mismatch") = SpecCase("qc_height_mismatch")

BugCommitQcView ==
  ActualCase("qc_view_mismatch") = SpecCase("qc_view_mismatch")

BugCommitQcPhase ==
  ActualCase("qc_prepare_phase") = SpecCase("qc_prepare_phase")

BugParentRoot ==
  ActualCase("parent_root_mismatch") = SpecCase("parent_root_mismatch")

BugPostRoot ==
  ActualCase("post_root_mismatch") = SpecCase("post_root_mismatch")

BugRequiresWitness ==
  ActualCase("no_witness") = SpecCase("no_witness")

BugWitnessParentRoot ==
  ActualCase("witness_parent_mismatch") = SpecCase("witness_parent_mismatch")

BugWitnessPostRoot ==
  ActualCase("witness_post_mismatch") = SpecCase("witness_post_mismatch")

====
