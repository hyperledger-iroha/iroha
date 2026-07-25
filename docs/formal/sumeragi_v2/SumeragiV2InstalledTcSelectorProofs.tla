---- MODULE SumeragiV2InstalledTcSelectorProofs ----
EXTENDS SumeragiV2Proofs, FunctionTheorems

(***************************************************************************
Exact installed-TC proposal selector.

The reducer stores the complete most recently installed timeout certificate
in `lastInstalledTc[node]`.  Proposal construction reads that value directly.
There is no search over `installedTCs`, no rank/subject reconstruction, and no
CHOOSE tie breaker.  The retained history is evidence that the exact selected
certificate was durably installed; it is not an alternate selector.
***************************************************************************)

LastInstalledTcEntry(node) ==
  [node |-> node, tc |-> lastInstalledTc[node]]

ExactCurrentInstalledTcEntries(node, roundView) ==
  {installed \in installedTCs:
    /\ installed = LastInstalledTcEntry(node)
    /\ lastInstalledTc[node] # NoTimeoutCertificate
    /\ lastInstalledTc[node].context = context
    /\ lastInstalledTc[node].view + 1 = roundView}

ExactSelectedInstalledTcForRound(node, roundView) ==
  LastInstalledTcEntry(node)

InstalledTcExactSelectionInvariant ==
  \A node \in ValidatorIds:
    lastInstalledTc[node] # NoTimeoutCertificate
      => /\ LastInstalledTcEntry(node) \in installedTCs
         /\ TcWellTyped(lastInstalledTc[node])
         /\ lastInstalledTc[node].highestPrepareQc
              \in PrepareQcOptionSet

StrongInstalledTcExactSelectionInvariant ==
  /\ StrongInductiveInvariant
  /\ InstalledTcExactSelectionInvariant

THEOREM StrongInductiveInvariantProjectsExactInstalledTcSelection ==
  StrongInductiveInvariant => InstalledTcExactSelectionInvariant
PROOF
  <1>1. ASSUME StrongInductiveInvariant
         PROVE InstalledTcExactSelectionInvariant
    <2>1. TypeInvariant
      BY <1>1 DEF StrongInductiveInvariant, Safety
    <2>2. ASSUME NEW node \in ValidatorIds,
                  lastInstalledTc[node] # NoTimeoutCertificate
           PROVE /\ LastInstalledTcEntry(node) \in installedTCs
                 /\ TcWellTyped(lastInstalledTc[node])
                 /\ lastInstalledTc[node].highestPrepareQc
                      \in PrepareQcOptionSet
      <3>1. LastInstalledTcEntry(node) \in installedTCs
        BY <2>1, <2>2
           DEF TypeInvariant, LastInstalledTcEntry
      <3>2. TcWellTyped(lastInstalledTc[node])
        BY <2>1, <3>1, Isa
           DEF TypeInvariant, LastInstalledTcEntry
      <3>3. lastInstalledTc[node].highestPrepareQc
               \in PrepareQcOptionSet
        BY <3>2 DEF TcWellTyped
      <3> QED BY <3>1, <3>2, <3>3
    <2> QED BY <2>2
         DEF InstalledTcExactSelectionInvariant
  <1> QED BY <1>1

THEOREM CoreSpecAtAlwaysStrongInstalledTcExactSelectionInvariant ==
  \A initialContext:
    CoreSpecAt(initialContext)
      => []StrongInstalledTcExactSelectionInvariant
PROOF
  <1>1. ASSUME NEW initialContext
         PROVE CoreSpecAt(initialContext)
                 => []StrongInstalledTcExactSelectionInvariant
    <2>1. CoreSpecAt(initialContext)
             => []StrongInductiveInvariant
      BY CoreSpecAtAlwaysStrongInductiveInvariant
    <2>2. [](StrongInductiveInvariant
               => InstalledTcExactSelectionInvariant)
      BY StrongInductiveInvariantProjectsExactInstalledTcSelection, PTL
    <2> QED BY <2>1, <2>2, PTL
         DEF StrongInstalledTcExactSelectionInvariant
  <1> QED BY <1>1

THEOREM ExactInstalledTcSelectorIsUnique ==
  \A node, roundView:
    (/\ StrongInstalledTcExactSelectionInvariant
     /\ ExactCurrentInstalledTcEntries(node, roundView) # {})
      => LET selected ==
               ExactSelectedInstalledTcForRound(node, roundView)
             current ==
               ExactCurrentInstalledTcEntries(node, roundView)
         IN /\ selected \in current
            /\ \A other \in current: other = selected
            /\ selected.tc = lastInstalledTc[node]
            /\ selected.tc.highestPrepareQc =
                 lastInstalledTc[node].highestPrepareQc
BY Isa
   DEF StrongInstalledTcExactSelectionInvariant,
       InstalledTcExactSelectionInvariant,
       ExactSelectedInstalledTcForRound,
       ExactCurrentInstalledTcEntries,
       LastInstalledTcEntry

(***************************************************************************
The proposal postcondition keeps the complete certificate and PrepareQC
identities.  The final disjunction is only the ordinary SafeToPrepare check;
it never identifies two QCs by their rank and subject.
***************************************************************************)

THEOREM BeginLocalProposalUsesExactInstalledTcAndPreservesLock ==
  \A node, subject:
    (/\ StrongInstalledTcExactSelectionInvariant
     /\ BeginLocalProposal(node, subject)
     /\ nodeView[node] > 0)
      => LET roundView == nodeView[node]
             selected ==
               ExactSelectedInstalledTcForRound(node, roundView)
             current ==
               ExactCurrentInstalledTcEntries(node, roundView)
             proposal == LocalProposalFor(node, subject)
         IN /\ selected \in current
            /\ \A other \in current: other = selected
            /\ proposal.timeoutCertificate = selected.tc
            /\ proposal.highestPrepareQc =
                 selected.tc.highestPrepareQc
            /\ proposal.justifyRank =
                 PrepareQcRank(selected.tc.highestPrepareQc)
            /\ proposal.justifySubject =
                 PrepareQcSubject(selected.tc.highestPrepareQc)
            /\ (lockRank[node] # NoRank
                  => \/ proposal.subject = lockSubject[node]
                     \/ /\ selected.tc.highestPrepareQc # NoPrepareQC
                        /\ selected.tc.highestPrepareQc.view
                             > lockRank[node]
                        /\ selected.tc.highestPrepareQc.subject
                             = proposal.subject)
PROOF
  <1>1. ASSUME NEW node,
              NEW subject,
              StrongInstalledTcExactSelectionInvariant,
              BeginLocalProposal(node, subject),
              nodeView[node] > 0
         PROVE LET roundView == nodeView[node]
                   selected ==
                     ExactSelectedInstalledTcForRound(node, roundView)
                   current ==
                     ExactCurrentInstalledTcEntries(node, roundView)
                   proposal == LocalProposalFor(node, subject)
               IN /\ selected \in current
                  /\ \A other \in current: other = selected
                  /\ proposal.timeoutCertificate = selected.tc
                  /\ proposal.highestPrepareQc =
                       selected.tc.highestPrepareQc
                  /\ proposal.justifyRank =
                       PrepareQcRank(selected.tc.highestPrepareQc)
                  /\ proposal.justifySubject =
                       PrepareQcSubject(selected.tc.highestPrepareQc)
                  /\ (lockRank[node] # NoRank
                        => \/ proposal.subject = lockSubject[node]
                           \/ /\ selected.tc.highestPrepareQc
                                    # NoPrepareQC
                              /\ selected.tc.highestPrepareQc.view
                                   > lockRank[node]
                              /\ selected.tc.highestPrepareQc.subject
                                   = proposal.subject)
    <2> DEFINE roundView == nodeView[node]
    <2> DEFINE selected ==
           ExactSelectedInstalledTcForRound(node, roundView)
    <2> DEFINE current ==
           ExactCurrentInstalledTcEntries(node, roundView)
    <2> DEFINE proposal == LocalProposalFor(node, subject)
    <2>1. /\ node \in ValidatorIds
           /\ lastInstalledTc[node] # NoTimeoutCertificate
           /\ LastInstalledTcEntry(node) \in installedTCs
           /\ lastInstalledTc[node].context = context
           /\ lastInstalledTc[node].view + 1 = roundView
           /\ ProposalWireValidFor(node, proposal)
      BY <1>1, IsaT(180)
         DEF StrongInstalledTcExactSelectionInvariant,
             StrongInductiveInvariant, Safety, TypeInvariant,
             InstalledTcExactSelectionInvariant,
             BeginLocalProposal, ProposalWireValidFor, ProposalJustified,
             roundView, proposal, LastInstalledTcEntry
    <2>2. current # {}
      BY <2>1
         DEF current, ExactCurrentInstalledTcEntries
    <2>3. /\ selected \in current
           /\ \A other \in current: other = selected
           /\ selected.tc = lastInstalledTc[node]
           /\ selected.tc.highestPrepareQc =
                lastInstalledTc[node].highestPrepareQc
      BY <1>1, <2>2, ExactInstalledTcSelectorIsUnique
         DEF selected, current
    <2>4. roundView # 0
      BY <1>1, SMT DEF roundView
    <2>5. LocalProposalJustification(node) =
             [timeoutCertificate |-> lastInstalledTc[node],
              highestPrepareQc |->
                lastInstalledTc[node].highestPrepareQc]
      BY <2>4, Isa
         DEF LocalProposalJustification, roundView
    <2>6. proposal =
             Proposal(
               context, roundView, subject, node,
               lastInstalledTc[node],
               lastInstalledTc[node].highestPrepareQc)
      BY <2>5
         DEF proposal, LocalProposalFor, roundView
    <2>7. /\ proposal.timeoutCertificate = lastInstalledTc[node]
           /\ proposal.highestPrepareQc =
                lastInstalledTc[node].highestPrepareQc
           /\ proposal.justifyRank =
                PrepareQcRank(
                  lastInstalledTc[node].highestPrepareQc)
      BY <2>6, SMT
         DEF Proposal
    <2>8. proposal.justifySubject =
             PrepareQcSubject(
               lastInstalledTc[node].highestPrepareQc)
      BY <2>4, <2>6, SMT
         DEF Proposal
    <2>9. /\ proposal.timeoutCertificate = selected.tc
           /\ proposal.highestPrepareQc =
                selected.tc.highestPrepareQc
           /\ proposal.justifyRank =
                PrepareQcRank(selected.tc.highestPrepareQc)
           /\ proposal.justifySubject =
                PrepareQcSubject(selected.tc.highestPrepareQc)
      BY <2>3, <2>7, <2>8
    <2>10. lockRank[node] # NoRank
             => \/ proposal.subject = lockSubject[node]
                \/ /\ selected.tc.highestPrepareQc # NoPrepareQC
                   /\ selected.tc.highestPrepareQc.view
                        > lockRank[node]
                   /\ selected.tc.highestPrepareQc.subject
                        = proposal.subject
      BY <2>1, <2>9, Isa
         DEF ProposalWireValidFor, SafeToPrepare
    <2> QED BY <2>3, <2>9, <2>10
         DEF selected, current, proposal
  <1> QED BY <1>1

=============================================================================
