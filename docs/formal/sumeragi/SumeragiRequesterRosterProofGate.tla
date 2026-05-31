---- MODULE SumeragiRequesterRosterProofGate ----
EXTENDS Naturals

(***************************************************************************
A bounded abstract model for local requester roster-proof detection.

This slice models `requester_has_local_roster_proof(...)`. Recovery paths use
the helper to decide whether a requester has enough locally verifiable roster
context to receive hintless block-sync material. The predicate must be true
when any one of four exact proof sources is present: a committed roster
snapshot for the requested block, an exact Commit-QC cache entry keyed by the
request height/view/epoch and vNext chain-order binding, an exact precommit
signer record for the round, or the actor's highest QC matching the exact
Commit QC header. Wrong phase, hash, height, view, epoch, or chain-order
binding evidence must not prove the requester roster.
***************************************************************************)

CONSTANT
  \* @type: Str;
  Bug

VARIABLES
  \* @type: Int;
  checked

\* @type: <<Int>>;
vars == <<checked>>

Cases == {
  "committed_snapshot",
  "commit_qc_exact",
  "precommit_record_exact",
  "highest_commit_exact",
  "all_sources",
  "no_evidence",
  "commit_qc_wrong_phase",
  "commit_qc_wrong_hash",
  "commit_qc_wrong_height",
  "commit_qc_wrong_view",
  "commit_qc_wrong_epoch",
  "commit_qc_wrong_chain_order",
  "precommit_record_wrong_hash",
  "precommit_record_wrong_height",
  "precommit_record_wrong_view",
  "precommit_record_wrong_epoch",
  "highest_wrong_phase",
  "highest_wrong_hash",
  "highest_wrong_height",
  "highest_wrong_view"
}

HasCommittedSnapshot(c) ==
  c \in {"committed_snapshot", "all_sources"}

HasExactCommitQc(c) ==
  c \in {"commit_qc_exact", "all_sources"}

HasExactPrecommitRecord(c) ==
  c \in {"precommit_record_exact", "all_sources"}

HighestExactCommit(c) ==
  c \in {"highest_commit_exact", "all_sources"}

SpecProofKnown(c) ==
  \/ HasCommittedSnapshot(c)
  \/ HasExactCommitQc(c)
  \/ HasExactPrecommitRecord(c)
  \/ HighestExactCommit(c)

ActualProofKnown(c) ==
  CASE Bug = "reject_committed_snapshot"
       /\ c = "committed_snapshot" -> FALSE
    [] Bug = "reject_exact_commit_qc"
       /\ c = "commit_qc_exact" -> FALSE
    [] Bug = "reject_exact_precommit_record"
       /\ c = "precommit_record_exact" -> FALSE
    [] Bug = "reject_exact_highest_qc"
       /\ c = "highest_commit_exact" -> FALSE
    [] Bug = "require_all_sources"
       /\ c \in {
            "committed_snapshot",
            "commit_qc_exact",
            "precommit_record_exact",
            "highest_commit_exact"
          } -> FALSE
    [] Bug = "accept_without_evidence"
       /\ c = "no_evidence" -> TRUE
    [] Bug = "accept_commit_qc_wrong_phase"
       /\ c = "commit_qc_wrong_phase" -> TRUE
    [] Bug = "accept_commit_qc_wrong_hash"
       /\ c = "commit_qc_wrong_hash" -> TRUE
    [] Bug = "accept_commit_qc_wrong_height"
       /\ c = "commit_qc_wrong_height" -> TRUE
    [] Bug = "accept_commit_qc_wrong_view"
       /\ c = "commit_qc_wrong_view" -> TRUE
    [] Bug = "accept_commit_qc_wrong_epoch"
       /\ c = "commit_qc_wrong_epoch" -> TRUE
    [] Bug = "accept_commit_qc_wrong_chain_order"
       /\ c = "commit_qc_wrong_chain_order" -> TRUE
    [] Bug = "accept_precommit_record_wrong_hash"
       /\ c = "precommit_record_wrong_hash" -> TRUE
    [] Bug = "accept_precommit_record_wrong_height"
       /\ c = "precommit_record_wrong_height" -> TRUE
    [] Bug = "accept_precommit_record_wrong_view"
       /\ c = "precommit_record_wrong_view" -> TRUE
    [] Bug = "accept_precommit_record_wrong_epoch"
       /\ c = "precommit_record_wrong_epoch" -> TRUE
    [] Bug = "accept_highest_wrong_phase"
       /\ c = "highest_wrong_phase" -> TRUE
    [] Bug = "accept_highest_wrong_hash"
       /\ c = "highest_wrong_hash" -> TRUE
    [] Bug = "accept_highest_wrong_height"
       /\ c = "highest_wrong_height" -> TRUE
    [] Bug = "accept_highest_wrong_view"
       /\ c = "highest_wrong_view" -> TRUE
    [] OTHER -> SpecProofKnown(c)

Matches(c) ==
  ActualProofKnown(c) = SpecProofKnown(c)

Init ==
  checked = 0

Next ==
  UNCHANGED vars

TypeInvariant ==
  /\ Bug \in {
       "none",
       "reject_committed_snapshot",
       "reject_exact_commit_qc",
       "reject_exact_precommit_record",
       "reject_exact_highest_qc",
       "require_all_sources",
       "accept_without_evidence",
       "accept_commit_qc_wrong_phase",
       "accept_commit_qc_wrong_hash",
       "accept_commit_qc_wrong_height",
       "accept_commit_qc_wrong_view",
       "accept_commit_qc_wrong_epoch",
       "accept_commit_qc_wrong_chain_order",
       "accept_precommit_record_wrong_hash",
       "accept_precommit_record_wrong_height",
       "accept_precommit_record_wrong_view",
       "accept_precommit_record_wrong_epoch",
       "accept_highest_wrong_phase",
       "accept_highest_wrong_hash",
       "accept_highest_wrong_height",
       "accept_highest_wrong_view"
     }
  /\ checked = 0

Safety ==
  \A c \in Cases: Matches(c)

PositiveProofSources ==
  /\ Matches("committed_snapshot")
  /\ Matches("commit_qc_exact")
  /\ Matches("precommit_record_exact")
  /\ Matches("highest_commit_exact")
  /\ Matches("all_sources")

NoEvidenceRejected ==
  Matches("no_evidence")

CommitQcExactKeyRequired ==
  /\ Matches("commit_qc_wrong_phase")
  /\ Matches("commit_qc_wrong_hash")
  /\ Matches("commit_qc_wrong_height")
  /\ Matches("commit_qc_wrong_view")
  /\ Matches("commit_qc_wrong_epoch")
  /\ Matches("commit_qc_wrong_chain_order")

PrecommitRecordExactKeyRequired ==
  /\ Matches("precommit_record_wrong_hash")
  /\ Matches("precommit_record_wrong_height")
  /\ Matches("precommit_record_wrong_view")
  /\ Matches("precommit_record_wrong_epoch")

HighestQcExactHeaderRequired ==
  /\ Matches("highest_wrong_phase")
  /\ Matches("highest_wrong_hash")
  /\ Matches("highest_wrong_height")
  /\ Matches("highest_wrong_view")

=============================================================================
