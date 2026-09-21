# Standalone private election protocol contract

Status: unresolved construction, first-release blocker F11. This specifies the
construction's required behavior, not an admitted circuit, protocol proof or key
registry entry. The [release goals](first_release_completion_goals.md) and
[audit matrix](zk_audit_matrix.md#election-statement-completion) remain authoritative.
The [candidate review](../docs/history/2026-09-21/standalone-election-candidates.md)
records why the constructions examined so far do not establish completion.

## Fault and disclosure boundary

The chain supplies one authenticated finalized order and durable availability
under its existing validator fault assumptions. Voter availability is a separate
fault domain. A proposed construction must state a nonzero tolerated dropout
bound, its corruption bound, synchrony assumptions and required public workers.
It must handle a participant disappearing after each message, including after
an accepted ballot or accepted conviction update, without later voter secrets.
Publishing setup material and then failing to cast is also an explicit case.
Crashes, retries and relayer failure cannot replace these cryptographic obligations.

Only the final totals and explicitly specified public context/traffic metadata
may leak. The proof must compare executions having equal final totals, equal
allowed metadata and equal corrupted-party inputs. It must account for what
totals imply when few honest choices remain. Anonymizing a published individual
ballot is insufficient: individual plaintexts and additional subset totals are
outside the allowed disclosure. Active scheduling and update transcripts must be
included in the adversary's view. No early-tally guarantee is inferred merely
from a phase name or ledger deadline.

There is no designated decryptor, decryption committee or master decryption key.
Do not reconstruct a voter's secret, remove a previously accepted ballot after
dropout, or rerun a smaller election as recovery. Software custody elsewhere in
the product does not authorize any exception to this election contract.

## Required phase and state interface

These are semantic checkpoints. A candidate may require additional bounded
messages, but must define their deadlines, authenticated transitions and abort
behavior before final wire types or circuit identifiers are introduced.

1. Freeze eligibility and policy. Bind network, election, eligibility root,
   asset identity/incarnation, smallest-unit scale, option set, exact conviction
   rule, arithmetic bounds, phase deadlines and protocol parameters. Enrollment
   proves the anonymous credential relation; the relayer only carries messages
   and pays fees. Account-to-ballot or account-to-bond publication is insufficient.
2. Accept an ordinary ballot or conviction update. Credential authorization,
   election nullifier and the confidential bond position must be one proof
   relation. Prove asset conservation, ownership, lock expiry and single spending.
   Ordinary choices cannot change. Updates preserve the hidden choice, cannot
   decrease the bond or lock end, and replace only the prior accepted weight.
   Acceptance is durable consensus state, not a transport acknowledgement.
3. Close one corpus. Commit the exact ordered accepted history and its latest
   accepted state per credential, including updates. Bind count, root, policy,
   eligibility and phase. The construction must derive this corpus from actual
   finalized execution, not a tally submitter's selected subset.
4. Complete the tally within the declared fault bound. Include every accepted
   ballot's latest weight even if its voter is now absent. Prove the complete
   relation to the closed corpus without publishing additional subset results.
   A public statement containing asserted totals is not that relation.
5. Persist one result. Restarts authenticate the original closed corpus and
   recover its completion. Duplicate results are idempotent only when identical;
   changed contexts, corpora or totals are rejected. Bond release cannot erase
   the evidence defining the result or permit a second spend.

## Required rejection traces and resource evidence

- Missing caster after publishing setup; accepted caster lost before the next
  phase; repeated dropouts up to the declared bound, including recovery rounds.
- A relayer substitutes a credential, network, eligibility root, policy, bond,
  nullifier, phase message or finalized corpus. Valid proofs from another
  context must not authorize the altered input.
- Concurrent updates, duplicate submission after a lost reply, changed choice,
  decreased bond/lock, stale previous weight, reused bond and restart at every
  durable transition. Count the latest accepted update exactly once.
- Adversarially selected survivor sets, retries with different closed corpora,
  and pre-/post-update transcript comparisons that reveal individual choices
  or a second subset sum despite an unchanged allowed final disclosure.
- Minimum/maximum amounts in smallest units, frozen asset scale, intermediate
  multiplication/square-root bounds and maximum aggregate weights. No field
  wraparound, display-unit conversion or silent saturation may alter the rule.
- Maximum eligibility, options, updates and dropout rounds, including actual
  proving/verifying memory, storage, I/O, work, proof size and deadline cost.
  Public tally extraction (including any bounded discrete logarithm) needs its
  own measured worst-case bound; short validity proofs do not supply that bound.

TODO: supply a concrete construction, exact security reductions and leakage
definition, independently review them, then implement the semantic circuits,
typed messages and authoritative state transitions. None of those obligations
is discharged by this contract or the literature comparison. Existing closed
production admission remains closed; no compatibility fixture is a substitute.
