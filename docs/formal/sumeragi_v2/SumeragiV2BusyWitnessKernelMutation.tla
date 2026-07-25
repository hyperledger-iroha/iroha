---- MODULE SumeragiV2BusyWitnessKernelMutation ----

(***************************************************************************
Bounded regression for the stale Busy-witness/readiness boundary.

The retired global invariant required a Busy node to have a matching
Completion in the active scheduler carrier, but did not require the
serialized Core owner to be ready for that Completion.  The negative initial
state therefore has:

  * one pending Proposal WAL owner;
  * that proposal already present in proposalIntents; and
  * one matching PersistProposal Completion in the active carrier.

The Completion is not executable because PersistProposal requires the intent
to be absent.  The old runtime branch moves it to the deferred carrier while
the pending Proposal remains Busy.  Because deferred work is deliberately
excluded from the active Busy-witness carrier, the old witness invariant is
false in the next state.

The repaired model combines serialized-owner uniqueness with both exact
Proposal WAL/signing readiness clauses.  RepairedInit enumerates the complete
two-by-two truth table of a competing sign owner and proposal-intent
membership.  The one valid row advances PersistProposal into its exact
SignProposal owner/witness; all three invalid rows are blocked without moving
the sole active witness.
***************************************************************************)

VARIABLES phase, pendingProposal, signProposal, proposalInIntents,
          activeCompletion, deferredCompletion

vars ==
  <<phase, pendingProposal, signProposal, proposalInIntents,
    activeCompletion, deferredCompletion>>

NodeBusy == pendingProposal \/ signProposal

(***************************************************************************
This is the retired global witness predicate: a deferred Completion is not an
active Busy witness because it cannot drain until the node becomes idle.
***************************************************************************)
OldActiveBusyWitnessInvariant ==
  NodeBusy => activeCompletion

SerializedOwnerUnique ==
  ~(pendingProposal /\ signProposal)

ProposalWalReady ==
  pendingProposal => ~proposalInIntents

ProposalSignReady ==
  signProposal => proposalInIntents

CombinedSerializedReadinessKernel ==
  /\ SerializedOwnerUnique
  /\ ProposalWalReady
  /\ ProposalSignReady

OldInit ==
  /\ phase = "Queued"
  /\ pendingProposal = TRUE
  /\ signProposal = FALSE
  /\ proposalInIntents = TRUE
  /\ activeCompletion = TRUE
  /\ deferredCompletion = FALSE

RepairedInit ==
  /\ phase = "Queued"
  /\ pendingProposal = TRUE
  /\ signProposal \in BOOLEAN
  /\ proposalInIntents \in BOOLEAN
  /\ activeCompletion = TRUE
  /\ deferredCompletion = FALSE

OldDeferStaleBusyWitness ==
  /\ phase = "Queued"
  /\ pendingProposal
  /\ ~ProposalWalReady
  /\ activeCompletion
  /\ ~deferredCompletion
  /\ activeCompletion' = FALSE
  /\ deferredCompletion' = TRUE
  /\ phase' = "Deferred"
  /\ UNCHANGED <<pendingProposal, signProposal, proposalInIntents>>

(***************************************************************************
The exact valid Proposal chain changes the sole serialized owner and replaces
the PersistProposal Completion with its causal SignProposal Completion.
Invalid serialized/readiness rows retain the active witness and are marked
Blocked; in particular, none can take the old defer transition.
***************************************************************************)
CombinedKernelRuntime ==
  /\ phase = "Queued"
  /\ IF CombinedSerializedReadinessKernel
     THEN /\ pendingProposal' = FALSE
          /\ signProposal' = TRUE
          /\ proposalInIntents' = TRUE
          /\ activeCompletion' = TRUE
          /\ deferredCompletion' = FALSE
          /\ phase' = "Executed"
     ELSE /\ UNCHANGED
                <<pendingProposal, signProposal, proposalInIntents,
                  activeCompletion, deferredCompletion>>
          /\ phase' = "Blocked"

TerminalStutter ==
  /\ phase \in {"Deferred", "Executed", "Blocked"}
  /\ UNCHANGED vars

OldNext == OldDeferStaleBusyWitness \/ TerminalStutter
RepairedNext == CombinedKernelRuntime \/ TerminalStutter

OldSpec == OldInit /\ [][OldNext]_vars
RepairedSpec == RepairedInit /\ [][RepairedNext]_vars

CombinedKernelOutcomeIsExact ==
  /\ (phase = "Executed" =>
        /\ ~pendingProposal
        /\ signProposal
        /\ proposalInIntents
        /\ activeCompletion
        /\ ~deferredCompletion
        /\ CombinedSerializedReadinessKernel)
  /\ (phase = "Blocked" =>
        /\ pendingProposal
        /\ activeCompletion
        /\ ~deferredCompletion
        /\ ~CombinedSerializedReadinessKernel)
  /\ phase # "Deferred"

RepairedCombinedKernelSafety ==
  /\ OldActiveBusyWitnessInvariant
  /\ CombinedKernelOutcomeIsExact

=============================================================================
