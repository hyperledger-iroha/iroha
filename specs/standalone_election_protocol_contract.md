# Standalone private election protocol contract

Status: unresolved construction, first-release blocker F11. This specifies the
construction's required behavior, not an admitted circuit, protocol proof or key
registry entry. The [release goals](first_release_completion_goals.md) and
[audit matrix](zk_audit_matrix.md#election-statement-completion) remain authoritative.
The [candidate review](../docs/history/2026-09-21/standalone-election-candidates.md)
records why the constructions examined so far do not establish completion. A
[later rejection trace](../docs/history/2026-09-23/election-fresh-mask-correction-rejection.md)
shows that publishing a fresh-mask correction for an accepted conviction update
reveals its hidden choice even when the final aggregate is unchanged.
The [late-dropout aggregate-opening review](../docs/history/2026-09-23/standalone-election-dropout-aggregate-blocker.md)
states the remaining recovery interface and its scoped disclosure tests.
The [fault-matrix source review](../docs/history/2026-09-24/standalone-election-dropout-fault-matrix.md)
maps those obligations to the present closed instruction and state shapes.
The [primary-source functional-opening review](../docs/history/2026-09-24/standalone-election-primary-source-functional-opening-review.md)
compares additional authority-free aggregation papers with this fault and
disclosure boundary without selecting a production construction.
The [programmed-opening screen](../docs/history/2026-09-25/standalone-election-programmed-opening-screen.md)
checks later functional-encryption candidates against exact closed-corpus and
late-dropout requirements. It identifies no qualifying construction; this is a
bounded review of the cited candidates, not an impossibility proof.

The retained Core election state has one bounded, ordered sequence of fixed
`(nullifier, commitment)` ballot pairs. This preserves the association and
admission order of public operation bytes through snapshot and restart; it does
not authenticate a credential or confidential bond, constrain a hidden choice
through updates, or prove a tally over the closed latest-state corpus. The
existing standalone ballot and tally admission guards stay closed. The
committee-free late-dropout construction and its review remain unresolved.

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

### Candidate fault model to instantiate

A candidate must give concrete values and units for: maximum eligible
credentials `N`; maximum accepted cast/update history `A`; maximum updates per
credential `U`; tolerated voters that stop sending after any message `d >= 1`;
actively corrupted credentials `c`; any required public-worker availability;
network delay/synchrony bounds; and a finite close-to-result deadline. State
whether corruptions are adaptive, what happens to an honestly cast voter's
secret after later corruption, and whether a corrupt participant may withhold
its own recovery messages. Validator finality/availability is the existing
chain assumption and must not silently substitute for voter or worker
availability. Relayers can fail, censor until the applicable ledger deadline,
retry, or equivocate; they never supply ballot authority.

For **every** finalized history within those bounds, quantify over dropout
sets and schedules chosen after the adversary sees preceding public messages.
An accepted voter can be the next dropout, even if it was the last active voter
or its latest accepted operation was an update. Setup participants that never
cast must have no tally contribution. The candidate must show that an honest
public submitter can finish from its stated durable inputs and permitted
messages within the deadline. If any required worker has private state, identify
it and prove the stated availability/corruption bound without making that
worker or a coalition a decryptor. A missing message outside the declared
bound may prevent completion, but it must never authorize a partial tally,
early opening, changed corpus, or reconstruction of voter secrets.

The safety claim is independent of completion: no valid result may differ from
the exact latest-state tally of the one finalized corpus, even under arbitrary
invalid messages, retries, crashes and corruptions. Privacy must cover the
complete public ledger, proofs, worker and relayer traffic, errors and timing
visible under the stated synchrony model. Its allowed leakage must be specified
before circuits: the final totals, declared public context and unavoidable
traffic metadata, plus the corrupted parties' own inputs. The candidate must
test equal-leakage worlds with adaptively scheduled dropouts and updates,
including a one-update batch. A simulator or reduction must explain why an
observer cannot obtain a plaintext ballot or a second, proper-subset total.

## Required phase and state interface

These are semantic checkpoints. A candidate may require additional bounded
messages, but must define their deadlines, authenticated transitions and abort
behavior before final wire types or circuit identifiers are introduced.

1. Freeze eligibility and policy. Bind network, election, eligibility root,
   asset identity/incarnation, smallest-unit scale, an option set with 2..=64
   choices, exact conviction
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

### Acceptance and adversarial matrix

The implementation and independent review must exercise the following cells
at minimum. “Accepted” means a finalized state transition with every recovery
artifact required by the candidate; an accepted transport response is not a
vote. Each cell must be repeated at its maximum declared `N`, `A`, `U`, `K`,
dropout count and message size where that dimension applies.

| Transition or fault | Required result | Rejected observation or state |
| --- | --- | --- |
| Eligible participant posts setup material, then never casts | No contribution or blocking dependency; closure uses only accepted history | Phantom weight, selected-survivor rerun, demand for its secret |
| Cast is finalized, voter disappears before its next protocol message | Finish includes this accepted choice and weight | Omission, plaintext opening, partial/subset result |
| Update is finalized, voter disappears immediately | Latest weight replaces exactly the old weight; hidden choice is unchanged | Old-plus-new counting, old-only counting, public mask-difference or singleton-batch choice test |
| Several voters disappear at adversarially chosen message boundaries up to `d` | Complete unique result by the declared deadline under the stated worker/network assumptions | Recovery that quietly changes the corpus or solicits voter secrets |
| Corrupt credentials send malformed or conflicting messages, or withhold after acceptance, within `c` | Invalid messages cannot alter a valid result; accepted corrupt ballots still count once | Invalid proof accepted, equivocated final result, corrupt-voter removal |
| Relayer retries after lost reply or submits two concurrent updates | Consensus order gives one accepted predecessor chain; identical retry has one effect | Second spend, stale-weight replacement, choice change, transport-level authority |
| Close, finish or storage crashes and restarts at each durable transition | Recovered result binds the same finality certificate, context, ordered history, latest-state map and totals | Prefix/counterfactual opening, second close, changed result, erased proof or bond evidence |
| Faults exceed a declared completion bound | Preserve finalized accepted history and fail closed until a qualified completion path exists | Partial tally, accepted-ballot deletion, master-key or committee fallback |

Passing shape checks, public PLAIN conviction tests, fixture proofs, or a short
validity proof does not satisfy any cryptographic cell. The candidate must
provide its transcript, soundness/completeness and privacy arguments together
with resource measurements before the production semantic guard is removed.

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
