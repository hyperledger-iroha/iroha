# KAGEMUSHA single design — proposal, revision 5

Status: **proposal for owner decision, 2026-10-02. Not accepted. It authorizes
no deletion and no production release.** Nothing in §3–§12 is implemented.
Sentences that say "exists", "existing", "today", "current" or "the repo"
describe present code.

Process note. Every repository and web fact in this document was gathered by
automated passes run by the author. None has been confirmed by a human expert,
a build, a test or a device. Revision 5 was drafted section by section by eight
automated passes, each of which first checked the findings of two external
critiques of revision 4 against the text, the repository and primary sources.
Sources are in
[`kagemusha_single_design_evidence.md`](kagemusha_single_design_evidence.md).
Its section 0 maps the factual claims of this revision to their sources and
says which rest on a single pass; its sections 1 to 6 are older research kept
as history, and its section 7 lists the sources this revision added. Sizes and
times are estimates or arithmetic unless a line says measured.

History:

- Revision 1 recommended signature-only payments and deletion of the
  recursive-proof code. An external review rejected that as overstated.
- Revision 2 withdrew the deletion and corrected six arguments. Revision 3
  corrected errors in those corrections; two automated checks of it found a
  working attack in the recovery rules and 37 further defects.
- Revision 4 added the owner's inputs of 2026-10-02: the counter or commitment
  mechanism, the timing wish, optional fees, user authentication, and the
  June 2025 TC3 document. One automated check found 67 defects, two serious.
- Two external critiques of revision 4 found that the loss rule paid early
  unloaders and left the deficit with holders who stayed offline; that an
  interrupted marker deletion left a way to restore an old balance; that a
  wallet destroyed its own key on a failed read; that daily limits could be
  spent several times by setting the date back; that the position on Decision
  B was asserted and not derived; and that nothing said what becomes of each
  existing track.
- The owner then stated: "offline payments must be secure and final. once you
  transfer from one phone to another, it must be a final transfer of value
  there, with no need to ever go online again unless there are regulatory
  controls".
- Revision 5 rewrites every section from §1 to §9, and §11 and §14, around
  that statement and the confirmed findings. Almost every critique finding
  was confirmed; none was refuted. One automated consistency check of the
  assembled text found 150 defects, 14 of them serious, mostly places where
  sections written in parallel disagreed. They are fixed. The result has not
  been checked, and more disagreements of the same kind should be expected.

## 0. Summary for the owner

**What this document takes the owner's statement to settle.** It reads the
statement on finality as five rules, F1 to F5 (§1). They are its reading, not
the owner's words, and they need confirmation (§12 item 32).

- A received payment is final value. Nothing that happens later reduces it,
  and it is redeemable at face value with no time limit.
- There is one loss rule: the operator underwrites (§8.4). The option under
  which holders shared a shortfall is removed. It paid early unloaders in full
  and left the deficit with whoever stayed offline.
- Only a regulatory control that is switched on sends a holder online: the
  block list, limits, expiry.
- A wallet never destroys its own key or balance on a condition that may pass.

**Terms used in this summary.** A scheme is one asset's offline arrangement:
one pool of value on the ledger, one set of keys and one table of tiers. A
tier is a set of terms that a phone's certificate may carry. The operator is
the party that stands behind the scheme and underwrites its losses. The issuer
is the scheme's online service; it certifies phones. To sync is for a phone to
contact the issuer. To load is to move value from the ledger into the wallet,
and to unload is to move it back. The lease is how long a certificate lets a
phone send before it must sync. The backstop is the operator's own cash in the
pool. Layer A is the whole design apart from the proof that Decision B is
about (§2).

**What that costs.** A compromised phone can create value that honest receivers
accept. No stock phone was found that can prevent it (§4.1). Final value with
no need to go online means the operator honours that value. With the
regulatory controls off, nothing bounds it and nothing reveals it (§3).

**What the design is.** One wallet core, one payment format and one value pool
per scheme (§4 to §10): device keys held in phone hardware and certified by an
issuer; a signed journal on each phone; a marker kept outside every backup, so
that an ordinary user cannot restore an older balance (§5.10; this rests on
device tests that have not been run); optional limits,
expiry and block list; a registry and a reserve on the ledger. Whether a
payment also carries a proof is Decision B (§2), which is the owner's. §2 shows
what a proof buys case by case.

**Where the statement cannot be fully met.** §3.1 lists every rule that can
still send a holder online, or put a balance out of reach, without a regulatory
control. The main ones:

- A wallet whose marker is gone while its key lives stops for good. Removing
  the passcode on an iPhone does this; so does restoring older wallet files.
  Letting such a wallet resume would let an ordinary user reset a balance. A
  wallet that has lost only its journal can be repaired online, and only if it
  signed nothing since its last sync (§7.2).
- A lost, reset or dead phone loses its balance. Recovery insurance is off by
  default.
- When an issuer key is compromised, the rule in §5.1 stops every phone under
  that key until it syncs. §3.1 recommends letting certificates stand until
  their own expiry.
- A payment that is signed but does not reach the receiver, or a refusal that
  does not reach the payer, leaves the amount in neither wallet until the two
  phones meet again.
- A phone whose own key signed two conflicting records is held, and its
  balance waits for a governed reinstatement. A platform fault can do this to
  an honest phone.
- If payments carry proofs, received value cannot be spent again until the
  receiving phone has proven it.

**Decide first.**

1. Confirm F1 to F5, and what counts as a regulatory control (§12 items 32 and
   33).
2. The default tier: limits and expiry on or off, and the lease length (item
   45). With both off the operator's exposure has no bound and cannot be seen.
3. What stands behind the operator's promise, and whether the backstop may be
   funded by issuing new money (item 41).
4. Decision B, using the tables in §2 (item 1), and whether one to two seconds
   is a gate for proof-carrying payments too (item 55).
5. Key revocation (item 34) and stopped wallets (item 35).
6. One design: whether Layer A replaces the existing signature-only suite in
   place, and whether acceptance withdraws the existing statements that make
   stock-phone keys online-only (item 56).

## 1. Requirements and the security goal

| | Requirement |
|---|---|
| R1 | Offline protocol. No network call authorizes a payment. |
| R2 | Any modern mainstream phone with hardware-backed key storage (Pixel 6, Samsung, Huawei, Meizu, iPhone). No custom secure-element applet. |
| R3 | Load value from the online ledger into the offline wallet. |
| R4 | Direct device-to-device transfer, instant, unbounded hops. Once value moves from one phone to another it is "a final transfer of value there". |
| R5 | Value may move back online. Nobody has to move it: there is "no need to ever go online again unless there are regulatory controls". |
| R6 | A blacklist of accounts; senders holding it will not pay them. |
| R7 | Optional daily and monthly limits. |
| R8 | Optional attestation expiry: sync before sending again. |
| R9 | A payment message stays within about 10 KB. |

The table is this document's summary of what the owner asked for. Words in
quotation marks are the owner's.

Owner's statement, 2026-10-02, verbatim:

> offline payments must be secure and final. once you transfer from one phone
> to another, it must be a final transfer of value there, with no need to ever
> go online again unless there are regulatory controls

R4 and R5 are restated from it. The owner first stated R4 as "the offline
value transfer itself is final and settled instantly", with "unbounded hops,
except when there are regulatory rules like blacklisted accounts or
daily/monthly spend limits, etc." R2 as first stated named a phone that "is
modern and mainstream and has a secure element". Hardware-backed key storage
and the exclusion of a custom applet are this document's reading of the
clarification below. This document reads "settled" as ruling F1 below: the
receiver holds final value at that moment.

This document reads the statement as five rulings. They are the document's
reading, not the owner's words. §12 asks the owner to confirm them.

- F1. **Final at commit.** A received payment is final value at the moment the
  receiver's wallet commits it. Nothing that happens later reduces it: no sync
  by anyone, no evidence against the payer, no block of the payer, no key
  revocation, no policy change. It is redeemable at face value whenever it is
  presented, with no time limit.
- F2. **One loss rule.** The operator underwrites. Any shortfall caused by
  counterfeit value is the operator's. A rate limit may delay a payout. It
  never reduces or cancels one. Other holders' behaviour must not delay an
  honest holder's payout without bound.
- F3. **Only a regulatory control sends a holder online.** The regulatory
  controls are R6, R7 and R8, each only where the scheme has switched it on.
  R6 includes receive freshness (`receive_not_after`, §5.5). R8 includes the
  reboot policy (§5.4). Any other rule that suspends paying or receiving until
  a sync, or that destroys or strands a balance, is removed, redesigned, or
  listed in §3.1 as an exception with the reason it cannot be avoided and the
  cost of avoiding it. The owner said "regulatory controls" and did not list
  them. Taking them to be R6, R7 and R8 is this document's reading.
- F4. **"Secure" is stated per attacker, and never beyond the evidence.** An
  honest user's committed value is not reduced by anyone else's action. A
  payment in flight can still be lost, and others can stop the wallet paying
  or receiving in the cases §4 lists. An ordinary user
  with the unmodified app cannot reset the wallet or double spend; this rests
  on the marker of §5.10 and on device tests that have not been run. A
  compromised phone can create value that honest receivers accept and the
  operator must honour, and only the switched-on regulatory controls bound
  that. §4 gives each statement with its scope. No section may imply more.
- F5. **No destruction on a transient condition.** A wallet never destroys its
  own key or balance on a condition that may pass: locked storage, a failed
  read, an error code. A destructive path needs an established, unrecoverable
  loss. The rule exists because a locked key store and a missing wallet look
  alike to an app that only tests whether a read succeeded.

R8 uses the owner's word "attestation". In this design the thing that expires
is the device certificate the issuer signs. A renewal issues a new certificate.
It does not repeat the platform attestation (§4, T2).

The owner stated the security model as: "our security model requires us to
have a hardware backed key that is used to attest that our signing key/state is
valid and the tx is from a valid app".

Owner clarifications, 2026-10-02:

- The design does not assume that wallet logic runs inside secure hardware; the
  project has no OEM access. The owner's mechanism is "hardware backed keys
  and some unique counter/commitment to prevent reset and double spend". §4.1
  states what stock phones offer for that: reset by an ordinary user can be
  prevented if the device tests pass; no way was found to prevent double
  spending by a compromised phone.
- Smart cards are a future option to explore. Nothing here is designed for
  them, and nothing should foreclose them.
- An earlier document, the June 2025 "TC3: Offline Capabilities" response,
  holds many of the ideas and requirements for offline. It is old and not fully
  current. §14 compares it with this proposal. This document assumes that
  R1–R9 win where the two disagree. The owner did not say so; §12 item 27 asks
  for confirmation.
- "1-2 s should be good ux" for a payment. This document takes one to two
  seconds as the target for "instant" in R4. The owner stated no percentile,
  no end points and no gate. Measuring it from the payer confirming to the
  receiver seeing the credit, and the 2 s p95 figure, are this document's
  reading (§2, §2.3).
- Fees are optional. A fee is received only into an online account, when
  someone syncs (§5.8).
- PIN or biometric is a user-experience function, and "generally it should be
  related to the secure hardware". §5.9 proposes one way to do that.

Stock phones fall short of the stated security goal in three ways. The rest of
this document uses the weaker statements in §4.

- No platform attests wallet state. Attestation covers a key and an app
  identity at one moment.
- On iPhone the payment key is not itself attested.
- No payment carries platform evidence that the genuine app produced it. On
  Android the app identity is asserted by the OS once, at key generation. On
  iPhone the payment signature carries no app identity.

## 2. The decision this proposal asks for

The design has two layers.

**Layer A — attested device ledger.** Issuer-certified device keys, a signed
hash-chained journal per phone, limits, expiry, block list, on-chain registry,
load and unload accounting, backstop. §4 to §10 specify it. Recursive V1 has no
offline limits and no block list, and its stock-phone key path is not admitted
in production (§2.2).

**Decision B — does a payment also carry a proof?** Under B1 every payment
carries a recursive proof about how the payer's state was reached. Under B2 a
payment carries signatures only. §2.1 defines the two statements such a proof
can make: the lean relation and the per-hop relation. A proof of the per-hop
relation shows that every transition behind a balance was valid and was signed
by a certified, registered device key. Neither relation shows that the total
supply is conserved, and this document does not use that word for either.

B1 has two shapes. They differ in when the proof is made.

- **Proof on the payment path.** The payer proves the SendSplit before it
  releases it. The repo's gates assume this shape. §9 calls it "proof before
  the commit", and calls the other shape "proving off the payment path".
- **Proof made beforehand.** The payment carries a proof of the payer's state
  before this payment, made in the background after the wallet's previous
  transition, together with the signed SendSplit. The receiver checks the
  SendSplit against the proven state by its own native check. This is the only
  shape that could meet the timing reading below. It is a sketch (§2.3,
  "Proving off the payment path"). No relation, circuit or code exists for it.

| | B1, proof on the payment path | B1, proof made beforehand | B2, signature only |
|---|---|---|---|
| The payment carries | Layer A objects and a proof of the payer's state after this SendSplit | Layer A objects, the signed SendSplit, a proof of the payer's state before it, and the payer's balance or a range proof | Layer A objects only |
| The receiver verifies before it commits | Layer A checks and the proof | Layer A checks, the proof, and the SendSplit against the proven state by its own native check | Layer A checks only |
| What a proof covers when the receiver commits | Every transition up to and including this payment | Every transition before this payment. This payment is covered when the receiver's own next proof absorbs it | Nothing |
| Work while the two people wait | The payer proves (repo gate 10 s p95), the payment crosses, the receiver verifies (repo gate 1 s p95) | The payment crosses and the receiver verifies (repo gate 1 s p95). A range proof, if used, is also made now | Signatures, and the payment crosses |
| Work afterwards, on each phone alone | The receiver proves its fold before it can spend the value offline | The payer proves its new state before it can pay again. The receiver proves its fold before it can spend the value offline | None |
| When the payment is final (F1) | At the receiver's commit | At the receiver's commit | At the receiver's commit |
| Arithmetic bug in an honest wallet | The wallet cannot prove the transition and releases nothing | One payment reaches a receiver, who cannot prove its fold | Accepted by every receiver. No peer-visible object carries a balance (§5.7) |
| Value with no load behind it | A compromised phone must load real value once and fork it (§2.1) | The same | A compromised phone signs it. It needs no load, and no evidence need exist (§5.3) |
| Double spend by a compromised phone | Not prevented | Not prevented | Not prevented |
| Value creation is checked by | The circuit, the mint authority it trusts, and Layer A | The same, and for the last hop the receiver's native check | Each certified phone running the genuine app |
| Counterfeit is paid for by | The operator (§8.4) | The operator (§8.4) | The operator (§8.4) |
| Payment size | Not measured. Estimate about 7.4 KB with one witness: the repo's 6,528 B proof ceiling plus about 0.9 KB of Layer A, if per-hop verification does not enlarge the transport proof. The repo's payment-message gate is 7,552 B | Not measured. The same estimate, plus the balance or a range proof | Estimate about 0.9 KB with one witness; not measured |
| On QR, with the repo's existing framing (§5.6) | About 48 frames: 4 to 10 s per pass at 12 to 5 frames per second, after the proof has been made. Computed, not measured | About 48 frames: 4 to 10 s per pass. Computed, not measured | About 7 frames: 0.6 to 1.4 s per pass at 12 to 5 frames per second. One still code only if the framing's still-code limit, about 0.34 KB of payload, is raised (§5.6). Computed, not measured |
| Status | No qualified prover. No State or payment proof has been produced (§2.2) | A sketch. Nothing is designed in detail or written | Needs no prover. None of it is written |

Proofs alone cannot prevent double-spending. That does not make the columns
equivalent. Evidence, the containment of bugs, and what a compromised phone
must do all differ. The second table below sets out those differences case by
case.

**What the owner's statement of 2026-10-02 changes.** The owner said: "offline
payments must be secure and final. once you transfer from one phone to another,
it must be a final transfer of value there, with no need to ever go online
again unless there are regulatory controls". §1 reads that as five rulings.
Three matter here. A received payment is final value when the receiver's
wallet commits it, and nothing later reduces it (F1). The operator underwrites
every shortfall that counterfeit causes (F2). Nothing forces a holder online
except a regulatory control the scheme has switched on (F3). For Decision B
that reading has four consequences.

- It does not take the decision. Paying and receiving need no network under
  B1 and under B2.
- Finality does not depend on a proof. Under every option the receiver's
  commit is the moment the value is final. No later proof, failed proof or
  evidence changes it.
- An honest holder is in the same position under B1 and B2. What a proof
  changes is what a buggy or compromised phone must do before honest receivers
  accept its value, and so what the operator later pays. Decision B is a
  decision about the operator's exposure and cost.
- It puts two conditions on B1. First, under F1 a holder must be able to
  redeem received value even if its phone never completes a proof (§2.3,
  "Redemption without a phone proof"). Second, under B1 received value can be
  spent again offline only after the receiver's phone has proven its fold. A
  phone that cannot complete that proof can use the value only by unloading,
  which is online. No regulatory control causes that, so it is an exception to
  F3. §3.1 lists it, and §2.3 sets a target for how often it may happen.

**The one to two second target.** The owner said: "1-2 s should be good ux".
This document reads that as a target of 2 s at the 95th percentile, measured
from the payer's confirmation to the receiver seeing the credit, transfer
included. The percentile, the end points, and whether the target is a gate that
a proof-carrying payment must also meet are the owner's to confirm (§2.3 Q0,
§12 item 15). Nothing in this section has been timed on a phone.

- **B2.** Inside the interval are one hardware signature on the paying phone
  and two on the receiving phone (its third, on the Request, comes before the
  payer confirms); one marker step on each phone (§5.10), which on Android is
  a Keystore key generation and a deletion; one durable commit on each phone;
  signature checks; and about 7 QR frames. Seven frames take 0.6 s per pass at
  12 frames per second and 1.4 s at 5. The repo's widget defaults to 5, and a
  scan may need a second pass. At the default rate the transfer alone uses
  1.4 s of the 2 s. That leaves 0.6 s for three hardware signatures, two
  marker steps and two durable commits. Whether B2 meets the target at that
  rate, or needs a higher frame rate or a faster carrier, is not measured.
  The budget also assumes that the receiver's phone does not ask for
  authentication again before it signs (§5.9).
- **B1, proof on the payment path.** The target rules out three things this
  shape assumes: proving while the two people wait (the repo's proving gate is
  10 s, and §9 proves a released transition before the commit); a verification
  gate of 1 s; and moving 7.4 KB over the repo's QR framing, which takes 4 to
  10 s per pass. This shape meets the target only if proving itself takes a
  fraction of a second on the floor device. No measurement was found (§9).
- **B1, proof made beforehand.** It can meet the target only if all of these
  hold:
  - the payer's phone finished the proof of its current state before the
    payment starts, which depends on how long each platform lets the wallet
    compute after its previous transition (not tested);
  - the receiver verifies the proof in a fraction of a second on the floor
    device (not measured);
  - the carrier moves about 7.4 KB in under a second. That needs NFC where the
    pair allows it (§5.6), a denser QR framing that is untested, or a smaller
    proof. One more option is untested: the proof does not depend on this
    payment, so it could cross before the payer confirms. That shortens the
    measured interval and not the time the two phones are held together;
  - the receiver learns that the proven balance covers the amount without a
    proof made at that moment, which means the payer shows its balance
    (§5.7), or a range proof is made in a fraction of a second.

  None of these is measured. A proof system with a smaller proof generally
  means a trusted setup, which is §12 item 4.

**What B1 buys, case by case.** The table uses only what §2.1, §4.1, §5.3,
§8.1 and §8.3 establish. "Operator's loss" is what the operator must honour
under §8.4.

| Case | B2 | B1, lean relation | B1, per-hop relation | Does the operator's loss differ? |
|---|---|---|---|---|
| An honest wallet with an arithmetic bug | The wallet signs transitions with a wrong balance. No receiver can tell (§5.7). The issuer's replay at a sync finds it only if the replaying core lacks the bug and the wallet syncs | The wallet cannot prove the wrong transition. Proof on the payment path: nothing is released. Proof made beforehand: one payment reaches a receiver, who cannot prove its fold; the faulty wallet cannot pay again | As lean | Yes. B2: every unit the bug creates that honest receivers accept, until a fixed build replaces the faulty one. B1: none on the payment path; at most one payment per faulty wallet with the proof made beforehand. A soundness bug in the circuit is not contained under B1 either |
| An ordinary user with the unmodified app | Cannot reset or double spend if the marker of §5.10 holds; device tests not run (§4.1) | The same. A proof does not check the marker and does not replace it | The same | No. If a restore tool defeats the marker on some phone, both payments are accepted under every option, and under every option they are two signatures at one sequence number (§5.3) |
| A compromised phone with one enrolled instance and no accomplice | Signs payments for any amount. It needs no load. Its chain is self-consistent and no evidence need exist (§5.3) | Must load real value once. It then multiplies that value through software-only intermediate states and folds the branches back. No conflicting signature by its own key; no evidence | Must load real value once. It cannot fold its own payments. To pay out more than its proven balance it must sign two successors of one state. The two receivers then hold a pair that is evidence under §5.3, if both records reach the issuer or the chain (§2.1 item 2; the argument is not checked). No holder has to sync | The bound is the same: the receiver door of §8.3 item 2. Under per-hop the block entry can come sooner, when two records meet. Evidence stops later payments only; every payment already received stays final and is the operator's loss |
| A compromised phone with a second enrolled instance, or a receiver who colludes | As the row above | As the row above | One real load and one extra enrollment. One branch is paid to the other instance, which folds it. The conflicting signature exists only as a private witness inside that instance's proof. No evidence | The bound is the same. B1 adds a cost to the attacker: one real load, and under per-hop one more enrollment that passes attestation and the registration cap (§8.1). A second profile on the same phone is enough |
| Theft of an issuer key | Certificate key with the witness quorum: unbounded offline counterfeit until root-signed revocations spread. Certificate key alone: the receipt bounds the number of new device ids. Voucher key: unbacked value minted onto genuine phones (§8.1) | Certificate and witness keys: as B2, after one real load. Voucher key: it depends on the mint authorization fixed in Q0 (§2.3). With the validator quorum seal kept in the relation, a stolen voucher key mints nothing a proof accepts. With an issuer-signed mint, it mints value every proof accepts | As lean. Keys the thief certifies are enrolled instances, so the row above applies | Only for the voucher key, and only if the mint stays quorum-sealed inside the relation. Forging a mint then needs the validator quorum's Pasta keys instead of one voucher key |

The net, in three statements.

- B1 with either relation changes the operator's loss in two of the five
  cases: an arithmetic bug in an honest wallet, and theft of the voucher key
  if the mint stays quorum-sealed.
- The per-hop relation adds one thing more: public evidence against an
  attacker with a single enrolled instance. The evidence exists only where
  two receivers' records both reach the issuer or the chain, and it stops
  later payments without undoing earlier ones.
- In every compromised-phone case the bound on counterfeit is the same under
  B1 and B2 (§8.3 item 2). Only the regulatory controls the scheme has
  switched on bound it. B1 raises the attacker's entry cost to one real load
  and, under per-hop, one extra enrollment.

**What B1 costs, and what is unknown.**

- No prover exists that can make any State or payment proof, on a host or on
  a phone (§2.2).
- The one recorded figure for the stock-phone circuit is far outside any
  phone gate (§2.2, "Scale of the gap"). It indicates scale. It is not a
  measurement.
- Received value cannot be spent again offline until the receiver's phone has
  proven its fold. With the proof made beforehand a wallet also cannot pay
  twice in a row until its new state is proven. Neither delay is measured.
- With the proof made beforehand, the receiver learns the payer's balance
  unless a range proof is added (§5.7).
- B1 has two exceptions to F3 that B2 does not have. A phone that cannot
  complete a proof can use its balance only by unloading. And a revoked key
  or a withdrawn circuit release inside a proof's history either stays
  accepted for good or forces a sync (§2.3, "Issuer keys and circuit releases
  inside a proof").
- The engineering cost is known only in part. The estimated stages sum to
  about 15 to 25 engineer-weeks, and they price a relation shaped like
  Recursive V1 with the proof on the payment path, which the timing reading
  excludes. The shape that could meet the timing is not estimated (§2.3).
- Unknown: whether any relation can be proven on the floor device within any
  target; proving time, memory, key size and energy; verification time on a
  phone; whether 7.4 KB crosses in time on each carrier; whether each platform
  lets a wallet finish a proof in the background; how often the two records
  of a fork meet when no holder has to sync.

**A third option is excluded.** The June 2025 design (§14), as read here,
carried the signed history of the value in each payment. One earlier hop costs
about 0.85 KB (certificate, receipt, signed transition; an estimate). A payment
passes 10 KB after about eleven earlier hops, or at once when the payer's
balance merges about eleven received payments, because each one brings its own
history. R4 and R9 together exclude it. B1 with the per-hop relation of §2.1
makes the same check at constant size; the lean relation does not check earlier
holders' signatures. B2 does not make it.

**Position.**

- Decision B is the owner's. This document does not take it and does not
  recommend either option. The two options are not equivalent, and the tables
  above say how they differ.
- Until the owner decides, a proof of the per-hop relation of §2.1 on every
  payment remains the requirement. The reason is procedural. The repo's
  record of the owner requirement of 2026-10-01 makes the platform and issuer
  equations and real State proofs in both fields mandatory
  (`specs/kagemusha_v1_production_readiness.md:8-25`). A proposal does not
  lapse a recorded requirement by default. The record is the repo's text and
  not a quotation of the owner. Reading it as the per-hop relation of §2.1 is
  this document's reading.
- The Pasta implementation is kept. Nothing in this section recommends
  removing it under either option (§11).
- Layer A on its own is B2. It does not carry production value before
  Decision B is taken in writing. The reason is that the two orders are not
  symmetric. A pool opened under B1 can drop the proof later: receivers stop
  requiring it as their apps update, and no balance has to be re-issued. A
  pool opened under B2 holds balances that no proof covers. Moving it to B1
  later needs one of three things. Every holder syncs once so that the issuer
  can replay its journal and re-issue its balance as a mint that proofs
  accept; that forces holders online for a reason that is not a regulatory
  control, and it certifies whatever counterfeit the pool holds at that
  moment. Or receivers keep accepting payments without a proof, and then the
  proof requirement is not in force. Or the scheme runs a second pool, which
  is no longer one design. Opening the pool first would therefore take
  Decision B by schedule.
- That decision is necessary, not sufficient, for a production release.
- The circuit-facing surface of Layer A is provisional until the relation is
  fixed (§2.3 Q0). It is the set of formats that the relation decides. §2.3
  lists it, and says which parts of Layer A can be built before then without
  rework.

One design still holds under either option: one payment format, one value pool
per scheme, one wallet core.

### 2.1 What a proof establishes depends on the relation

A relation is the statement a proof proves about the payer's state. Two are
candidates. A third needs hardware that no target phone is shown to have.

1. **Lean relation**: state arithmetic and mint authorization proven; the
   device signature checked natively on the last hop only. This contains
   arithmetic bugs and makes creating value require one real load. It gives no
   attribution: earlier holders are unauthenticated, so a compromised phone can
   fork that balance through software-only intermediate states and fold both
   branches back under a valid proof, with no conflicting signature by its own
   key.
2. **Per-hop relation**: each transition's device signature, issuer
   certificate and registration receipt verified in-circuit. Every unit then
   traces to a load through transitions signed by issuer-certified, registered
   device keys (hardware-held on Android by attestation; on iPhone subject to
   §4). A fork now needs two signatures by one key over two successors of one
   state.
   - The relation must constrain the sequence number, the previous-digest
     link and the cumulative counters of each transition, as it constrains
     the balance. This rule is what makes a hidden transition useless as
     padding. Without it a compromised phone chooses its counters, as it does
     under B2, and no pair of its transitions need be evidence (§5.3).
   - The relation must reject a ReceiveFold whose credit was signed by the
     folding device's own key. Otherwise a single compromised instance pays
     itself on a hidden branch and folds that credit into its visible chain.
   - With both rules, a phone with one enrolled instance and no accomplice
     cannot pay honest receivers more than its proven balance without leaving
     evidence in their hands. Take any two of its payments that come from
     different branches. At one sequence number they are two digests. At
     adjacent numbers the link is broken. Further apart, the later one fails
     the cumulative-outflow rule of §5.3, because a branch can raise its
     proven `cum_out_after` only by a proven outflow, and that outflow debits
     the same branch. Padding with outflows therefore costs the branch as much
     as it hides. This argument is this document's own. It has not been
     checked by a second pass or written as a test.
   - The pair is evidence only when both transitions reach the issuer or the
     chain. Each sits in one receiver's journal, and no holder has to sync.
     So the evidence may never appear.
   - Evidence never reduces a payment already received (F1). It leads to a
     block entry, which stops later payments among receivers that hold the
     entry (§12 item 11).
   - None of this holds if one branch is paid to any other enrolled instance
     the attacker controls, or to a receiver who colludes. A second profile on
     the same phone is enough. The conflicting signature then exists only as
     a private witness inside the next proof. Supply is therefore inflatable
     without evidence for the price of one real load and one extra enrollment.
   - What this relation adds over the lean one is evidence against an
     attacker with a single enrolled instance, under the conditions above, and
     the requirement that every intermediate holder is an enrolled key.
     Per-transition validity and provenance hold; aggregate supply is not
     conserved.
3. **Per-hop relation plus hardware uniqueness**: prevention only if every
   device in the pool has the primitive and each hop proves its use, and only
   for as long as the primitive holds. One enrolled device without it gives the
   whole pool item 2's inflation. No target phone is shown to have it (§4.1):
   the Pixel 6 StrongBox key's use limit is software-enforced, its TEE key and
   other vendors' StrongBox keys are untested, and the iPhone assertion counter
   appears to be set by the OS, not by the enclave. No published design
   combining per-hop proofs with a stock-phone uniqueness primitive was found;
   the precedents found run custom code in a secure element.

Neither the lean nor the per-hop relation proves any of the following. That a
state has only one successor. What time it was when a transition was signed.
Which block list the payer held. That the wallet used its marker (§5.10). A
proof can check the arithmetic of the day and month counters at every hop; it
cannot check the time those counters are indexed by. Expiry, limits and the
block list are therefore enforced as under B2: by the payer's own app, and by
the receiver on the last hop.

Finality does not depend on a proof. The receiver's commit makes a payment
final under every option (F1). If the receiver's later proof of its fold fails
or is never made, the value is still the receiver's and still redeemable. It
cannot be spent again offline until the fold is proven.

Under B1, a refund must not be authorized by a signature alone. Otherwise a
compromised receiver folds a payment and also signs a refusal, the payer
refunds, and supply rises with one consistent chain on each phone. So the
refusal is a proven transition of the receiver's state, and the payer's refund
verifies it. The alternative is that refunds do not exist offline under B1. A
refused payment's amount would then be in neither wallet until a sync, which
§3.1 does not allow. With a proven refusal, a compromised receiver that folds
and also refuses has signed two successors of one state. That is the fork of
item 2, no better and no worse.

### 2.2 State of the Pasta implementation

As read from source and repo records on 2026-10-02. Not confirmed by build,
test or device.

- **Node verifier.** Code that verifies paired-Pasta proofs and decides
  accumulators exists and is called from block execution
  (`crates/iroha_core/src/smartcontracts/isi/kagemusha.rs:2617-2633`). With no
  release keys the node keeps a reject-all verifier, and it has never accepted
  a genuine State proof because none exists
  (`specs/kagemusha_v1_production_readiness.md:156-160, 297-299`).
- **Provers.** No shipped build can produce a Bootstrap, MintFold, SendSplit,
  ReceiveFold or RedeemSplit State proof, and the production prover has no
  Rotate entry point. The witness source the provers require has only a failing
  test stub
  (`crates/iroha_core_zk/src/kagemusha_v1_recursion/native_outgoing_witness.rs:57,
  167-178`), and no release keys exist.
- **Closest thing to an end-to-end proof.** A lineage generator for the
  secure-hardware relation exists as ignored unit tests: two devices, a mint, a
  send, a receive and release
  (`.../real_payment_corridor/state_milestone.rs:2879-2974`). It has no recorded
  completed run. RedeemSplit stops after terminal authorization, and
  verification there uses a test-local verifier.
- **Stock-phone keys.** A separate "ordinary" Guard circuit verifies a
  full-width P-256 device signature and a P-256 issuer signature at k = 16 in
  both fields, under a mock prover
  (`.../ordinary_guard_composition.rs:178-181`,
  `.../ordinary_guard_composition_tests.rs:262-323`). The State relation
  contains its consumer, but the production construction refuses it
  (`.../composite.rs:1961-1968`), ordinary ReceiveFold is refused in every
  construction (`.../composite.rs:1003-1008`), and the only proof test is
  ignored. App Attest and KeyMint classes are rejected at the three monetary
  folds. The repo records key generation for this path as blocked at 8,584
  advice columns against a 1,024 ceiling (`status.md:168-171` at commit
  `b2a3cd05bc`; the file is being edited in the working tree and the lines
  have moved).
- **Scale of the gap.** The record reads: "Ordinary recursive credential
  generation is blocked by the Eq circuit requiring 8,584 advice columns
  against the 1,024-column limit."
  - What the ceiling is. It is a guard in the key generator on the build
    host. It refuses a circuit before synthesis, so that a width regression
    fails before memory is allocated
    (`crates/iroha_core_zk/src/kagemusha_v1_recursion/artifact_resource_preflight.rs:34-39,
    485-490`). It is not the phone gate. The comment beside it says that 1,024
    columns at k = 16 already need at least 4 GiB for two field buffers, and
    that an earlier circuit with 6,738 columns could enter a process above
    160 GiB.
  - The conversion. One advice column at k = 16 is 2^16 rows of 32 bytes,
    which is 2 MiB for one evaluation bank. The repo uses this arithmetic for
    the Claim circuit: 96 columns, 192 MiB
    (`.../artifact_resource_preflight.rs:1470-1472`).
  - The arithmetic. At k = 16, 8,584 columns is 17,168 MiB, about 16.8 GiB,
    for one advice bank. That is about 134 times the repo's 128 MiB phone
    gate and 8.4 times the ceiling. The ceiling itself is 2 GiB for one bank,
    16 times the phone gate, so a circuit can pass the ceiling and still miss
    the phone gate. A process holds more than one bank.
  - For comparison, the widest graphs the readiness record describes have
    264 and 234 advice columns in total, and neither produced a State proof
    (`specs/kagemusha_v1_production_readiness.md:756-765, 781-789`). 8,584 is
    32 to 37 times those.
  - What is not known. The repo does not say which circuit of the ordinary
    path the figure belongs to, or which part of it produces the width. One
    automated estimate is that two P-256 checks do not account for it and that
    it comes from the carrier or SHA-256 layout; no build confirmed that. The
    record gives no k; k = 16 is what the ordinary Guard builder uses. It is
    not known whether the width is a layout defect that can be removed, like
    the 6,738-column regression, or the size of the relation. The ordinary
    path is also bound to the online profile's objects, and Q1 re-specifies
    it (§2.3).
  - What follows. The figure sizes a refused build of unknown composition.
    It shows that one circuit on the stock-phone path, as built, is two
    orders of magnitude wider than the phone gate allows, by the repo's own
    conversion. It is not a measured memory requirement of the per-hop
    relation, and no such measurement exists.
- **P-256 cost.** Unmeasured. The reduced-window tests run at k = 18 as a test
  choice; the full-width equation is built at k = 16 by widening columns. No
  cell count, key size, memory or time is recorded.
- **Runs.** No credential, Guard, mint, State, terminal or wrapper proof is
  recorded as completed on a host; only small component proofs are. Three
  diagnostics ended at a 2,700-second guard with 1.43, 1.47 and 6.65 GB peak
  memory, and two earlier runs exited in key generation near 1.9 GB. All five
  stopped before any State proof. They were non-release builds on unstated
  hardware, so none is a proving-time measurement
  (`specs/kagemusha_v1_production_readiness.md:756-765, 781-789, 859-879,
  913-919`).
- **Memory.** The Claim circuit's minimum configuration is 96 advice columns at
  k = 16: a 192 MiB bank and a 236 MiB lower bound against the repo's 128 MiB
  gate. These are source-level bounds. The current unsplit Claim circuit
  therefore misses that gate by construction. The same spec sketches a
  five-slice k = 15 split (79 MiB for one evaluation bank, eighteen proof
  instances per transition). Its slice arithmetic exists as test-only code; the
  subclaims and joins are unimplemented, and memory is unmeasured
  (`specs/kagemusha_v1_phone_algorithm.md:737-743, 768-771, 811-813, 833-834`).
- **Hardware uniqueness.** On a Pixel 6 the use limit of a StrongBox key is
  attested as software-enforced, both feature flags are false, and the embedded
  secure element denies the app a channel
  (`specs/kagemusha_v1_production_readiness.md:342-379`,
  `specs/kagemusha_v1_phone_algorithm.md:515-519`). The raw chain is held
  outside the repository. That measurement covers StrongBox on a Pixel 6 only.
  The probe for an ordinary TEE key exits on the feature flag before it
  generates a key
  (`kotlin/client-android/src/main/java/org/hyperledger/iroha/sdk/offline/probe/AndroidKeyMintSingleUseProbeV1.kt:79-81`),
  so a one-use TEE key has never been generated or attested. §4.1 has the rest.
- **The counter in code.** The phone spec names "an attested strict-next
  counter or one-use-key ratchet" as the ordinary-app candidate and requires
  `after = before + 1` (`specs/kagemusha_v1_phone_algorithm.md:10-24,
  234-243`). The code accepts any App Attest counter above a retained floor:
  the data model
  (`crates/iroha_data_model/src/kagemusha/kagemusha_v1/hardware_selection.rs:489`),
  a circuit helper that only tests build
  (`crates/iroha_core_zk/src/kagemusha_v1_recursion/composite.rs:1605, 1652-1656`)
  and the Swift client
  (`IrohaSwift/Sources/IrohaSwift/KagemushaAppAttestEvidenceV1.swift:342-347`).
  With gaps allowed, counters c+1 and c+2 are both valid successors of one
  state, so the counter stops neither a fork nor a rollback.
- **Phones.** No on-device harness runs this prover. The mobile bridge enables
  the prover feature unconditionally
  (`crates/connect_norito_bridge/Cargo.toml:42, 49`).

### 2.3 Qualification plan

The plan has to end in a decision. So it fixes four things before any
measurement: the targets, a budget and a date for each stage, a limit on how
often the relation may be reconsidered, and what each outcome leads to. Each
stage has a named owner (§12).

**Q0 — targets, budget, relation.** The owner first fixes the targets and the
budget table below. Targets are fixed before any measurement so that no result
can move them silently. A target changed later is an owner decision, recorded
with the measurement that prompted it. The relation is then fixed in two
variants, lean and per-hop, after deciding the relation choices listed below,
with predicted rows, columns, key size and one-bank memory for each. A variant
whose prediction misses the memory or key target is changed or dropped before
Q1.

| Target | Current repo gate | To be fixed |
|---|---|---|
| End to end: payer confirms to receiver sees the credit, transfer included, p95 | none defined; the gates below sum to far more | 2 s proposed. The owner said "1-2 s should be good ux". The percentile, the end points, and whether a proof-carrying payment must meet the same figure are for the owner to confirm |
| Proving, p95 | 10 s | |
| Complete handoff (send, payment, receiver's fold, across two phones), p95 | 30 s | |
| From a transition to its proof being ready, on one phone, p95 (the wait before paying again or spending received value) | none defined | |
| Receive then re-spend on one phone, p95 | none defined | |
| Verification of an incoming payment, p95 | 1 s | |
| Peak process memory | 128 MiB (repo-defined, not a platform limit) | |
| Proving key / verifying key | 64 MiB / 64 KiB | |
| Energy per payment | a measurement is required by the release model; no limit is set | |
| Payment message, binary | 7,552 B payment; 9,211 B whole exchange; 6,528 B proof | |
| Proof completion: failures in foreground, background and under memory pressure | none defined | Zero failures in the Q2 runs proposed. A phone that cannot complete a proof can use its balance only by unloading |
| Host budget for Q1 (wall time and peak memory for one proof) | none defined | |
| Floor device | Pixel 6 is the repo's mandatory profile | |

The memory target has to be settled in Q0: it is raised, the SHA-256 claim
chain is removed from the relation, or the Claim is split as the
phone-algorithm spec sketches (slice arithmetic test-only, the rest
unimplemented; memory unmeasured).

The budget table. Blanks are the owner's. The proposed values are estimates for
one engineer who knows this stack. None is a commitment and the basis of each
is stated.

| Stage | Proposed budget | Basis | Budget | Date |
|---|---|---|---|---|
| Q0b, baseline measurements | 2 engineer-weeks, plus the phones | This document's guess. The work is small test apps over carriers and key stores the repo already has. No measured basis | ____ | ____ |
| Q0, relation in two variants with predictions | none proposed | Not estimated. It includes the relation choices below | ____ | ____ |
| Q0a, stack check (optional) | 4 engineer-weeks, as a time box | Upper end of an automated estimate of 1.5 to 4 | ____ | ____ |
| Q1, host | none proposed | An automated estimate gives 8 to 14 engineer-weeks for the per-hop variant's proofs alone if the relation stays shaped like Recursive V1. That shape has the proof on the payment path, which the timing reading excludes. The lean variant, the proof made beforehand, the refund path, Migrate and any new relation are not estimated | ____ | ____ |
| Q2, floor device | 5 engineer-weeks | Upper end of an automated estimate of 3 to 5, once Q1 delivers keys and a witness | ____ | ____ |
| Q3, soundness | none proposed | Not estimated. It includes an independent review | ____ | ____ |
| Total, all stages and one reconsideration | none proposed | The stages that have an estimate sum to about 15 to 25 engineer-weeks | ____ | ____ |
| Reconsiderations of the relation or proving system allowed | 1 | Each one is a new relation and circuit stack, which this document cannot estimate | ____ | |

Layer A has no estimate in this document. One is needed before the owner can
compare the cost of B1 with the cost of the design without it.

**Q0b — baseline measurements.** These need no prover. They start as soon as
the targets are fixed and run beside the relation work. Each is made on a
Pixel 6, one low-memory Android phone and one iPhone, over at least 100
operations, and recorded as median and p95.

- Carrier throughput. Payloads of 0.9 KB and 7.4 KB, in each direction, on
  each pair of phones, on each carrier of §5.6: the repo's QR framing at 5 and
  12 frames per second, a denser QR framing, NFC where the pair allows it, and
  each radio carrier §5.6 keeps. Time from the first frame or tap to a
  complete decode, second passes included.
- Hardware signing. One P-256 signature by the device key under each
  authentication mode §5.9 keeps; on Android for a TEE key and a StrongBox
  key.
  The same with a caller-supplied 32-byte digest, which settles whether the
  "Algebraic signed digest" choice below is open on each phone.
- Marker step. One marker creation and deletion, with the durable commit
  between them (§5.10).
- The exchange without a proof. Request, Payment and Outcome with real
  hardware signatures, marker steps and a carrier, from the payer's
  confirmation to the receiver's credit. This is the time every option
  spends. What is left of the target is what a proof may use.
- Background execution. A stand-in computation sized to the proving and
  memory targets is started after a payment. Record whether it completes
  with the app in the background, with the screen locked, in low-power mode,
  and under memory pressure, and after how long the platform suspends or
  ends it.
- One-use keys. Generate an ordinary TEE key limited to one use, with an
  attestation challenge, and record whether the limit is listed as
  hardware-enforced. Repeat for a StrongBox key on each vendor. The repo's
  probe never generated the TEE key (§2.2). Where a limit is
  hardware-enforced, also begin two operations before finishing either, and
  repeat after a security-patch update (§4.1).

**Q0a — stack check (optional, time-boxed).** Run the existing ignored lineage
tests for the secure-hardware relation under an optimized profile. This shows
whether the stack can produce a proof at all. It is not a baseline for the
stock-phone relation and not a gate.

**Q1 — host.** For each variant, lean first: one complete lineage with real
proofs in both fields. Bootstrap, MintFold, SendSplit, ReceiveFold of another
party's payment, the refund path of §2.1, RedeemSplit, an unload that carries
transitions the wallet has not proven, Migrate, receiver-side verification,
node-side verification with the authenticated verifier. Record time, memory,
key and proof size, separately for the P-256 verification and for the rest. A
variant that misses the host budget is dropped. For the per-hop variant this
needs: the production refusal lifted, an ordinary ReceiveFold consumer, key
generation under the column ceiling, and a Guard re-specified against Layer
A's objects (the existing one is bound to the online profile's approval
objects). Passing the column ceiling is not passing the memory target (§2.2).

**Q2 — floor device.** No harness exists; it needs a bench entry point,
host-generated keys and captured witnesses, and iOS and Android test wrappers.
On a Pixel 6, one low-memory Android phone and one iPhone, over at least 100
operations each plus a sustained run to thermal steady state, measure: send
proving and peak memory; end-to-end verification of an incoming payment;
receive proving; the time from a transition to its proof being ready; complete
handoff and receive-then-re-spend; energy from the fuel gauge or a power
monitor; proof completion in the background and under memory pressure; message
bytes. Hardware signing and the carriers were measured in Q0b.

**Q3 — soundness.** A variant that met the targets enters soundness
qualification: a mutation test per constraint, the 1,024-handoff run,
independent review. With the proof made beforehand, every mutation is also run
against the receiver's native check of the last hop, and the two must reject
the same inputs. Passing targets is not qualification.

**What each outcome leads to.** A stage ends at its exit result, at its date,
or when its budget is spent, whichever comes first. No stage extends itself.

| At the end of | Outcome | It leads to |
|---|---|---|
| Q0b | The exchange without a proof misses the end-to-end target on a carrier the owner requires | The owner changes the target or the carrier before any prover work. This outcome says nothing about Decision B |
| Q0b | The exchange without a proof meets the target, but no required carrier moves 7.4 KB in the time left | B1 needs a smaller proof, a proof that crosses before the payer confirms, or a longer target for proof-carrying payments. The owner picks one in writing, or B1 is excluded on timing |
| Q0b | A platform ends background computation before the proving target | The proof made beforehand needs the app held in the foreground on that platform after each transition. The owner accepts that, or that shape is dropped for that platform |
| Q0 | Neither variant has a predicted fit to the memory and key targets | This uses the one reconsideration. If it is already used, the final gate |
| Q1 | A variant completes the lineage within the host budget | That variant goes to Q2 |
| Q1 or Q2 | Only the lean variant passes | The owner chooses: continue with the lean relation, accepting that it gives no attribution (§2.1); use the reconsideration on the per-hop relation; or the final gate |
| Q1 or Q2 | No variant passes | The one reconsideration, within the remaining budget and date. If it is already used, the final gate |
| Q2 | A variant meets every target on every floor device | That variant goes to Q3 |
| Q3 | The variant passes | B1 with that relation is available. The owner takes Decision B in writing, with the measured cost of B1 beside the table in §2 |
| Q3 | The variant fails | The defect is fixed within the remaining budget, or the final gate |
| Any stage | Its date passes or its budget is spent without the exit result | Work on the stage stops. The final gate |

The final gate is an owner decision in writing among three options.

- B2, with what §2.4 lists.
- One named constraint is relaxed and one more cycle runs under a new budget
  and date. The candidates are the absence of a trusted setup, the 10 KB
  bound, the timing target for proof-carrying payments, and the hardware of
  R2.
- No production release of offline value.

Passing every stage makes B1 available. It does not choose it. Until one of
these decisions is written, Layer A carries no production value (§2).

**What can be built before the relation is fixed.** Every signed preimage
starts with `tag ‖ scheme id` (§5). Work done before Q0 completes uses a test
scheme id. Its encodings, vectors and enrolled keys are discarded when the
relation is fixed. This rule is what keeps early work from fixing the relation
by accident.

Built now without rework, because they do not depend on any signed byte
format:

- the device tests of §10.4 and the Q0b measurements;
- the carriers of §5.6, which move opaque bytes and must be sized for 7.4 KB
  as well as 0.9 KB;
- the attestation verifiers of §10.3, except the one policy value that says
  which digest authorization an Android key must have;
- storage, the durable commit and the marker (§5.2, §5.10);
- the block index and the derivation of the list (§5.5);
- pool accounting, the backstop, the limiters and claims (§8.2 to §8.4).

Built now as logic over typed fields, with the byte formats and the signature
checks behind an interface that is replaced when the relation is fixed:

- the wallet core's time rules, limits, expiry and block-list checks (§5.4,
  §5.5);
- the registry, the registration flow and the receipts (§8.1);
- the evidence predicates (§5.3);
- the issuer service: enrollment, sync, replay and key custody (§6).

Not fixed before the relation, because the relation decides them. This list is
the circuit-facing surface:

- the transition preimage and its digest function, and the state commitment;
- the digest authorization of the device key. On Android it is fixed and
  attested at key generation, so no production enrollment happens before
  this is settled;
- the signature schemes of the certificate, the receipt and the voucher;
- the shape of the load and unload instructions, and the replay guard for
  mints;
- the payment envelope, the Outcome, and whether an offline refund exists
  (§2.1);
- what the receiver learns about the payer's balance (§5.7);
- the shape of the fee schedule and how a SendSplit binds its fee policy
  (§5.8);
- §9's rule on when a proof is made relative to the commit;
- every conformance vector.

Relation choices to decide in Q0:

- **Proving off the payment path.** The 2 s target leaves no room to prove
  while two people wait. The payment would then carry a proof of the payer's
  state before this payment, made in the background after the wallet's previous
  transition, plus the signed SendSplit. The receiver checks the SendSplit
  against that state without a new proof. Its own next proof, also made in the
  background, absorbs the SendSplit and its hardware signature. What follows:
  - The receiver must check, before it commits, that the proven balance
    covers the amount and the fee. Otherwise a compromised phone pays any
    amount over a proven state of one unit, the receiver's fold can never be
    proven, and the payment is final all the same. So the payer shows its
    balance, and the receiver learns it (§5.7), or a range proof is made
    while the two wait.
  - The receiver accepts the last hop on its own native check, the same kind
    of check B2 makes. Whatever that check accepts and the circuit rejects is
    value the operator must honour. The two must accept exactly the same last
    hops, and Q3 tests that.
  - If they ever disagree, through a bug or an input made to pass one and
    fail the other, the receiver holds final value that it cannot prove and so
    cannot spend offline, and the payer has released a transition it cannot
    prove. Under the proof on the payment path an unprovable transition is
    never released.
  - A wallet cannot make a second proof-carrying payment until its new state
    is proven. Received value is spendable offline only after the receiver's
    fold is proven (§9).
  - §9's rule that a released transition is proven before the commit changes.
  - §2.1's refund rule must be restated. A refusal cannot be proven while the
    two wait. So the Outcome carries the receiver's earlier proof with a
    signed refusal, the payer checks the refusal against that proof natively,
    and the payer's next proof absorbs it. The alternative, no offline refund,
    is the one §3.1 does not allow.

  This is a sketch to evaluate, not a result.
- **Redemption without a phone proof.** A received payment is final and
  redeemable whether or not the holder's phone ever completes a proof (F1).
  Under either B1 shape a wallet can hold committed transitions that it has
  not proven. So the unload path must not require a proof of them. The ledger
  verifies the last proof the wallet has. It checks the signed transitions
  after it with the same native check a receiver runs, including the proof
  inside each payment received, so that whatever a receiver accepted the
  ledger redeems. This rule is what keeps a phone that cannot prove from
  stranding its holder's value. It costs the ledger one proof verification
  per unproven payment in the unload, and the unload has to be bounded in
  size. It is not designed.
- **Issuer keys and circuit releases inside a proof.** A per-hop proof
  verifies earlier holders' certificates and receipts against issuer and
  witness keys, and every proof is made under one circuit release. A holder
  who never syncs still holds a proof made under old keys and an old release.
  Either every key and release ever valid stays accepted inside proofs, and
  then a revoked key or a withdrawn release cannot be removed from them; or
  it is removed, and then every balance whose history contains it cannot be
  paid offline until its holder syncs. The second is a forced sync that no
  regulatory control causes (F3), and it reaches further than the
  key-revocation rule of §5.1, because value spreads. This has not been
  analysed. Q0 must state which it is.
- **Algebraic signed digest.** Android documents signing a caller-supplied
  32-byte digest (`DIGEST_NONE`). The Secure Enclave API documents signing a
  digest the caller provides; that it accepts 32 bytes that are not a SHA-2
  output is an inference and needs a device test. App Attest assertions cannot.
  It removes SHA-256 from the device-signature check. SHA-256, and with it the
  claim chain, leaves the relation only if the certificate, receipt and
  state-commitment head are algebraic too. It does not remove the non-native
  P-256 arithmetic, which is the larger cost. Caveats: the hardware then signs
  whatever 32 bytes the app supplies; the repo's Poseidon parameters are not
  independently qualified; StrongBox support is test-enforced rather than
  mandatory and needs a device check. Android fixes and attests the digest
  authorization at key generation, and the repo's key policy is SHA-256-only.
  The key must be generated with `DIGEST_NONE` authorized before the first
  enrollment, or choosing this later re-enrolls every Android phone.
- **One P-256 verification per hop.** Certificates and receipts are
  operator-controlled formats. In a proof-friendly signature scheme they verify
  natively to the circuit, leaving only the device signature non-native.
- **Mint authorization.** Today a mint is an exact quorum of validator
  Pasta-Schnorr seals over a top-up root plus a recursive authority chain,
  verified as two inner proofs on every step. An issuer signature instead moves
  mint authenticity from validator keys to the voucher key: a stolen voucher
  key then mints value every proof accepts. If that is not accepted, keep the
  quorum seal and confine it to a MintFold-only circuit. This choice decides
  the last row of the second table in §2.
- **Consumed-credit set.** The current depth-256 path costs 3,084 Poseidon
  permutations per field per step (counted from source). A replacement must
  keep non-membership-then-insert in one step and the full credit id in the
  leaf, state its capacity, and never prune by time. With single-use Requests a
  per-receiver receive counter can replace the set for ReceiveFold. The set
  also guards MintFold, so mints then need their own in-circuit replay guard,
  for example an issuer-assigned per-wallet mint sequence.
- **Per-operation circuits** instead of one fixed-shape relation verifying five
  inner proofs on every step. This needs a uniform wrap proof or verifying-key
  selection.

Published reference points for one P-256 verification. For size: about 40,000
constraints in Kimchi on Pasta, with no time given, and about 0.5 million
advice cells in a third-party halo2-lib fork (4.4 s on a laptop, KZG on BN254).
Phone timings exist for other proof systems. A third-party benchmark reports
about 6 s in a browser on a Samsung Galaxy A23, and an out-of-memory failure on
an iPhone 16, for one P-256 verification in Noir. A credential system that
works in P-256's own field reports a few hundred milliseconds on mobile for a
flow that includes ECDSA verification. None of these is recursive, and none is
a Halo2-family or Pasta-IPA prover. No phone measurement was found for a P-256
verification in such a prover, or for any recursive construction that contains
one. The sources are in the evidence appendix, section 3.

If Q2 fails, the alternatives depend on which constraint the owner relaxes. "No
trusted setup" is not among R1–R9. No KAGEMUSHA spec states it; it follows from
Recursive V1's IPA stack and the workspace ZK policy, which rejects
trusted-setup backends (`specs/zk_envelopes.md:8`,
`specs/zk_cryptographic_audit.md:214, 1186`). Whether it binds here is an owner
decision, and relaxing it changes that policy too. A circuit-specific setup has
published sub-second phone timings for single non-recursive proofs. A universal
setup has published Pixel 6 timings of about 1 to 8 s for single circuits. A
message cap of tens to hundreds of KB admits hash-based recursion. No published
phone measurement for merging two parties' proofs was found for any of these.
With no setup and 10 KB both fixed, Halo-style accumulation is the only family
for which an implemented cross-party merge was found. That is an absence of
counter-examples, not an impossibility result.

### 2.4 If B2 is chosen

The operator underwrites every loss below (F2, §8.4). No holder bears any of
it. Limiters may delay a payout; they never reduce it. Nothing a receiver
verifies bounds counterfeit. The owner accepts, explicitly:

- Arithmetic is trusted, not proven. Mitigations: one wallet core, a rules
  version in every transition, issuer replay at sync. A common bug in the one
  core, or an old build that never syncs, is not caught. Value such a bug
  creates is accepted by honest receivers and is final.
- A compromised phone can fabricate value with a self-consistent chain that no
  evidence contradicts. It needs no load. Nothing a receiver verifies bounds
  the total. With R7 and R8 on, each honest receiver's tally and the lease
  bound what one receiver accepts, and nothing bounds how many receivers the
  phone reaches (§3). With R8 off there is also no issuer-side signal (§8.3).
- Migrate moves whatever the old journal claims.
- A stolen voucher key mints unbacked value onto genuine phones (§8.1). Under
  B1 that holds only if the mint is issuer-signed (§2.3).
- The backstop is sized to an assumed number of compromised phones and
  receivers reached (§8.3 item 6). The protocol supplies no upper bound on
  the loss.
- The choice is hard to reverse. Adding a proof to an open pool later needs
  every holder to sync once, or a second pool (§2, "Position").

What B2 does not change: when a payment is final, that no payment needs the
network, and what an ordinary user with the unmodified app can do (§4.1). The
difference from B1 is the second table in §2: bug-created value, the voucher
key if the mint would have stayed quorum-sealed, the one real load a
compromised phone must make, and the per-hop evidence against an attacker with
one enrolled instance.

Choosing B2 does not remove the Pasta implementation. Removing the recursion
code and its consensus coupling is a separately approved change (§11).

## 3. What the requirements cannot buy

The owner's statement (§1) asks for two things together. A payment between two
phones is final. Nobody has to go online afterwards unless there are
regulatory controls. On stock phones (R2) the two have one price, and the
operator pays it.

A receiver that is offline cannot tell a payment made by a compromised phone
from an honest one (§4.1). If the payment is final, the receiver keeps the
value. If nobody has to go online, nobody can be asked for anything later. So
the operator honours value that compromised phones create. With the regulatory
controls off, nothing in the protocol bounds that value, and nothing reveals it
until total redemptions exceed total loads (§8.3). With them on, the bound is
per receiver and per window, not a total, and R8 and R6 freshness are exactly
the controls that send holders online.

**What the statement gives.**

- A received payment is final (F1). The receiver's wallet enforces it: the
  wallet never removes a committed credit. The ledger enforces it at unload:
  the chain checks the redeeming device's registry row and signature and pays
  the increase in its cumulative total (§8.2). No ledger rule looks at who
  paid the redeemer, so later evidence against a payer, a block of a payer, a
  revoked key or a changed policy has nothing to act on.
- One loss rule (F2). The operator funds every claim. No holder's balance is
  written down. §8.4 has the rule.
- No holder is sent online except by a regulatory control the scheme switched
  on, or by one of the exceptions that §3.1 keeps (F3). §3.1 lists every rule
  that could do it and what becomes of each.

**What it costs.** These follow from R2, R4 and R5 on stock phones under either
option of Decision B.

1. **A compromised phone can re-spend without bound, and receivers cannot tell
   offline.** The operator pays for all of it (F2). What each control does,
   and who applies it:
   - R7 on. Each honest receiver accepts at most the payer's limit per
     window from one payer, and at most twice that in one real day or month
     across a window boundary. The receiver enforces that from the limit in the
     payer's certificate and its own tally of credits from that device id
     (§5.4). The protocol does not limit how many receivers a compromised
     phone reaches, so the total is not bounded.
   - R8 on. A certificate the issuer stops renewing ends at its lease, among
     receivers whose clocks are right (§5.4). A compromised phone that keeps
     renewing is stopped only by a block entry (§8.3).
   - R6. A block entry stops a device at each receiver that holds the entry.
     A receiver gets the list at a sync or from a peer.
   - All off. Only `max_payment` remains, applied to each payment. The
     per-counterparty cap counts as a limit under R7 (§5.4), so it is off as
     well. Nothing bounds the total, and the issuer has no signal (§8.3).
2. **"Final" has a precise meaning.** A received payment is irrevocable by the
   payer. It is the receiver's from the moment the receiver's wallet commits
   it. It is a claim on the operator at face value, with no time limit, that
   does not depend on the payer's later status. It is spendable again offline
   subject to the holder's own certificate and limits and, under B1, a
   completed proof (§9). Three things it is not:
   - It is not immediate redemption. Rate limits may delay a payout (§8.2).
   - It is not a guarantee that the operator can pay. The protocol makes the
     operator liable. It does not make the operator solvent.
   - Under B1 it is not spendable until the receiver's phone has proven its
     fold (§3.1).
3. **A lost, reset or reinstalled phone loses its offline balance.** Under R4
   and R5 any recovery is a double-spend channel and can only be capped
   insurance. It is off by default (§7.3).
4. **The exchange is not atomic.** The payer is debited when its signed
   transition commits, before the Payment is released. If the Payment never
   reaches the receiver, or a `Refused` Outcome never reaches the payer, the
   amount is in neither wallet. It comes back only when the two phones meet
   again and the payer presents the Payment once more. No sync-time recovery
   is defined (§12 item 26). §3.1 says why this order is kept.
5. **Time-based controls are only as good as device clocks.** No phone gives
   apps an attested clock (§5.4).
6. **The regulatory controls are the only risk controls, and they are what
   sends holders online.** R7 needs no sync. R8 needs one sync per lease. R6
   freshness needs one sync per `receive_not_after` period. With R8 off there
   is no issuer-side signal of counterfeit at all (§8.3). R7 and R8 stay
   optional. This document proposes that the shipped default tier has both
   on; the owner decides (§12 items 6 and 19). "Never" and "unlimited" are
   explicit values that governance acknowledges as unbounded exposure for the
   operator.
7. **A balance can still be lost or stopped without any regulatory control.**
   §3.1 lists the cases this document could not remove: a stolen operator key,
   a lost marker or journal, a dead key, a lost account key, a fraud hold on a
   phone that accused itself, an undelivered payment, and, under B1, value
   not yet proven.

### 3.1 What can still send a holder online

The table lists every rule in §4 to §9 that suspends paying or receiving until
a sync, strands a balance, or destroys one. "Regulatory" means R6, R7 or R8,
switched on by the scheme. A rule that is not regulatory is removed,
redesigned, or kept as an exception for the owner. Where a rule is removed or
redesigned, "Was" gives the rule it replaces. Each redesign is specified in
the section named in the first column; this table states what it must achieve.

| Rule | Trigger | Effect on the holder | Regulatory | Ruling |
|---|---|---|---|---|
| Send expiry (§5.4) | The certificate's `Lease` has passed `not_after + expiry_grace` | Cannot send until a renewal sync. Receiving and unloading continue | R8 | Kept |
| Reboot under `require_anchor` (§5.4) | Any reboot, where the tier chose this policy | Cannot send until the phone reaches the issuer | R8 (reboot policy) | Kept. It is an opt-in tier value |
| Receive freshness (§5.4, §5.5) | `receive_not_after` has passed | Cannot create a Request until a sync. The balance stays spendable and redeemable | R6 | Kept |
| Block entry from the account block index (§5.5) | The ledger blocks the account | Payers holding the list refuse a `receive_blocked` device. Receivers holding it refuse a `send_blocked` one. Lifted only by a renewal after the ledger unblocks | R6 | Kept |
| Day or month limit (§5.4) | The counter has reached the limit | Cannot send more until the window turns. No sync is needed | R7 | Kept |
| Clock-reset state, where a certificate carries a lease or limits (§5.4) | After a reboot the wall clock is more than `clock_regress_tolerance` behind the wallet's last known time | Cannot send under that certificate until the clock is back within the tolerance or the phone re-anchors. If the last known time is ahead of real time, it ends when real time comes within the tolerance of it, or at a sync. A payer whose certificate carries a lease or limits refuses this wallet's Request | R7 or R8: it stops only what needs time | Redesigned. The state no longer stops Request creation. The Request is marked, and a payer with no lease and no limits pays it |
| Clock-reset state, no lease and no limits (§5.4) | As above | Was: no Request could be created | No | Removed. Sending and receiving are unaffected |
| Tightened tier row (§5.1, §5.11) | A tier-row notice lowers a limit, or brings in or shortens a lease | As the rule read, peers holding the new descriptor refused a certificate above the new row until the holder renewed. Now the stricter of the certificate's term and the row's applies, term by term, as soon as either side holds the notice | R7 or R8 with new values | Redesigned (§5.11). No certificate is voided. At its strictest the holder cannot send until the next window or the next sync. Receiving and unloading continue |
| Issuer certificate key or witness key revoked (§5.1) | The root revokes a key after a compromise | The phone can neither pay nor request from the moment it holds the revocation until it syncs. The revocation passes from phone to phone | No | Kept for now as an exception. Owner decision; options below |
| Witness receipt for release above own loads (§8.1, §8.2) | A witness-key revocation leaves a row's recorded receipt below its quorum | The claim stays recorded. Nothing above the row's own loads is released until a receipt under the new witness keys is recorded in the row | No | Kept as an exception. Owner decision (§12 item 43). The rule makes the witness quorum a second lock on the unload door. Its cost is that an honest net receiver's payout waits on the witnesses, on no date the protocol sets |
| Forged block entries under a stolen list key (§8.1) | A thief of the list key signs entries against honest devices | Receivers holding the forged list refuse the device until a root-signed epoch bump reaches them, from a peer or at a sync | No | Kept as an exception. It follows from R6: a list that peers obey can be forged by whoever holds its key. No value is lost |
| State-lost on a failed read (§7.2) | The key, the journal or the marker cannot be read: locked storage, an error code | Was: the wallet deleted its key | No | Redesigned (F5). A read that fails is never taken as absent. The wallet signs nothing, deletes nothing and reads again at the next unlock or launch. It deletes no key on any condition it detects itself |
| Inconsistent wallet: marker or journal absent while the key lives, or the issuer holds a transition this key signed that the journal lacks (§5.10, §7.2) | The store answers that the item does not exist. On iPhone, removing the passcode deletes the marker | The wallet stops paying, requesting and unloading, and keeps everything. If the marker is still present, an online repair restores the journal from the issuer's copy. If the marker is gone, the balance cannot be used again | No | Kept as an exception. Was: the wallet deleted its key. Any path that gives the balance back without the marker is a reset an ordinary user can perform: back up, pay, remove the marker, restore. Avoiding it makes every such reset a double spend the operator pays. Recovery insurance (§7.3) is the capped form |
| Dead key (§3 item 3, §7.2, §7.3) | The phone is lost, broken or erased, or the app is uninstalled or its data cleared | The balance is gone. The user declares the loss and enrolls again; nothing is restored | No | Kept as an exception. Only a signature by that key can show the balance was not spent. Avoiding it is recovery insurance: capped, and off by default |
| Device id retired by its account (§7.2) | The bound account declares a loss. Whoever holds the account key can do it to a live phone | Holders of the list refuse the device. It can still unload everything it holds | No | Kept as an exception. Retirement by itself takes no value. Where someone else holds the account key, it leaves the phone one way out among holders of the list, an unload, and that pays the account the other party controls; before the retirement the holder could still pay the balance away offline |
| Account key lost (§8.2) | The holder loses the key of the bound account | The wallet still pays and receives offline. It cannot unload or Migrate | No | Kept as an exception. An unload pays only the bound account, and no instruction changes it. The balance stays spendable offline |
| Device key invalidated by a settings change (§5.9) | Android: the secure lock screen is removed or reset, or biometric enrollment changes, with a key bound to user authentication | The key is dead; the balance is gone | No | Redesigned, if the owner agrees (§5.9, §12 item 23). The device key carries no authentication requirement, so no settings change invalidates it. The app shows the platform's authentication prompt before it asks the key to sign. Cost: the hardware no longer refuses a payment signature without authentication. A thief who does not know the PIN and who compromises the phone can then pay |
| Recovery claim (§7.3) | The account holder files a claim | Was: the old id's row was suspended, then retired at payout; a live phone that stayed offline lost its balance; defending needed a sync | No | Redesigned. Recovery is off by default. Where an operator enables it, a claim never suspends, blocks or retires the old device id |
| Fraud hold on a phone that accused itself, or on a Migrate successor (§5.3, §7.2, §8.2) | Two signatures at one sequence number reach the chain. An honest phone can produce them through a commit lost after release or a platform restore | The row takes no Load and no Unload. Peers holding the entry refuse the device. Was: the unpaid claim was cancelled | No | Kept as an exception, changed in one point. The hold freezes the row; it cancels nothing. A governed reinstatement to the bound account must exist before production value. The chain cannot tell a fault from a cheat, and a forked row left open draws `unload_limit` per window without end. The honest holder goes online once |
| Migrate abandons unresolved payments (§7.2) | The user migrates while a payment has no stored Outcome, or after one of its Requests has expired unpaid | A later `Refused` Outcome can no longer be refunded. A late Payment to one of the old id's Requests gets no Outcome, so its payer gets no refund | No | Kept as an exception. The user chooses when to migrate, and the app lists the open payments with their total first. Avoiding it means the successor may fold a `Refused` Outcome for the old id's SendSplit and answer for the old id's Requests: one more transition kind and its evidence rules. Not designed. Owner decision |
| Request retention (§5.2) | The retention period passes with no Outcome stored | Was: a late Payment got no Outcome, and the payer's debit became permanent | No | Redesigned. A wallet answers any authentic Payment addressed to a Request it signed, with no time limit. If its journal holds the ReceiveFold for that payment it answers `Credited`. If the Request has expired or was decided for another payment it answers `Refused`. The Payment carries only the Request's digest, so the wallet keeps every Request it signed on record for as long as the wallet exists (§5.2) |
| Undelivered Payment or Outcome (§5.2, §3 item 4) | The scan is interrupted, or the receiver withholds a `Refused` Outcome | The amount is in neither wallet until the two phones meet again | No | Kept as an exception. No exchange between two phones is atomic without a third party. Debiting the payer first, with no refund on a timeout, is the order in which an interruption cannot create value. A refund on a timeout would let any two users create value by not scanning the Outcome |
| Marker delete fails (§5.2, §5.10) | The key store refuses the delete | Nothing is released until the delete succeeds. The payer is debited and the Payment is held | No | Kept. It is a retry on the phone, not a sync. If the key store never recovers, the wallet stays unavailable (§7.2). It signs nothing and unloads nothing while that lasts, and no length of time turns it into a loss |
| Unload limit (§8.2) | A claim is above the row's own loads, or pool cash is short | The payout waits. Was: tier and pool limiters served first come each window, so other rows could take every window; and a claim above the headroom was not recorded | No | Redesigned (F2). A claim is recorded in full and never reduced or cancelled. Above its own loads a row is released `unload_limit` per window, on dates fixed when the claim is recorded and independent of other rows. No cap is shared between rows. When pool cash is short, amounts due queue first in, first out, and the operator funds the queue (§8.4) |
| Holder-borne loss rule (§8.4) | A shortfall | Was: the unpaid part was the holders' loss | No | Removed (F2) |
| Load left without a voucher (§7.1) | The issuer answers a committed load with neither a voucher nor a signed statement that the load id is void | The loaded amount stays in the pool as the account's claim. No rule ends the wait | No | Kept as an exception. The chain cannot see vouchers. A time limit enforced by the chain needs every voucher anchored on-chain before release, which is one more write per load |
| Unproven received value, B1 only (§9) | The receiver's phone has not yet proven its fold | The credit is final but not spendable offline until proven. If that phone can never prove it, the credit can leave only by unload | No | Exception under B1, for Decision B. F1 holds under B1 only if the ledger redeems a credit that passed the native check without the holder's proof (§2.3, "Redemption without a phone proof"). Not designed and not measured |
| No offline refund, B1 only (§2.1, §2.3) | Q0 chooses that refunds do not exist offline | A refused payment's amount is in neither wallet until a sync | No | Not allowed (F3). Q0 keeps a refusal the payer can fold offline (§2.1) |
| Revoked key or withdrawn circuit release inside a proof, B1 only (§2.3) | A proof's history contains a key that is later revoked or a circuit release that is later withdrawn | Either the key or release stays accepted inside proofs for good, or every balance whose history contains it cannot be paid offline until its holder syncs | No | Exception under B1, for Decision B. Q0 states which. The second choice is a forced sync that spreads with the value |
| Rules-version floor (§5.11) | The root raises `rules_floor` to retire a version | From the effective date an app whose highest version is below the floor can neither pay nor be paid by wallets that hold the notice, until the app is updated. The balance is unchanged and unload stays open | No | Kept as an exception, if the owner allows a floor at all (§5.11). A version with a defect that creates value stays acceptable to every receiver that supports it until those receivers stop listing it. Avoiding the floor retires a version only at renewal, and never under a `Never` certificate. With no floor, payer and receiver use the highest version they share, and with none in common nothing is signed |
| Planned key rotation and scheme closure (§5.11) | The operator replaces a key on schedule, or closes the scheme to loads | Nothing. What the old key signed stays valid on its own terms. Closure ends loads and enrollment; payments, renewals and unloads go on | No | No exception. Redemption never ends (F1). A holder whose phone is failing after closure cannot Migrate and unloads instead |

Not in the table: steps that are online by nature. Enroll, load, unload, renew,
Migrate, a repair, a declaration of loss and a recovery claim need the issuer
or the chain, and under witness model (B) a new device waits the seasoning
delay (§8.1). An iPhone whose App Attest key died needs one online
re-attestation before its next sync (§7.2); offline it pays and receives as
before. None of these stops a wallet that is already working offline.

**Key revocation: three options.** The certificate key signs device
certificates. The witness keys sign registration receipts. A thief of the
certificate key alone cannot make a new device acceptable, because a receiver
also requires a receipt (§8.1). A thief of both can certify and register
software keys, which is unbounded counterfeit. Before a revocation reaches a
receiver, that receiver accepts the thief's objects under every option. The
options differ in what happens afterwards.

| Option | Honest holders under the revoked key | Thief of the certificate key, at a receiver holding the revocation | Thief of the certificate key and the witness quorum, at that receiver |
|---|---|---|---|
| 1. Suspend until sync (the rule in §5.1) | Each can neither pay nor request from the moment it holds the revocation until it syncs. One synced phone stops every phone it hands the revocation to. A thief can cause this by forcing the operator to revoke | Nothing | Nothing |
| 2. Certificates stand until their own expiry | With R8 on: nothing extra. Each phone gets a certificate under the new key at the renewal R8 already requires. With a `Never` certificate: nothing, or one sync within a grace period the owner sets | For a registered device whose holder cooperates: a certificate that this receiver accepts for at most one lease after the revocation time. That evades a refused renewal for one lease. It cannot exceed the tier row, lift a block entry, or make a new device acceptable | Software keys that this receiver accepts for at most one lease after the revocation time. Unbounded counterfeit for that long; the operator pays |
| 3. Never revoke | Nothing | For registered devices whose holders cooperate: renewals without end, and lifting every R6 block entry by signing a higher serial | Unbounded counterfeit without end |

Option 2 needs four rules. Each is checked by a receiver from the root-signed
revocation, the root-signed tier table and its own time.

- The revocation carries a root-signed revocation time. A receiver holding it
  accepts a certificate under the revoked key only if the certificate has not
  expired on the receiver's own time, and its `not_after` is no later
  than the revocation time plus the longest lease the tier row allows. The
  second test is there because the thief can sign any `not_after`. A payer
  applies the same two tests to a receiver's `receive_not_after`.
- A receiver holding the revocation treats a block entry for a device id as
  covering every serial under the revoked key. Otherwise the thief lifts R6
  entries by signing a higher serial.
- A receipt under revoked witness keys stands for as long as the certificate
  presented with it stands. The issuer hands over a receipt under the new
  witness keys at renewal, and only for a device that has a registry row.
- A `Never` certificate has no expiry to stand until. Either it stands
  without limit, which is option 3 for that tier, or it stands for a
  root-signed grace period after the revocation time. A grace period is a
  forced sync with a deadline, so it is an exception to F3 for that tier.

Under option 2 the bound is only as good as the receiver's clock, like every
expiry (§5.4).

This document recommends option 2. With R8 on it adds no forced sync, and it
removes the mass suspension that option 1 lets a thief trigger. Its cost is
that a thief of both keys keeps one more lease of counterfeit at each receiver
after the revocation arrives there, at the operator's expense. Option 3 is
acceptable only to an operator who accepts unbounded loss from a key theft.
For `Never` tiers the owner chooses between a grace period and no limit. Until
the owner decides, §5.1 keeps option 1, and it is an exception to F3.

## 4. Trust model

What the design relies on:

- T1. The device key is non-exportable and, on an uncompromised OS, usable only
  by the enrolled app.
- T2. What platform attestation shows at enrollment (on iPhone also at the
  one-off re-attestation of §7.2, which repeats the same reliance), never per
  payment:
  - **Android.** The attested Keystore key is the payment key. The chain shows
    hardware custody, locked verified boot and patch level at key generation.
    The app identity in it is asserted by the OS, not by the secure hardware.
    Nothing is re-attested for an existing key.
  - **iPhone.** Apple certifies the App Attest key only: an Apple-rooted
    certificate for a key Apple documents as held in the Secure Enclave, the
    App ID, the environment, and a nonce covering a value the caller chose. On
    iOS 27 and later it also carries a distribution category and bundle
    version. It carries no OS version, patch, boot or jailbreak state. The
    payment key is a second, app-created Secure Enclave key that Apple offers
    no way to attest; its public key is hashed into the caller-chosen value.
    That proves that code under the App ID holding the attested key named
    those bytes. It does not prove the bytes are a Secure Enclave key, that the
    key is on the same device, or that the caller holds it. iPhone payment-key
    custody therefore rests on two things attestation does not show: the OS
    was uncompromised at enrollment, and the build that enrolled was a released
    build that creates the key in the Secure Enclave. If either fails the
    issuer has certified an exportable key. The existing verifier requires the
    two keys to differ.
  - Neither platform attests wallet state.
  - **R8 renews a certificate, not an attestation.** A renewal is the issuer
    signing a new device certificate for the same key. On Android the
    attestation was made once, at key generation. Its boot state, patch level
    and app identity are those of that day, however often the certificate is
    renewed. On iPhone a sync carries an App Attest assertion. It shows that
    something holding the attested key signed the request. It shows no OS or
    patch state. What a renewal does add is the issuer's own checks: the
    journal extends the acknowledged head and replays (§7.2), the account is
    not blocked (§5.5), and on Android the stored attestation serial is not
    on Google's revocation list (§6). A phone compromised after enrollment
    that presents a consistent journal passes all three.
- T3. The genuine app enforces the rules, and on an uncompromised OS its state
  cannot be rolled back while the key lives. §4.1 gives the mechanism: a marker
  of the latest state kept where no backup reaches. Its rules are in §5.10. It
  rests on reading code and documentation. No device has been tested (§10.4).

Not assumed: an attested clock, a hardware counter that resists a compromised
OS, or that every enrolled phone stays uncompromised.

**What "secure" means, per attacker (F4).** Each statement names its scope.
Nothing in this document claims more.

- **An honest user**, with the unmodified app on a phone that is uncompromised
  and in the user's hands.
  - Value the wallet has committed, loaded or received, is not reduced by
    anyone else's action. A payer that turns out to be compromised, evidence
    against a payer, a block of a payer, a key revocation and a policy change
    all leave it as it was (F1). A recovery claim by whoever holds the account
    key does not touch the device (§7.3).
  - The ability to pay or receive can be stopped by someone else in three
    cases, all exceptions in §3.1: an operator key is revoked; a thief of the
    list key forges a block entry; whoever holds the user's account key
    retires the device id. No value is lost in the first two. In the third
    the phone can still unload, but an unload pays the account. A holder
    whose account key someone else holds can then get the balance out only by
    paying receivers that do not hold the entry. A regulatory
    control applied to the user's own account is outside this scope.
  - A payout above the row's own loads is released at `unload_limit` per
    window, on dates other rows cannot change (§8.2). When the pool is short
    it waits for the operator to fund it (§8.4).
  - A payment in flight can be lost. If the receiver refuses a Payment and
    never shows the `Refused` Outcome, the payer's amount is in neither wallet
    until the two phones meet again. The receiver gains nothing by it.
  - The scope excludes three things. Someone who holds the phone and can
    unlock it can pay the balance away, as with cash (§5.9). A phone
    compromised by a third party is a compromised phone, below. The operator's
    ability to pay is outside the protocol (§3 item 2).
  - A platform fault can cost an honest user the balance, or send the user
    online. A key store that loses its latest marker write in a power cut is
    repaired at the next start. One that keeps a later write and loses an
    earlier one stops the wallet for good (§5.10). A commit lost after
    release makes the phone sign twice at one sequence number, which brings a
    fraud hold (§3.1). The forced power-off
    tests of §10.4 decide whether either happens. They have not been run.
- **An ordinary user with the unmodified app** on an unrooted phone, using
  what the platform offers: backup and restore, reinstall, phone-clone tools,
  developer options, the settings app.
  - This user cannot bring back an earlier balance and cannot double spend,
    if the device tests of §10.4 pass. The unmodified app signs one successor
    per state, and the marker stops a restored older state from signing
    (§4.1). The wallet polices itself; no receiver can check it.
  - This user can weaken the time-based controls by setting the clock. §5.4
    states what remains of R7 and R8 then.
  - Where an operator enables recovery insurance, this user can collect up to
    the cap for a balance already spent. That is the insurer's cost (§7.3).
- **A compromised phone**: a modified app, or a compromised OS, on a phone
  that passed attestation at enrollment.
  - It can make the hardware key sign anything, any number of times. It can
    create value that honest receivers accept, and the operator must honour
    that value (F1, F2).
  - Only the regulatory controls the scheme has switched on bound it, and each
    bound is applied by a receiver from inputs the receiver can check: the
    limit in the payer's certificate against the receiver's own tally (R7);
    the certificate's `not_after` against the receiver's own time
    (R8); the receiver's own copy of the block list (R6). None bounds the
    total (§3 item 1).
  - Evidence identifies it only where two of its transitions reach the issuer
    or the chain (§5.3). With R8 off none need ever arrive.
- **A thief of an operator key.** §8.1 states what each stolen key allows.
  §3.1 states what a revocation then does to honest holders.

What each party can check:

- A receiver, offline, from the Payment and its own state: the issuer and
  witness signatures; the certificate against the tier row; the device
  signature; that the Payment answers its own Request; expiry and limits on
  its own time; its own list; its own tally; under B1, the proof. It
  cannot check the payer's marker, whether the payer signed another successor
  of the same state, or whether the payer's app is genuine now. Under B2 it
  cannot check the payer's balance either.
- The issuer, at a sync, from the uploaded journal: that it extends the
  acknowledged head and that its arithmetic replays. It cannot check that no
  other branch exists.
- The chain, at an unload, from the RedeemSplit and the registry row: the
  device signature and the increase in the cumulative total. It verifies no
  balance (§8.2).

### 4.1 The counter or commitment

The owner's mechanism is hardware-backed keys plus a unique counter or
commitment that prevents state reset and double spending (§1). A study on
2026-10-02 looked for every such primitive an ordinary store app can reach,
without OEM access, and had each one attacked. Nothing was run on a device.

Two attackers have to be kept apart.

- **U, an ordinary user with the unmodified app** on an unrooted phone. U uses
  what the platform offers: backup and restore, reinstall, phone-clone tools,
  developer options. "Reset" means bringing back an earlier balance while the
  key still signs.
- **M, a modified app or a compromised OS** on a phone that passed attestation
  at enrollment. M can ask the hardware key to sign anything, any number of
  times.

**Against U, reset can be prevented on both platforms if the device tests of
§10.4 pass.** The wallet keeps a marker of its latest state in a place that no
backup carries. A restored older state then names a marker that is gone.

| Platform | Marker | Why an old one cannot come back | What is open |
|---|---|---|---|
| Android | A marker key in Android Keystore. Each time the wallet signs and commits an object, it creates the next marker, makes the new state durable in a commit whose digest gives that marker its name (§5.10), then deletes the old marker | Keystore keys are in no backup, and uninstall and clear-data delete them (AOSP source) | No device tested. Vendor backup and clone tools are undocumented on this point. A Keystore lookup can fail for reasons other than absence (§5.10) |
| iPhone | A marker item, a random value, in the keychain class that needs a passcode and is never backed up. The same order as on Android: create the next marker, make the new state durable naming it, delete the old one | Apple documents that items of this class are not backed up and never move to another device | Removing the passcode deletes the marker, and the balance cannot be used again (§3.1). The class is readable only while the phone is unlocked, so a read that fails is not a missing marker (§5.10). Not device-tested |
| HarmonyOS NEXT | Not designed here (§10.2). Backup and clone are opt-in per app; its key store has no use limit or counter | | |

The marker rules are in §5.10: which signed objects it covers, the order of the
steps, what the wallet does after an interrupted step, and how it tells a marker
that cannot be read from one that is gone. On Android the app also opts out of
backup and device transfer and declares that a package rollback keeps its
data.
The results that bear on trust are these.

- Without a marker, a long-lived key can be reset by an ordinary user. On
  stock Android, developer options allow a package rollback that restores an
  app's data and leaves Keystore alone (source reading; not run). Xiaomi
  and Honor document restore of third-party app data onto the same phone, and
  Meizu a local backup of app data. Huawei documents it too and excludes what
  it calls financial application data; how an app is classed is not
  documented.
  On iPhone, Apple documents that device-only keychain items return when a
  backup is restored to the same phone. The Secure Enclave key is kept as such
  an item; developer reports say such a key is gone or no longer signs after a
  restore, and this is untested. One desktop tool is reported to restore one
  app's files without touching the keychain.
- The marker holds only if the key store makes a marker durable when it says
  it has, and keeps a deleted one deleted. Neither is checked. If a power cut
  can undo a creation, recovery repairs it as long as the key store lost only
  its latest writes (§5.10). If it kept a later write and lost an earlier one,
  an honest user loses the wallet. If it can undo a
  deletion, an old marker is alive for a restore. The forced power-off tests
  of §10.4 decide it.
- The marker is the paying app policing itself. A receiver cannot check it. It
  does nothing against M.

**Against M, nothing on a stock phone prevents a double spend.**

- The secure hardware is a signer with no memory of what it signed. M copies
  the wallet's files, pays one receiver, puts the copy back and pays another.
  The second receiver sees exactly what an honest payment looks like, so any
  rule that accepts honest payments accepts this one. No cryptographic
  construction changes that. Signatures that reveal the key when used twice
  need the key outside the hardware, where M already holds it. A bond deters
  only where the gain is bounded, and here it is not (§3).
- Every published design found that prevents double spending of value the
  receiver can spend again offline runs wallet code in a secure element or a
  trusted application. Every design found that keeps such value transferable
  without that hardware detects or scores risk afterwards. The claim is
  limited to transferable value because one cited scheme falls outside it.
  PulpoPay (ePrint 2026/2199) claims to prevent double spending for an
  offline receiver and names no secure hardware. As one automated pass read
  the full text, the payer, while online, has each coin issued to a named
  receiver's key. That receiver checks the coin against its own records, and
  it cannot spend the coin again offline. Only the abstract was read a second
  time; it gives a timing for "a payment with an offline receiver" and
  neither states nor contradicts that mechanism. If the reading is right, the
  scheme fits the argument above: there is no second receiver who would
  accept the same coin. R1 and R4 exclude it, because the payer must be
  online and the value does not move on.
- **iPhone assertion counter.** Apple documents it as the number of assertions
  a key has signed and asks servers to check that it grows. Apple does not say
  where it is kept. One third-party source, for one old iPad model, shows the
  assertion data, counter included, being put together by an OS service, with
  the enclave signing what it is given. If that holds on current iPhones, M
  sets any counter it likes. Not tested on a current device.
- **Android one-use keys.** The interface, the reference code and the one
  measurement do not settle whether any phone enforces a limit of one in
  hardware.
  - The KeyMint interface says a use limit above one is enforced in software.
    A limit of one is enforced by the secure hardware only if that
    implementation can do so with its secure storage; the attestation then
    lists the limit at the hardware's own security level.
  - AOSP publishes two reference implementations, and they differ. The Rust
    reference (`platform/system/keymint`, `common/src/tag.rs` at commit
    `fda4e68d`) enforces a limit of one itself when secure storage is
    available and leaves it to software otherwise. That branch has no
    StrongBox exclusion. The same code rejects two other tags for StrongBox,
    `MAX_USES_PER_BOOT` and `ROLLBACK_RESISTANCE`, as tags that need per-key
    storage. The JavaCard applet that AOSP publishes for StrongBox
    (`platform/external/libese`, `ready_se/google/keymint/KM300`,
    `KMKeyParameters.java`) lists the use limit among the tags left to
    software. AOSP's Trusty reference for the TEE enforces a limit of one
    where it has secure deletion storage. All three are source readings. So
    AOSP's own StrongBox applet does not enforce a limit of one, and neither
    the interface nor the Rust reference stops a vendor's StrongBox from
    enforcing it. None of this says what a given phone does.
  - The one measurement is the repo's. On a Pixel 6, a StrongBox key with a
    limit of one was attested with the limit in the software list (§2.2).
    That is one phone and one security level. No TEE key limited to one use
    has been generated on any phone, and no StrongBox other than the Pixel
    6's has been tried.
  - Even where the hardware enforces the limit, reading the reference code
    shows two ways a compromised OS gets more than one signature from such a
    key: it can begin two operations before finishing either, and after a
    security-patch update it can obtain a second usable copy of the key.
    Whether a vendor's TEE or StrongBox firmware allows either is not tested.
    The tests of §10.4 therefore cover both security levels on each vendor,
    with both attacks. The best a chain of one-use keys gives, unless those
    tests show otherwise, is two conflicting hardware signatures, which is
    evidence and not prevention.
- **Tencent SOTER**, on phones sold in mainland China, signs with a counter
  kept in the TEE. The counter is shared by every app on the phone, so it has
  gaps and cannot show that a state had one successor.
- §3 item 1 therefore stands. A compromised phone is bounded only by the
  regulatory controls the scheme has switched on, each applied by a receiver
  (§4). Evidence identifies it only where its transitions reach the issuer or
  the chain. The backstop pays for what it creates; it bounds nothing.

**Co-signing with App Attest (§12 item 12).** Requiring an App Attest assertion
on every transition, with the counter equal to the sequence number, would let a
receiver check for a reset itself instead of trusting the payer's app. It adds
nothing against U beyond the marker above, and nothing against M if the counter
is set by the OS. Its cost is that any lost assertion, and any invalidation of
the key by Apple, strands the balance; Apple engineers confirm a field defect
that leaves such keys permanently invalid on some phones. A stranded balance is
what F3 rules out. Recommended: do not co-sign. Revisit only if a test on a
jailbroken or research iPhone shows the counter cannot be forged, and then only
a pool of iPhones alone could claim prevention.

**What "prevent double spend" can mean on stock phones** is therefore: an
honest user cannot do it by accident or with platform tools, if the device
tests pass; and a compromised phone can do it, is bounded only by the
switched-on regulatory controls, and is identified only where its transitions
meet. Prevention against a compromised phone needs one of two things. One is a
counter or one-use key that the secure hardware enforces against the OS, with
each hop proving its use (§2.1 item 3); no target phone is shown to have one,
and the tests of §10.4 would show it. The other is hardware that runs wallet
logic, which the owner has set aside for now (§12 item 31).

## 5. Protocol (Layer A)

Canonical Norito. Every signed preimage is `tag ‖ scheme id ‖ bare payload`,
fixed-width scalars, explicit presence bytes for optional fields. All wire
objects are new. Sizes below are estimates extrapolated from the existing
attested suite (certificate 296 B, transition 240 B, payment 570 B, framed);
none is measured.

### 5.1 Objects

Sizes in this section are estimates. None is measured. The table at the end
says who signs each object and where it travels. The marker step of §5.10
applies to every object in that table that a wallet signs and commits.

- **Device id** = `H(tag ‖ scheme id ‖ device public key)`. Every verifier and
  the registry recompute it from the certified key. The issuer never enrolls
  one device key twice: renewal raises the serial under the same device id,
  and a retired id is never re-enrolled.
- **Scheme descriptor**: the root signature over the on-chain scheme cell's
  (epoch, digest). Chain, asset, scale, pool id, role-separated keys, the tier
  table, scheme-wide ceilings, admission policy. It also holds:
  - the root public key and `next_root_digest`, the hash of the root key that
    will follow it (§5.11);
  - `rules_floor` and `rules_max`, the lowest and highest rules version a
    wallet may sign under (§5.11);
  - `list_epoch` (§5.5);
  - the fee schedules and the table of beneficiary accounts (§5.8);
  - how long a Request is valid for a new payment (§5.2);
  - the recovery parameters (§7.3);
  - the scheme status: open, or closed to loads (§5.11).

  A wallet receives the whole descriptor from the issuer at enrollment and at
  each sync. The descriptor is not sent peer to peer.
- **Tier row**: one row of the tier table. It holds the highest terms a
  certificate of that tier may carry, one value for each term of the device
  certificate below; the fee schedule (§5.8); the user-authentication mode
  (§5.9); and the ledger parameters of §8.2 and §5.8.
- **Root-signed notices**: parts of the descriptor that are signed one by one
  so that they can be handed peer to peer when one side is behind. Each
  carries the scheme id, the descriptor epoch at which it was made, its kind,
  its body and the root signature. About 0.11 KB plus the body. Kinds:
  - issuer key certificate: a certificate, voucher, list or witness key, its
    role and its key id;
  - key revocation;
  - tier row: one row, with `effective_from_ms` (§5.11);
  - rules range: `rules_floor`, `rules_max`, `effective_from_ms` (§5.11);
  - root succession: the next root key, signed by the old root key and by the
    new one (§5.11);
  - scheme status (§5.11).

  A notice with a newer epoch replaces an older notice of the same kind and
  subject. A wallet that holds an older set is looser than the scheme until it
  receives the newer one; §5.11 says how notices travel.
- **Key revocation.** Revoking a certificate key voids every certificate under
  it; revoking witness keys voids every receipt left below its quorum.
  Affected phones can neither pay nor request until they sync. Their balances
  are not changed, they can still unload, and value that other wallets
  received from them before is not changed. This forced sync is not a
  regulatory control. It is an exception to the rule that nothing forces a
  holder online except a regulatory control the scheme has switched on. It
  applies only on key compromise, and whether it stays is the owner's decision
  (§3.1). A planned key change uses no revocation and forces nothing (§5.11).
  The regulatory controls that can force a sync are listed in §5.4.
- **Device certificate** (about 355 B): device key, serial,
  `descriptor_epoch`, `not_before`, send expiry (`Lease` ending at
  `not_after`, or `Never`), optional `receive_not_after`, `max_payment`, day
  and month limits (`Limit` or `Unlimited`), per-counterparty cap (`Cap` or
  `None`), `opening_day` and `opening_month`, `expiry_grace`,
  `window_future_tolerance`, `clock_regress_tolerance`, reboot policy,
  optional fee policy (id, rate, fixed part and ceiling; about 70 B, §5.8),
  platform, issuer key id. No account field. Peers ignore the platform field.
  - `descriptor_epoch` is the epoch of the descriptor the issuer held when it
    signed. It tells a verifier which tier row was in force at issuance.
  - `opening_day` and `opening_month` are amounts already counted against the
    day and the month that contain `not_before`. They exist so that a new
    certificate never gives the same account a second allowance in a window
    (§5.4).
  - §5.4 says what each tolerance is for.
- **Registration receipt** (about 150 B with one witness, about 67 B per extra
  witness): witness signatures over (scheme id, device id, registered tier,
  registration height); valid while at least k are under unrevoked keys. Peers
  take the tier from the receipt, not the certificate. To each term they apply
  the stricter of the certificate's value and the newest root-signed row they
  hold for that tier (§5.11). They refuse the certificate outright only when
  that row has not changed since the certificate's `descriptor_epoch` and the
  certificate exceeds it, because then the issuer key signed terms the scheme
  did not allow. Every Request and Payment carries certificate and receipt.
- **Registration authorization** (issuer-signed, never sent to peers): scheme
  id, device id, account id, tier, certificate serial. The registry requires
  the registering transaction's authority to equal that account.
- **Voucher** (voucher-key signed, issuer to wallet only; about 0.3 KB):
  scheme id, voucher id, device id, load id, amount, ledger transaction hash,
  `issued_at_ms`, voucher key id. The field list follows the existing suite's
  voucher
  (`IrohaSwift/Sources/IrohaSwift/KagemushaAttested/KagemushaAttestedModels.swift:685-693`).
  §7.1 says how it is issued.
- **Transition** (about 315 B): rules version, device id, seq, previous digest,
  kind, amount, `fee`, `fee_policy_id`, subject, counterparty,
  `device_time_ms`, anchor flag, clock-reset flag, cumulative `cum_out_after`,
  cumulative `cum_refund_after`, cumulative `cum_fee_after`, gross-sent and
  refunded counters for the day and the month of `device_time_ms`, certificate
  digest. Kinds: Bootstrap (seq 0 only), MintFold, SendSplit, ReceiveFold,
  RedeemSplit, RefundFold, Recertify, Migrate, MigrateFold. For RedeemSplit,
  `amount` is the increment and `redeemed_total_after` is a separate field.
  - The rules version is the first field, and its place and width never change
    (§5.11).
  - `fee` is zero where the payer's certificate holds no fee policy.
    `fee_policy_id` is the digest of the fee policy record the fee was
    computed under, which is the one in the payer's certificate. Settlement
    is judged under that record however late the SendSplit arrives (§5.8).
    `cum_fee_after` is gross, like `cum_out_after`.
  - For a SendSplit, the subject is the digest of the Request and the
    counterparty is the receiver's device id.
  - `device_time_ms` is the signer's own time, or for a SendSplit under a
    time-dependent control the effective time (§5.4). Day and month are the
    UTC day and UTC calendar month of that value, so the counters need no
    index field.
- **Request** (receiver to payer; about 0.7 KB with one witness): `rules_lo`
  and `rules_hi`, the lowest and highest rules version the receiver can judge
  (§5.11); receiver certificate and receipt; nonce; amount (or zero plus a
  maximum); `created_at_ms`, the receiver's own time (§5.4); anchor flag;
  clock-reset mark (§5.4); what the receiver holds (the highest notice epoch,
  `list_epoch` and list counter, root succession number); optional notices and
  an optional list segment. The receiver's device key signs it, so that nobody
  can ask for payment in another device's name. It is single-use.
- **Payment** (payer to receiver; about 0.96 KB with one witness): payer
  certificate and receipt, the signed SendSplit, what the payer holds (as in
  the Request), optional notices and an optional list segment. Only the
  SendSplit is signed. The repo's QR framing carries up to 1,024 B in four
  data frames (§5.6), so the Payment stays at about 7 frames only while it is
  at most 1,024 B; a second witness (67 B) passes that and adds two frames.
  Computed, not measured.
- **Outcome** (receiver to payer; about 0.2 KB): payment id, verdict
  (`Credited` or `Refused`), a reason code, the receiver's device id,
  optional notices and an optional list segment outside the signed part. The
  receiver's device key signs it. The reason code tells the payer's app what
  to show: time, limit, expiry, block, version, stale Request.
- **Signatures per payment.** The receiver's device key signs the Request, the
  ReceiveFold and the Outcome; the payer's signs the SendSplit, and the
  RefundFold after a refusal. Received value exists only as a signed
  ReceiveFold, so there is no mode in which the receiver does not sign. A
  receive costs the receiver three hardware signatures and the payer one.
  Signing time on a phone is not measured.
- **Block list**: signed segments of entries `(device id, flags,
  dead_through_serial)`, flags `receive_blocked` and `send_blocked`. About
  40 B per entry and about 0.1 KB per segment for its header and signature.
  An entry covers every certificate of that device id with a serial at or
  below `dead_through_serial`; a certificate with a higher serial is not
  blocked by it. Entries are merge-only: the higher serial and the union of
  flags win. §5.5 has the rules.
- **Issuer responses** (sync acknowledgement, countersigned Migrate): §7.
  Each carries issuer time and covers a nonce the device made in this boot,
  which is what an anchor needs (§5.4).

| Object | Signed by | Size (estimate) | Travels |
|---|---|---|---|
| Scheme descriptor | root key | not estimated | issuer to wallet |
| Root-signed notice | root key | 0.11 KB plus body | issuer to wallet; peer to peer |
| Device certificate | issuer certificate key | 355 B | in every Request and Payment |
| Registration receipt | k of n witness keys | 150 B, plus 67 B per extra witness | in every Request and Payment |
| Registration authorization | issuer | not estimated | issuer to account to ledger |
| Voucher | voucher key | 0.3 KB | issuer to wallet |
| Transition | the wallet's device key | 315 B | SendSplit in the Payment; every kind to the issuer at sync; RedeemSplit, fee settlement and evidence to the ledger |
| Request | receiver's device key | 0.7 KB | receiver to payer |
| Payment | the SendSplit inside it, by the payer's device key | 0.96 KB | payer to receiver |
| Outcome | receiver's device key | 0.2 KB | receiver to payer |
| Block-list segment | list key | 0.1 KB plus 40 B per entry | issuer to wallet; peer to peer |

One exchange is about 1.9 KB in all (0.7 + 0.96 + 0.2) before framing, with
one witness and nothing optional attached. A notice or list segment carried in
a peer message counts against that message's size bound (§5.6). Under B1 the
Payment also carries the proof: about 7.5 KB (6,528 B plus 0.96 KB; §2 and
§5.6 round these to 0.9 KB and 7.4 KB) against the repo's 7,552 B gate,
if per-hop verification does not enlarge the transport proof (§2).

### 5.2 Payment exchange

One rule covers every object a wallet signs and commits: a Request, a
SendSplit, a ReceiveFold with its Outcome, a `Refused` Outcome, a RefundFold,
and every other transition of §5.1. The steps run in this order.

1. The wallet is ready (§5.10): the marker of the journal head is present and
   no other marker exists.
2. Create the marker for the new commit.
3. Sign in memory.
4. Commit: one durable transaction holds the object, its signature and its
   state change.
5. Delete the previous marker and confirm that it is absent.
6. Release the object.

Steps 2 and 3 do not depend on each other and may run at the same time.
Durable means SQLite `synchronous=FULL`, and on Apple platforms `fullfsync` and
`checkpoint_fullfsync`. Before step 4 a signature exists only in memory: it is
never logged, stored or passed to another process. If step 5 cannot be
confirmed, step 6 does not happen. The object stays committed and unreleased
until recovery finishes the deletion (§5.10). Step 5 is there because a
released object whose previous marker still exists can be undone by putting
the earlier files back.

A signature that changes no wallet state takes no marker step, for example the
device key's signature over the issuer's nonce in a sync. It is made only when
the wallet is ready. The one exception is the request for the repair of §7.2,
which an inconsistent wallet signs with its device key.

A Payment is authentic for a receiver when three things hold: its certificate
and receipt verify under issuer and witness keys the receiver knows, revoked
or not; the device id recomputes from the certified key; and the device
signature verifies. Every other check in step 4 is a judgment. A failed
judgment on an authentic Payment addressed to an own Request on record is
answered with a signed refusal, so that the payer can refund.

1. **Request** (receiver). Receiver certificate and receipt, nonce, amount (or
   zero plus a maximum), `created_at_ms`, `rules_lo` and `rules_hi`, anchor
   flag, clock-reset mark, what the receiver holds, optional notices and an
   optional list segment (§5.1). `created_at_ms` is the receiver's own time
   (§5.4). The Request is signed and committed under the rule above before it
   is displayed, and it is single-use.
   - A wallet creates no Request while its requesting is suspended (§7.1
     lists the cases). The clock-reset state (§5.4) is not one of them: a
     wallet in that state still creates a Request and sets the clock-reset
     flag.
   - The Request is open for a new payment for a few minutes of monotonic
     time, in the boot that created it. After that, or after a reboot, it is
     closed.
   - The receiver keeps every Request on record, open or closed, with any
     Outcome it stored, for as long as the wallet exists. This is for the
     payer: a payment or a refusal that did not cross can still be settled
     whenever the two phones meet again. The cost is storage, about 0.3 KB per
     Request (estimate, not measured). §12 asks whether to cap it.
2. **Pre-check** (payer). The payer applies the rules the receiver will apply,
   on the payer's own clock, list and counters. If any check fails, nothing is
   signed and both apps show the reason; on a time failure they show both
   dates.
   - The Request's certificate and receipt verify under unrevoked issuer and
     witness keys, the device id recomputes, the certificate is within its
     tier row, and the Request's signature verifies under the receiver's
     device key.
   - The rules version the payer would sign under lies between the Request's
     `rules_lo` and `rules_hi` (§5.11 defines the version rules). This check
     is there so that a payer is never debited by a transition the receiver
     cannot judge.
   - If the Request carries the clock-reset mark and the payer's certificate
     carries a lease or a limit, the payer refuses. A payer whose certificate
     carries neither pays.
   - The payer's own sending is not suspended (§7.1 lists the cases).
   - A payer whose certificate carries no time-dependent control does not use
     the Request's time (§5.4). It signs its own time, and of the time checks
     below it applies only the `receive_not_after` check. Where steps 3 and 4
     name `t_eff`, its signed time is its own time, which may be below the
     Request's `created_at_ms`.
   - For every other payer, let `t_eff = max(Request created_at_ms, the
     payer's own time)`. §5.4 defines own time. It is never below the
     certificate's `not_before` or the wallet's last signed `device_time_ms`,
     so `t_eff` is never below them either.
   - If the payer's certificate carries a time-dependent control (§5.4), the
     Request is not dated more than `window_future_tolerance` ahead of the
     payer's clock reading, and `t_eff ≤ created_at_ms +
     window_future_tolerance`. Without the first bound a receiver with a
     wrong clock would move the payer's signed time forward. A payer whose
     certificate carries none makes neither check.
   - Expiry: `Never`, or `t_eff ≤ not_after + expiry_grace`.
   - If the certificate carries limits: the day and month indices are those
     of `t_eff`, and the counters for those indices stay within the limits.
   - The receiver is not `receive_blocked` in the payer's list. The receiver's
     `receive_not_after`, if set, is not earlier than `t_eff`.
   - The amount matches the Request and is within `max_payment`. The fee is
     the one §5.8 requires.

   A receiver with a newer list or a different clock can still refuse. Outcome
   and RefundFold handle that.
3. **Sign and commit** (payer). The SendSplit carries `device_time_ms = t_eff`
   if the payer's certificate carries a time-dependent control, and the
   payer's own time if it carries none (§5.4). It also carries the Request as
   subject, the receiver's device id as counterparty, and the
   fee with the fee policy it was computed under (§5.8). The pre-check is
   re-evaluated immediately before the commit. The commit fails unless
   `(device id, seq)` is new, the previous digest is still the head, and no
   SendSplit exists for this Request. If the commit reports an error, the
   wallet re-reads the journal and treats the transition as committed only if
   the row at that seq has the same digest. Any other row means the commit
   failed: the signature is discarded and nothing is released. A failed
   signing leaves the wallet unchanged. The payer is debited at the commit.
   The payment id is the digest of the committed SendSplit; the Outcome, the
   ReceiveFold and the RefundFold each name it. The Payment is released after
   the previous marker is confirmed absent. Under B2 no prepared state exists;
   under B1 see §9.
4. **Receive.** The receiver first looks up the payment id. If it has a stored
   Outcome for it, it returns that Outcome and does nothing else. Otherwise it
   checks that the Payment is authentic and that its subject is an own Request
   on record. If either fails, it signs nothing. Then it judges the Payment:
   issuer and witness keys unrevoked; the certificate against its tier row; a
   rules version that the Request listed; itself as counterparty; the Request
   open and not yet decided; the amount; the time checks of §5.4 on the
   SendSplit's `device_time_ms`, which the receiver makes only
   where the payer's certificate carries a time-dependent control; expiry;
   limits and its own tally; the block list; the fee (§5.8); and conflict
   with the last transition seen from this payer.
   - If everything holds, the ReceiveFold, its signature, the verdict and the
     signed `Credited` Outcome commit together. From that commit the payment
     is final for the receiver: nothing that happens later reduces it (§3).
   - If a judgment fails, the verdict and a signed `Refused` Outcome commit
     together. That commit decides the Request.

   Either commit takes the marker step, and the Outcome is released after the
   previous marker is confirmed absent. A `Refused` Outcome takes it too:
   without it a receiver could refuse, let the payer refund, put its earlier
   files back and credit the same payment.
5. **Outcome** (receiver): `Credited` or `Refused`. Presenting the Payment
   again returns the stored Outcome. That lookup comes first, before any other
   check. An authentic Payment with no stored Outcome, addressed to an own
   Request on record that is closed or was already decided for another payment
   id, is answered `Refused`.
6. **RefundFold** (payer): only for a `Refused` Outcome that verifies under the
   counterparty key named in its own SendSplit, once per payment id, and only
   if the payer has stored no Outcome for that payment id before. Never on a
   timeout. Under B1 see §2.1. The payer acts on the first valid Outcome it
   stores for a payment id. A second, different Outcome for the same id changes
   nothing in the payer's wallet; it is evidence against the receiver (§5.3).

A receiver signs an Outcome only for an authentic payment addressed to a
Request it holds on record. Anything else gets no signed object. That includes
a Payment the receiver cannot parse; an unmodified payer never sends one,
because of the version check in step 2.

Presenting again. A payer shows the same Payment as often as needed and never
signs a second SendSplit for one Request. A receiver shows the same Request and
the same stored Outcome as often as needed. Nothing in this section makes an
unmodified wallet sign two different objects for one sequence number or two
different Outcomes for one payment id.

What stays open. The exchange is not atomic (§3 item 4). The payer is debited
at its commit. If the Payment never reaches the receiver, or a `Refused`
Outcome never reaches the payer, the payer stays debited until the two phones
meet again; if they never do, the value is lost. No rule here forces either
side online to settle it, and no offline rule can refund on a timeout without
letting a payer refund a payment that was credited. A relay through the issuer
when both sides sync is an owner decision (§12 item 26).

### 5.3 Evidence

Let Δ(t) be the outflow a transition must add: its amount for SendSplit and
RedeemSplit, zero otherwise. `cum_out_after` is gross and never decreases.

Evidence is publicly verifiable and accepted on-chain from anyone. An
unmodified phone that follows §5.2 can produce evidence against itself through
two causes only.

- A commit that the phone's storage loses after its object was released. If
  the key store kept its own writes of that step, the wallet finds a marker
  its journal does not name, stops (§5.10) and signs nothing more. If the key
  store lost them too, the wallet continues from the earlier head and its next
  object is evidence. The durability rule of §5.2 and a forced power-off test
  (§10.4) guard this.
- A restore that brings back an earlier journal together with a marker that
  journal names. That needs a platform or vendor tool that restores key-store
  entries, or a marker deletion that a power cut undid (§5.10). The rollback
  script of §10.4 must rule both out per device (T3).

The table lists each evidence line and the path by which an unmodified phone
could produce it.

| Evidence | Path on an unmodified phone |
|---|---|
| Two different digests at one `(device id, seq)` | The two causes only. A second signature over the same bytes has the same digest and is not evidence |
| Adjacent transitions whose previous-digest link is broken, or where `cum_out_b ≠ cum_out_a + Δ(b)`; the same for `cum_fee_after` (§5.8) | The two causes only |
| `seq_a < seq_b` where `cum_out_b < cum_out_a + Δ(b)` | The two causes only |
| `seq_a < seq_b` with a lower certificate serial at b | The two causes only. A wallet moves to a higher serial by Recertify and never goes back |
| `seq_a < seq_b` under one certificate with a lower `device_time_ms` at b | The two causes only. §5.4 keeps every signed time at or above the last signed one |
| Within one certificate and one day or month index, a gross-sent or refunded counter that regresses | The two causes only |
| A transition naming certificate serial N above a device-signed `Recertify` to a higher serial | The two causes only. A head declared in a sync request is not evidence, and neither is an issuer-signed checkpoint alone, so a lost renewal response cannot accuse an honest wallet |
| Any transition above a `Migrate` of the same device id | The two causes only. A wallet signs nothing after it commits a Migrate, whether or not the issuer answers |
| A Bootstrap at any seq other than 0 | None |
| `Credited` and `Refused` Outcome for one payment id; a ReceiveFold and a `Refused` Outcome for one payment id | The two causes only. The stored-Outcome lookup of §5.2 comes before every other check, and each Outcome commit takes the marker step |
| A certificate and receipt whose device id has no registry row | None by the phone. It shows misuse of the certificate key and of the witness quorum (§8.1) |
| A MintFold whose voucher matches no on-chain load | None by the phone. It shows misuse of the voucher key (§8.1). The phone that folded the voucher cannot check the chain offline |

A ReceiveFold after an outflow of 100 carries `cum_out = 100` and Δ = 0, so it
is not evidence. Conformance vectors cover every kind in both positions.

**Padded forks.** Under B2, and under B1 with the lean relation, two receivers'
transitions are sure to be evidence against a payer only when they sit at the
same or adjacent sequence numbers. A compromised phone can pad one branch with
a transition nobody sees, and it chooses the cumulative counters, so no such
pair need exist. Evidence then appears only when the device's own journal
reaches the issuer: at a renewal if R8 is on, and never if it is off. Under B1
with the per-hop relation the counters are proven. Padding then does not hide a
fork by a phone with one enrolled instance and no accomplice. It still hides a
fork whose other branch is paid to a second instance the attacker controls
(§2.1 item 2). Evidence therefore identifies a compromised phone in some cases.
It does not bound what that phone creates.

**What evidence does.** Accepted evidence does nothing to value that other
wallets received from the accused device. A received payment is final when the
receiver's wallet commits it (§5.2 step 4), and no later evidence against the
payer reduces it. Accepted evidence places a fraud hold on the accused device's
row. The row takes no Load and records no new claim, its recorded claims are
not paid while the hold stands, and holders of the block entry refuse the
device (§5.5, §8.2). §3.1 keeps this as an exception. Whether it stays, and
the governed reinstatement that must exist beside it, is an owner decision
(§12 items 11 and 36).
That decision has to cover an honest phone accused through one of the
two causes above: a hold on its row strands a balance that no regulatory
control touched. The last two lines of the table show a fault in an issuer-side
key, not in the phone; the decision has to say what they do to the phone that
holds the object.

### 5.4 Time, limits, expiry

Three controls depend on time: limits (R7), expiry with its reboot policy
(R8), and receive freshness (R6, §5.5). No phone gives an app an attested
clock (§3 item 5). Each rule below therefore names who checks it and with
what input.

A certificate carries a time-dependent control if it has a `Lease`, a day or
month `Limit`, or a per-counterparty `Cap`. Counting the per-counterparty cap
as a limit in the sense of R7 is this document's reading. A scheme that
switches none of these on issues certificates with none. For a wallet under
such a certificate no rule in this section stops a payment.

**Clock, floor and own time**

- **Anchor.** A persisted record of issuer time, a suspend-inclusive monotonic
  reading, and a boot identity. It is accepted only from a direct issuer
  exchange whose signed response covers a nonce the device generated in this
  boot. Issuer-signed timestamps that arrive through a peer raise "last issuer
  time" and never create an anchor. Clocks: Android `elapsedRealtimeNanos`,
  Apple `mach_continuous_time`. Boot identity: Android `BOOT_COUNT`; iOS has
  none documented, so the shell treats any doubt as a reboot.
- **Clock reading.** While anchored it is issuer time plus monotonic elapsed
  time, and the user's date setting has no effect on it. The app persists the
  latest anchored time whenever it runs. After a reboot the wallet is
  unanchored and the clock reading is the wall clock, which the user can set.
- **Floor.** The highest of: last issuer time; the latest anchored time
  persisted; the time in the last object the wallet signed and committed (a
  transition's `device_time_ms` or a Request's `created_at_ms`); and the
  certificate's `not_before`. Real time is at or above the floor unless the
  wallet signed something while a clock was ahead.
- **Own time.** What the wallet uses as "now":
  1. Clock reading at or above the floor: own time is the clock reading.
  2. Clock reading behind the floor by no more than `clock_regress_tolerance`
     (default 24 h): own time is the floor, held until the clock reading
     passes it. Nothing is suspended.
  3. Clock reading further behind: the wallet is in the clock-reset state
     (below). Own time is the floor.
- A wallet never signs a time below its floor. Every Request carries the
  receiver's own time as `created_at_ms`. Every transition carries the
  signer's own time as `device_time_ms`, except a SendSplit under a
  time-dependent control, which carries the effective time defined next. The
  rule is checked by the wallet's own app from its journal, by the issuer
  when it replays the journal, and by anyone who holds two transitions of one
  device under one certificate with the later one carrying the lower time
  (§5.3).
- **Date check.** When the wallet is unanchored and the clock reading is more
  than 30 days (proposed) ahead of the floor, the app shows the date and asks
  the user to confirm it before the wallet signs anything. This is for the
  user who sets a wrong year after a dead battery: one object signed at that
  date would lift the floor and put the wallet in the clock-reset state once
  the date is corrected. It is a prompt, not a control.

**Effective time for a payment**

Let `r` be the Request's `created_at_ms`, `c` the payer's clock reading, `p`
the payer's own time, and `W` the payer's `window_future_tolerance`.

A payer whose certificate carries a time-dependent control applies these
checks in its pre-check and again immediately before the commit (§5.2). If
any fails, nothing is signed, nothing is debited, and both apps show both
dates.

1. The payer is not in the clock-reset state, and the Request is not marked
   clock-reset.
2. `r ≤ c + W`: the receiver's time is at most `W` ahead of the payer's
   clock reading.
3. `p ≤ r + W`: the payer's own time is at most `W` ahead of the receiver's.
4. `t_eff = max(r, p)`. Own time already includes `not_before`, so this is
   the largest of the Request's time, the payer's device time and the
   certificate's `not_before`.
5. Expiry: `Never`, or `t_eff ≤ not_after + expiry_grace`.
6. Windows: the day and month are those of `t_eff`. After this payment
   `gross_sent − refunded ≤ limit` in both.
7. The receiver's `receive_not_after`, if set, is not earlier than `t_eff`.
8. The SendSplit is signed with `device_time_ms = t_eff`. That value becomes
   the payer's floor.

What the rules are for.

- Rules 4, 6 and 8 put the payment in the window of the receiver's date
  whenever the receiver's time is later than the payer's. A payer who sets
  its own date back therefore gains nothing from a receiver whose clock is
  right.
- Rule 2 protects the payer. Without it a receiver with a wrong or false date
  could push the payer's floor far into the future, which would use up the
  payer's future windows and bring its lease to an early end. The rule
  compares against the clock reading and not against own time, so that
  repeated Requests cannot walk the floor forward step by step: whatever
  receivers do, the payer's floor is never more than `W` ahead of the payer's
  own clock.
- Rule 3 stops a payer from spending the allowance of a window that the
  receiver's clock says is still more than `W` away.

What a wrong receiver clock can cost an honest payer whose own clock is
right: its floor moves ahead of real time by at most `W` in all. It may then
count payments in the next window up to `W` early and reach its lease end up
to `W` early. It loses no value, is forced to no sync, and still passes rule
3 with every receiver whose clock is right.

A payer whose certificate carries no time-dependent control makes none of
these checks and does not use the Request's time. It signs its own time. The
only time rule it applies is rule 7, against the later of its own time and
`r`. This is so that a receiver's wrong clock cannot move the floor of a
payer that has nothing to judge on time.

**What the receiver checks.** Input: the signed SendSplit, the payer's
issuer-signed certificate, the tier row the receiver holds, and the
receiver's own committed Request. Let `T` be the SendSplit's
`device_time_ms`, and `W` the tolerance in the payer's certificate. If the
payer's certificate carries a time-dependent control:

- the receiver's own Request was not marked clock-reset;
- `T ≥ r`: the payer signed at or after the receiver's own time;
- `T ≤ r + W`;
- `T ≥ not_before`;
- `Never`, or `T ≤ not_after + expiry_grace`;
- the SendSplit's day and month counters are those of `T`, include this
  amount, and are within the limits;
- the receiver's own tally (below) stays within its cap.

A failure is answered `Refused`, and the payer refunds when it has the
Outcome (§5.2). With `T ≥ r` and the expiry check, a receiver whose clock is
right never credits a payment under a certificate whose lease, with its
grace, has ended. If the payer's certificate carries no time-dependent
control, the receiver makes no time check.

**Tolerances**

| Field | Default | What is compared | What it is for |
|---|---|---|---|
| `window_future_tolerance` (`W`) | 2 h, proposed; not derived from a measurement | the Request's time against the payer's clock reading; the payer's own time against the Request's time; the signed time against the Request's time | The largest disagreement between the two wallets' times at which a payment under a time-dependent control is made. Smaller: honest pairs with slightly wrong clocks cannot pay until one clock is set. Larger: one side's wrong clock moves the time the other side signs by more |
| `clock_regress_tolerance` | 24 h | the clock reading against the floor | A clock reading up to this far behind the floor is treated as a small error, such as a wrong time zone, and the floor is used. Further behind, the clock is treated as lost |
| `expiry_grace` | tier value | the effective time against `not_after` | The period after `not_after` in which the wallet still pays while the app tells the user to renew. Payer and receiver add it alike, so it moves the cut-off and adds no slack between them |
| Request validity | a few minutes; descriptor value | monotonic time since the Request was made | A Request is paid only while it is fresh, so its `created_at_ms` is close to the time of payment. It must be shorter than `W` |

**Limits (R7)**

- Day and month are the UTC day and UTC calendar month of the signed time. The
  limit check is `gross_sent − refunded ≤ limit`. A RefundFold adds to the
  refunded counter of a window only when its SendSplit carries the same
  window.
- Fixed windows let a wallet send one limit just before a boundary and one
  just after. A payer may also sign up to `W` ahead of the receiver's time, so
  it can use the next window up to `W` early. It then has no allowance left
  in that window when real time reaches it. The gain is at most one extra
  window's allowance, once.
- **Receiver tally.** For each payer device id the receiver keeps the sum of
  the amounts it credited in each day and each month of the signed time. It
  refuses a payment that would take a sum above the payer's limit for that
  window, or above the per-counterparty cap where that limit is `Unlimited`.
  Input: the receiver's own journal, and the signed time it has checked
  against its own Request. This is the only limit check that holds against a
  compromised payer, and it holds per receiver. One receiver accepts at most
  one limit per window from one payer, and at most two in one real day or
  month across a boundary.
- The limit subject is the account. The issuer splits the account's limit
  across the certificates of its devices, so that the limits in the live
  certificates of one account never add up to more than the account's limit.
  Input: the registry's binding of device ids to accounts, and the issuer's
  own record of certificates. A share returns to the account when its device
  id is retired and its last certificate can no longer send (§7.2).
- **Opening counters.** A certificate's `opening_day` and `opening_month` are
  where the wallet's counters start for the day and the month that contain
  `not_before`. Other windows start at what the journal already
  holds for them: zero, unless the wallet signed ahead into that window
  (§7.2). The issuer sets the opening counters:
  - at a renewal or a Recertify, from the journal the wallet uploads: the net
    amount sent under every signed time in or after the current day, and the
    same for the month. Amounts the wallet signed under a time ahead of issuer
    time are counted now, so they cannot be spent a second time when that date
    arrives;
  - for a Migrate successor, in the certificate it receives at the renewal
    after its MigrateFold (§7.2): the same figures from its own journal, to
    which the MigrateFold has added the old device's counters;
  - for a device enrolled after another device of the account was retired
    with no journal covering its last period (§7.2), zero. The issuer cannot
    know what the retired device spent, so it does not give that device's
    share out again while its certificate can still send. The new
    certificate carries only a share that was not allocated.

  This is for an ordinary user who would otherwise renew, rotate the key or
  re-enroll to get a second allowance in the same window. The cost falls on an
  honest user who loses a phone: the new phone cannot send the lost phone's
  share until that phone's lease and grace have ended (§7.2). It delays
  sending and forces no sync.
  It is not a bound against a compromised phone, whose counters are its own
  statement.

**Expiry (R8) and receive freshness**

- Send expiry gates sending only. A wallet whose lease and grace have ended
  by its effective time does not pay until it renews. It still receives and
  still unloads.
- `receive_not_after` is a separate optional gate on receiving, used for R6
  freshness (§5.5). A wallet creates no Request once its own time is past its
  `receive_not_after`. A payer refuses a Request whose certificate's
  `receive_not_after` is earlier than the payer's effective time (rule 7).
  Who can enforce it: the payer's unmodified app, with its own time, which is
  never below the payer's floor.

**Reboot policy** (certificate field)

- `wall_clock`: after a reboot the wallet uses the wall clock as its clock
  reading, and what it signs is flagged unanchored. Recommended default.
- `require_anchor`: after every reboot, chosen or not, the wallet does not pay
  under this certificate until it reaches the issuer. Requesting continues. An
  opt-in tier value that governance acknowledges as an online requirement
  after each reboot. It makes the payer's clock reading one the user cannot
  set. With it, R7 and R8 hold against an ordinary user of the unmodified app
  whatever the receiver's clock says, because rule 3 refuses a Request dated
  more than `W` behind an anchored time. That rests on the anchor tests below
  passing on the device.
- The anchor flag is self-declared. It is audit data for the issuer, not a
  control.

**Clock-reset state.** The clock reading is behind the floor by more than
`clock_regress_tolerance`. The wallet has no usable clock. It reaches this
state when a dead battery restores an old date, or when something was signed
while the date was ahead and the date was then corrected.

- Sending under a certificate with no time-dependent control is unaffected.
  The wallet signs the floor as its time and sets the clock-reset flag.
- Sending under a certificate with a time-dependent control is suspended.
- The wallet still creates Requests. Each carries the floor as
  `created_at_ms` and the clock-reset mark. A payer whose certificate carries a
  time-dependent control does not pay it (rule 1); a payer with none does.
- A wallet in this state answers `Refused` to a payment under a certificate
  with a time-dependent control, because it cannot judge the signed time.
  Only a modified payer sends one.
- Loading, unloading and sync continue, and a sync re-anchors.
- The state is evaluated at each use. It ends when the clock reading is again
  no more than the tolerance behind the floor, or when the phone re-anchors.
  If the floor is not ahead of real time, setting the correct date ends it. If
  the floor is ahead of real time, the correct date does not end it: it ends
  when real time comes within the tolerance of the floor, or at a sync.
- At a sync where issuer time is below the floor, the issuer issues a new
  certificate in that sync with the opening counters above, and the wallet
  adopts it with `Recertify`, which carries issuer time. After a Recertify the
  floor is that issuer time. This is the only way a floor comes down, and it
  needs the issuer because no other time is trusted for it.

So the clock-reset state costs something only where a regulatory control
needs time: on the wallet's own sending if its certificate carries one, and
on receiving from payers whose certificates carry one. The mark is
self-declared; a modified receiver can leave it off, and then rules 2 and 3
judge its date like any other.

A floor that is ahead of real time by more than `W` and less than
`clock_regress_tolerance` does not put the wallet in the clock-reset state.
While it lasts, rule 3 stops the wallet paying receivers whose clocks are
right, and rule 2 stops payers whose clocks are right paying its Requests,
where a time-dependent control applies. It ends when real time is within `W`
of the floor: a wait of at most `clock_regress_tolerance` less `W`, with no
sync. Others cannot cause it (rule 2); it follows only from signing while the
wallet's own clock was ahead.

**Worked cases.** A wallet P has a day limit L and policy `wall_clock`. It
spent L on 1 October, has been idle since, and its floor is 1 October. It is
now 5 October, 12:00 UTC, and P has been rebooted. `W` is 2 h and
`clock_regress_tolerance` is 24 h. Apps are unmodified and receivers' clocks
are right unless the row says otherwise.

| # | Case | Outcome | What P can send on 5 October |
|---|---|---|---|
| 1 | P's date is set back to 1 October | P's app refuses (rule 2): the Request is dated 4 days after P's clock reading. Nothing is signed. When P's date is corrected the payment is signed at the receiver's time, in the 5 October window, and P's floor becomes 5 October | L, once the date is right |
| 2 | P's date is set to 2, 3 and 4 October in turn, with a different receiver each time | Refused each time, as in case 1. No earlier window can be used | L, once the date is right |
| 3 | P's date is 1 h behind | Signed at the receiver's time. P's floor moves up to it | L |
| 4 | P's date is set forward to 9 October | P's app refuses (rule 3). Nothing is signed and the floor does not move | nothing until the date is right, then L |
| 5 | P's date is set forward to 9 October, P creates a Request, and the date is then set back to 5 October | The Request lifted the floor to 9 October. P is in the clock-reset state. Sending under the limit is suspended. It resumes at 9 October 10:00, in the 9 October window, or at a sync | nothing |
| 6 | The receiver's date is 3 October; P's is right | P's app refuses (rule 3). Nothing is signed | nothing to this receiver until its date is right |
| 7 | The receiver's date is 3 October and P's date is set to 3 October | Accepted, signed 3 October. P spends the 3 October allowance. Not enforceable: both clocks are wrong | L from 3 October, and L again on 5 October with a receiver whose clock is right |
| 8 | The receiver's date is 9 October; P's is right | P's app refuses (rule 2). The floor does not move | nothing to this receiver |
| 9 | The receiver's date is 1 h ahead; P's is right | Signed at the receiver's time. P's floor is 1 h ahead of real time. No further effect unless a window boundary or the end of the lease falls in that hour | L |
| 10 | Both dates are 9 October | Accepted, signed 9 October. P has spent the 9 October allowance and cannot sign below 9 October. Once P's date is corrected it is in case 5. Not enforceable; no gain over time | L from 9 October |
| 11 | P's policy is `require_anchor` | P does not pay until it reaches the issuer. After that its clock reading cannot be set, so the cases in which P's date is set (1 to 5, 7 and 10) do not arise | nothing until a sync, then L |
| 12 | P was not rebooted since its last sync | P is anchored. Its date setting is not used | L |
| 13 | P's battery died and the clock came back years in the past | Clock-reset state. The user sets the correct date and the state ends. No sync | L, once the date is right |
| 14 | The receiver is in the clock-reset state | Its Request is marked. P does not pay it (rule 1). A payer with no time-dependent control does | nothing to this receiver |
| 15 | P runs a modified app | It signs the receiver's time and whatever counters it likes. The receiver's checks pass. The receiver's own tally holds it to L per window at that receiver. Expiry is still judged at the receiver's time | no bound across receivers |

Month windows behave the same way with the month of the signed time.

**Who can verify what**

| Rule | Checked by | Input | Holds against |
|---|---|---|---|
| The signed time is at or after the receiver's time, and at most `W` after it | the receiver | the SendSplit and the receiver's own Request | any payer |
| The lease, with its grace, has not ended at the signed time | the receiver | the SendSplit and the issuer-signed certificate | any payer, where the receiver's clock is right |
| The payer's counters are within the limits | the payer's unmodified app; the receiver reads the declared counters | the payer's journal; the SendSplit | an ordinary user. Not a compromised payer, whose counters are its own statement |
| One payer does not exceed its limit at one receiver | the receiver | the receiver's own journal | any payer, per receiver |
| The receiver's time is not far ahead of the payer's | the payer's unmodified app | the Request and the payer's own clock | protects the payer; it is not a check on the payer |
| A wallet does not sign below its floor | the wallet's unmodified app; the issuer at sync; anyone holding two transitions (§5.3) | the journal | an ordinary user; a compromised phone only where both transitions are seen |
| A new certificate does not reset an allowance | the issuer | uploaded journals, the registry binding, its own records | an ordinary user |
| One account's devices together stay within the account's limit | the issuer | the registry binding and its own records | an ordinary user. Not limits across accounts |

**What in this section can force a sync.** Each is a regulatory control the
scheme switches on, and none touches value already received.

- Lease expiry (R8): sending stops until the certificate is renewed.
- `require_anchor` (R8, reboot policy): sending stops after a reboot until the
  phone reaches the issuer.
- `receive_not_after` (R6 freshness): requesting stops until the certificate
  is renewed.
- The clock-reset state with the floor ahead of real time, on a wallet whose
  certificate carries a time-dependent control: sending stops until real time
  comes within the tolerance of the floor, or until a sync. It arises only
  because R7 and R8 need a time.

Limits (R7) delay sending to the next window and never force a sync. A
failed tolerance check forces no sync; it is cleared by setting the date. A
wallet under a certificate with no time-dependent control is never forced to
sync by anything in this section. Two rules outside this section also force
a holder online and are not regulatory controls: key revocation (§5.1, §3.1)
and, if the owner allows it, a rules-version floor (§5.11).

**Stated limit.** A phone's clock is expected to keep running while it is
powered off, so that an ordinary reboot leaves it as accurate as its setting;
no primary source was found, and the long power-off test must confirm it per
device. The clock is lost when the battery is exhausted or removed, and the
restored value is platform-specific (Android: never earlier than the system
build date; iOS: undocumented). Two unmodified wallets whose restored clocks
both read behind real time, above their own floors and within `W` of each
other, accept each other's expired certificates with no one having changed a
clock. The error is real time minus the receiver's own time, and it has no
upper limit. Certificate expiry therefore bounds creation only among
receivers whose clocks are right.

Not enforceable offline, and against whom:

- R7 and R8 against a payer and a receiver whose clocks are both wrong in the
  same direction, by accident or because two users set them. Neither wallet
  signs below its own floor, so the pair needs a receiving wallet that has not
  signed or synced since the date they choose. Value that then reaches a
  receiver whose clock is right is judged at that receiver's time and under
  the sender's own limits. Under `require_anchor` the payer's side of this
  needs a modified app.
- Any limit against a compromised payer beyond each receiver's own tally.
  Nothing limits how many receivers it reaches (§3 item 1).
- Expiry against a compromised payer that keeps renewing (§8.3).
- Receive freshness against a payer whose own time is behind the receiver's
  `receive_not_after`, and against a modified payer.
- Limits across accounts not bound to one verified identity.
- The anchor flag and the clock-reset mark against a modified wallet. Both
  are self-declared.

Required tests, per platform: long power-off; repeated restarts; battery
exhaustion, recording the restored clock; a reset that lands above the
floor; reboot on each side of a payment and with an open Request; both sides
unanchored; app killed and relaunched within one boot; long device sleep
while anchored; post-reboot uptime above the stored reading; Android boot
count rewritten over adb; iOS clock set within one boot; time-zone change;
backward clock steps of a minute, 14 hours and 30 days; a fold signed with the
date ahead, then corrected; re-anchor below the last signed time; replayed and
peer-relayed issuer time; a `Never` and `Unlimited` certificate with no cap
sends and requests throughout, with its Requests marked only in the
clock-reset state. Added for the effective time:

- each row of the worked-cases table, with the signed time, the window and the
  floor recorded after each step;
- a Request dated ahead by just under and just over `W`, with the payer's
  floor checked after each;
- twelve payments in a row to receivers each dated `W` ahead of the payer's
  time, to show that rule 2 keeps the floor within `W` of the clock reading;
- a payment just before and just after UTC midnight and a month end, with the
  payer's counters and the receiver's tally recorded;
- opening counters after a renewal, after a Recertify with the floor ahead of
  issuer time, after a Migrate, and after re-enrollment following state loss;
- the date check: a wallet idle for 31 days and unanchored prompts once and
  then pays;
- under `require_anchor` on iPhone, how often the shell reports a reboot when
  none happened, over a week of normal use.

### 5.5 Block list (R6)

R6 is a blacklist of accounts that senders holding it will not pay (§1).
That is the flag `receive_blocked`. The second flag, `send_blocked`, under
which holders of the list refuse payments from a device, is this document's
addition. Fraud holds use it, and a ledger that blocks an account from
sending can use it. The owner has not asked for it (§12).

- **Source of truth.** A new consensus index per asset, account →
  `{send_blocked, receive_blocked}`, with a change counter. The offline list
  is derived from it in both directions. A phone holding a stale list is
  looser than the ledger until it refreshes.
- **Entries.** Consensus expands a blocked account into one entry per device
  id through the registry: `(device id, flags, dead_through_serial)`. The
  issuer may only transcribe that set plus its own fraud holds. An entry
  covers every certificate of that device id with a serial at or below
  `dead_through_serial`. Entries are merge-only: the higher serial and the
  union of flags win.
- **Where the serial comes from.** `dead_through_serial` is the last
  certificate serial anchored in the device's registry row when the block is
  made. The issuer anchors each new serial in the registry row before it
  releases the certificate, and the registry refuses to anchor a serial while
  the account carries any flag. Checked by consensus, from the block index
  and the registry. This is so that an entry covers every certificate that
  exists when the block is made, and no certificate can be issued above the
  entry while the block lasts. A renewal therefore waits for one ledger
  transaction to be final. If the issuer stops between anchoring and
  releasing, the wallet asks again and the issuer releases the same serial or
  anchors the next; a gap in serials is harmless.
- **What holders of an entry do.**
  - A payer refuses a receiver whose certificate is covered by a
    `receive_blocked` entry. Nothing is signed.
  - A receiver answers `Refused` to a payer whose certificate is covered by a
    `send_blocked` entry, and attaches the list segment that holds the entry.
    The payer refunds when it has the Outcome.
  - A wallet that holds an entry covering its own certificate creates no
    Request if the flag is `receive_blocked` and does not pay if it is
    `send_blocked`.

  Who can enforce this: the unmodified app of whoever holds the entry, with
  the signed list segment and the counterparty's certificate as input. A
  modified payer pays whom it likes.
- **Value already received.** An entry acts only on payments not yet made. A
  payment that the receiver's wallet committed before the receiver held the
  entry is final. No entry, with either flag or as a fraud hold, causes the
  receiver's wallet, the issuer or the ledger to reduce, refuse or delay that
  value, whatever happened to the payer afterwards. An entry also changes no
  balance on the blocked device. What the ledger does with a held or retired
  row is in §8.2.
- **Issuer and ledger.** The issuer refuses enrollment, renewal, load ids and
  vouchers for a blocked account. The registry refuses the registration of a
  new device for one, and the anchoring of a serial as above. Whether a
  blocked account may unload is the ledger's rule for that account; the
  offline list does not decide it.
- **Lifting.** An R6 entry is lifted only by a renewal above its serial,
  after the ledger has cleared every flag on the account. The flags of an
  entry lift together: clearing one of two flags on the ledger has no effect
  offline until both are cleared. Fraud holds and retired ids use the maximum
  serial and are final for that id. Reinstatement is therefore never a
  return to the held id. §3.1 requires a governed reinstatement to the bound
  account; §12 item 36 asks what it pays.
- **Freshness.** A payer is never forced online to refresh its list.
  Freshness comes from the receiver's side: with `receive_not_after` set in
  the tier, a blocked account cannot renew, its certificate's
  `receive_not_after` passes, and payers refuse it by their own time (§5.4).
  The longest a block can go unenforced among unmodified payers whose clocks
  are right is then the length of the receive lease. The price is that every
  receiver must renew within that period to keep requesting; this is the one
  forced sync R6 brings. With `receive_not_after` unset, R6 holds only as of
  each payer's last list, and nothing forces anyone online.
- **Distribution.** A separate list-signing key signs the list in segments.
  Each segment carries the scheme id, `list_epoch`, a counter, the issuer
  time at which it was made, and its entries. Lists are ordered by
  `(list_epoch, counter)`. A wallet receives the whole list from the issuer at
  a sync and may merge any authentic segment a peer hands it; merging can
  only tighten. The issuer time in a segment raises "last issuer time"
  (§5.4). The list is permanent and grows by about 40 B per entry (estimate):
  10,000 entries are about 0.4 MB on each phone. A peer message has room for
  at most about 200 entries under R9, so a wallet catches up on a long list
  only at a sync.
- **List key compromise.** A stolen list key creates no value. It can block
  honest devices among wallets that receive its segments. The root raises
  `list_epoch` in the descriptor and certifies a new list key. The first
  segment of the new epoch carries every entry still in force. A wallet keeps
  applying the old epoch's entries until it holds that first segment and then
  drops them. Merge-only entries stop a forged segment lifting a block.

Outside what any list can stop: a new account on the same phone or another;
after a fraud hold, a new device key under the same account; and receiving
through an accomplice. A block entry binds a device id, not a phone.

### 5.6 Transport

The three messages are transport-neutral. Nothing in §5.2 depends on the
carrier. A carrier moves bytes between two phones. It adds no check and removes
none: the wallet that receives a message applies §5.2 to the bytes it received,
whatever carried them.

- **Three transfers.** The Request goes from the receiver to the payer, the
  Payment from the payer to the receiver, the Outcome from the receiver to the
  payer. A received payment is final when the receiver's wallet commits it
  (§5.2 step 4). That does not depend on the Outcome reaching the payer. A
  `Refused` payment is refunded only if the Outcome reaches the payer. A
  Request is single-use, so it cannot be printed.
- **Interrupted transfers.** The payer's wallet commits the SendSplit before
  it releases the Payment (§5.2 step 3). If the carrier then fails, the payer
  is debited and the receiver holds nothing. The payer's wallet keeps the
  committed Payment and presents it again (§9). Presenting it twice is safe:
  the receiver stores one Outcome per payment id and returns it unchanged
  (§5.2 step 5). No carrier makes the exchange atomic. §3 item 4 states what
  an honest payer can lose and when.
- **QR is the baseline.** Of the carriers the repo implements, QR is the only
  one that works between every pair of target phones. It needs a screen and a
  camera on each phone and no entitlement from the platform. An exchange is
  three scans: the payer scans the Request, the receiver scans the Payment,
  the payer scans the Outcome. Aiming the camera selects the peer.
- **Size on QR.** One QR symbol holds at most 2,953 bytes (version 40, lowest
  error correction). The repo's existing framing shows a still code only up to
  about 0.34 KB of payload. Above that it animates 256-byte frames with one
  parity frame per two
  (`IrohaSwift/Sources/IrohaSwift/IrohaPeerQRV1.swift:198-199, 267-293`). Under
  that framing a 0.7 KB Request is about 6 frames, a 0.9 KB Payment about 7,
  a 7.4 KB Payment about 48, and a 0.2 KB Outcome is one still code. At 12 to
  5 frames per second one pass of the Payment takes 0.6 to 1.4 s at 0.9 KB and
  4 to 10 s at 7.4 KB. The repo's widget defaults to 5 frames per second
  (`IrohaSwift/Sources/IrohaSwiftTransferUI/KagemushaWidgets.swift:281`). These
  figures are computed. No scan was timed. How many passes a scan needs, how
  long the receiver takes to aim at the payer's screen, and how dense a still
  code a phone reads from another phone's screen are not measured.
- **NFC.** Two stock phones have no symmetric NFC mode. One phone reads the
  other's card emulation. The table says what the platforms allow. Nothing in
  it is tested here.

  | Payer → receiver | QR | NFC tap |
  |---|---|---|
  | Android → Android | yes | yes |
  | iPhone → Android | yes | yes: the iPhone reads, the Android receiver is the card |
  | Android → iPhone | yes | in the European Economic Area, with Apple's card-emulation entitlement. Elsewhere not with the repo's flow, in which the receiver is the card; it needs a reversed flow in which the Android payer is the card and the iPhone reads |
  | iPhone → iPhone | yes | only in the European Economic Area, with Apple's card-emulation entitlement |

  An Android app can emulate a card and can read one. An iPhone app can read
  an ISO 7816 card on iOS 13 or later. Without a secure-element applet, which
  R2 excludes, an iPhone app can emulate a card only in the European Economic
  Area, on iOS 17.4 or later, for a developer established there, with an
  entitlement from Apple. Apple's separate NFC and Secure Element platform
  covers more countries, not mainland China, and needs an applet and a
  commercial agreement with Apple. Android removed phone-to-phone NFC (Beam)
  in Android 14. The repo's flow makes the receiver the card
  (`IrohaSwift/Sources/IrohaSwiftMobileTransports/IrohaPeerNfcCoreNFCV1.swift:449-453`).
  The reversed flow is not designed.
- **Bluetooth LE connection.** Both platforms let a store app make one. An
  iPhone app can take either role, exchange data over the connection, and
  needs the user's Bluetooth permission. An Android app can scan and connect,
  and can advertise and serve a connection where the phone's Bluetooth chipset
  supports LE advertising; on Android 12 or later it needs three runtime
  permissions. Neither platform asks for Google Play services or an
  entitlement, and neither limits it to a region. One of the two phones takes
  the advertising role, and an iPhone app can take it while it is in the
  foreground. So the platforms allow a connection on every pair that includes
  an iPhone, iPhone to iPhone among them, and on a pair of Android phones
  where at least one supports LE advertising. Three limits apply.
  - It is not implemented. The repo has no Bluetooth carrier.
  - It is not measured. Connection setup time and transfer time for 0.9 KB
    and 7.4 KB are unknown on every pair. That an iPhone and an Android phone
    interoperate follows from the two platforms' documentation and was not
    tested. Which Android target phones support LE advertising was not
    checked. HarmonyOS NEXT was not examined.
  - It needs a way to select the peer. A radio reaches every phone in range.
    If the payer's phone takes a Request from whichever phone answers, another
    phone in range can supply its own Request and be paid. An advertisement
    cannot carry the Request: an iPhone app advertises 28 bytes. One
    arrangement is for the receiver to show a short still code that names the
    connection, and for the Request, the Payment and the Outcome to cross
    over it. The code would have to bind the Request, for example by carrying
    its digest, or another phone could answer in the receiver's place. That
    leaves one scan in place of three. It is a sketch, not a design.
- **Google Nearby Connections.** This is the repo's radio carrier. On Android
  it needs Google Play services, which Huawei phones and mainland-China builds
  are reported to lack. On Apple platforms, Google's README at the revision
  the repo pins lists Wi-Fi LAN as the only supported medium, so an iPhone and
  another phone need a common local network. Whether it connects with no
  network is unverified. The existing flow has both users confirm a
  connection code, which selects the peer. Nearby is optional and not in the
  first cut.
- **Other radio.** The repo's transport description names Multipeer, an Apple
  framework; no code uses it. Wi-Fi Aware was not examined.
- **Existing code.** The repo has one envelope and three carriers in Swift and
  Kotlin (animated QR, NFC over ISO 7816 commands, Nearby), a second QR framing
  in Rust (`specs/qr_stream.md`), and a short description in
  `specs/peer_transport_v1.md:83-96` that matches neither byte layout. Layer A
  reuses one envelope and one QR framing under a new profile code; the
  normative spec says which. The envelope registers one profile today, so a
  Layer A message is refused until its profile is added. No carrier has a
  recorded device measurement.
- **Bystanders.** QR and NFC are in the clear, and so is a Bluetooth LE
  connection unless the wallet encrypts. Of the existing carriers only Nearby
  encrypts. Anyone who films the codes reads both device ids, the amount and
  the payer's totals (§5.7). Encrypting the Payment and the Outcome to a key
  carried in the Request costs about 100 bytes (estimate). The Request itself
  cannot be hidden from someone who can see it. Owner decision (§12).
- **R9 has no unit.** This document reads it as: the Payment is at most 10,000
  bytes in canonical binary form, before framing. No carrier above has a hard
  limit near that size. The bound is a budget for the time two phones are held
  together. A notice or list segment carried in a peer message counts
  against that message's bound.

Which carriers a wallet must support is an owner decision (§12 item 16). The
measurements that decide it are in §10.4.

### 5.7 What the exchange reveals

Privacy is not among R1–R9. Layer A is pseudonymous, not anonymous. This
section lists what each party can learn. Nothing limits what a party keeps: a
wallet cannot make another phone, the issuer or the chain forget.

- **The pseudonym.** A device id is a stable pseudonym. It changes only when
  the wallet enrolls a new key: on Migrate, after an established loss of state
  (§7.2), or on reinstatement. The registry records the succession on Migrate
  only. In the other cases the old and new ids are linked only through the
  account each is bound to.
- **Each side learns** the other's device key, tier, limits, certificate
  dates, reboot policy, platform, issuer key and registration height. They are
  in the certificate and receipt that every Request and Payment carries.
- **The payer also learns**, from the Request, the receiver's clock reading
  (`created_at_ms`), its anchor flag, whether it is in the clock-reset state
  (§5.4), and which descriptor and list epochs it holds.
- **The receiver also learns**, from the SendSplit, the payer's sequence
  number, its lifetime gross outflow, refunds and fees, its gross sent today
  and this month, the fee and the fee policy it was computed under (§5.8), and
  the time the payer signed. That time is `t_eff` (§5.4), which may be the
  receiver's own `created_at_ms`. The receiver does not learn the balance; no
  peer-visible object carries one. That changes if the Q0 choice "Proving off
  the payment path" shows the balance (§2.3).
- **Two payments from one payer**, seen by one receiver or by two who compare,
  show how much the payer sent and how many operations it made in between.
- **The issuer** sees every transition of a wallet that syncs, and so both
  sides of each of its payments. A wallet that never syncs shows the issuer
  nothing. §7.1 lists what can make a wallet sync.
- **The chain** holds a pooled reserve. It never executes an individual
  offline payment and never reverses one. What it holds about wallets is this:
  - the registry row that binds a device id to an account;
  - each load, and each unload as a whole RedeemSplit;
  - whole transitions wherever evidence is submitted;
  - where the issuer anchors acknowledged heads and renewal serials (§5.1,
    §7.2), the time of each sync and renewal of each device id;
  - where a fee schedule applies, a record of every payment whose fee is
    settled: the SendSplit and the receiver's credit, so the payer, the
    receiver, the amount and the payer's totals (§5.8). That record pays the
    fee. It does not move, confirm or undo the payment.
- **Bystanders.** §5.6 says what someone near the two phones can read.
- **Why the fields are there.** These fields are what the receiver's checks
  (§5.2 step 4) and the evidence rules (§5.3) test. Hiding them removes those
  checks. Recursive V1 specified a payment that showed the receiver no payer
  credential (`specs/kagemusha_v1.md:36-40, 240-251`). B1 as defined here,
  Layer A checks plus a proof, gives that up. Restoring it would be a further
  relation choice for Q0, not among those listed in §2.3, and it costs the
  receiver-side tally, `send_blocked` and fork evidence.

### 5.8 Optional fees

Owner, 2026-10-02: fees are optional, and a fee is received only into an
online account, when someone syncs. The rules below are this document's
reading of how to do that. They are untested.

- Fee policy. A fee policy is a root-signed record: a rate in parts per
  million, a fixed part, a ceiling, and one beneficiary, which is an online
  account. The fee on an amount is
  `min(ceiling, fixed + floor(amount × rate / 1,000,000))`. The formula is
  fixed so that the payer, the receiver and the ledger compute the same
  number. The fee policy id is the digest of the record. The ledger keeps
  every record ever installed, keyed by its id, and never changes a record's
  rate, fixed part or ceiling (§6). A scheme with no record has no fees.
- Which policy a wallet pays under. The issuer writes one fee policy, or none,
  into each device certificate: the id and the three numbers. A wallet pays
  under the policy in the certificate it holds. A new policy reaches a wallet
  at its next renewal and not before. With R8 off a wallet may keep its first
  policy for as long as it lives. The rule is there so that a fee change never
  stops an offline wallet paying: the owner's statement is that a holder need
  not go online again unless there are regulatory controls, and a fee is not
  one.
- The payment. The SendSplit carries `fee` and `fee_policy_id`. Both are in
  the signed preimage, so the payment binds the policy it was computed under.
  The payer's balance falls by the amount plus the fee, and the payer's app
  shows both before the user confirms. The receiver is credited the amount.
  Nobody holds the fee offline. It leaves offline circulation when the
  SendSplit commits. It becomes a claim of the beneficiary on the pooled
  reserve when the receiver credits the payment.
- The receiver's check. Before it commits, the receiver checks that
  `fee_policy_id` equals the id in the payer's certificate, which the Payment
  carries, and that `fee` equals the formula applied to the numbers in that
  certificate. If the certificate has no fee policy, the SendSplit must carry
  no fee. Otherwise the receiver answers `Refused`. The receiver uses the
  payer's certificate and not its own copy of the scheme descriptor, because
  the two phones may hold different descriptor epochs. This is the only
  offline check. A modified payer and a receiver who agrees with it can leave
  the fee out.
- A fee never makes a received payment less final. The receiver's credit is
  the full amount and is fixed when the ReceiveFold commits. Settlement
  happens later, on the ledger, between the pool and the beneficiary. No
  result of settlement debits, holds or delays a wallet or a registry row's
  claims. A fee that fails a ledger check is not paid, and nothing else
  follows from the failure.
- Refusal. A RefundFold returns the amount plus the fee, and a refused payment
  pays no fee. The ledger therefore pays a fee only when the settlement also
  carries the receiver's signed ReceiveFold or `Credited` Outcome for that
  payment id. A SendSplit alone pays nothing: the chain cannot tell whether it
  was refused. If the payer never scanned the Outcome, the fee waits for the
  receiver's sync. A fee paid for a payment that was also refunded needs a
  credit and a `Refused` Outcome from one receiver, which is evidence under
  §5.3. The alternative, a fee kept on a refused payment, charges an honest
  payer for a payment that did not happen.
- Undelivered payment. If the Payment never reaches the receiver, or a
  `Refused` Outcome never reaches the payer, the payer has lost the amount and
  the fee (§3 item 4). No credit exists, so no fee is ever paid on it.
- Counters and evidence. Every transition carries a cumulative
  `cum_fee_after`. Let Φ(t) be the fee of a SendSplit and zero for any other
  kind. The rules of §5.3 that name `cum_out_after` and Δ apply in the same
  way to `cum_fee_after` and Φ. `cum_fee_after` is gross: a refunded fee stays
  in it.
- Limits. `max_payment`, the day and month counters, `cum_out_after`,
  `cum_refund_after` and the receiver's tally count the amount only. The fee
  is outside them. The payer's balance must cover the amount plus the fee.
- Settlement. The signed SendSplit and the receiver's credit reach the issuer
  when the payer or the receiver syncs. The issuer submits
  `SettleKagemushaFee` (§6) carrying both. The chain checks, with these
  inputs:
  - the SendSplit's signature, against the device key in the payer's registry
    row;
  - the ReceiveFold's or `Credited` Outcome's signature, against the device
    key in the registry row of the counterparty the SendSplit names, and that
    it names this payment id;
  - that `fee_policy_id` is a record in the ledger's table, and that `fee`
    equals the formula under that record;
  - that neither row is under a fraud hold;
  - that this payment's fee has not been paid before.
  It then pays `fee` from the pooled reserve to the record's beneficiary. The
  check is against the record the SendSplit names, however late the
  settlement arrives and whatever the tier table says by then. The chain does
  not see the payer's certificate, so it does not check that the policy named
  is the one the issuer gave that payer. The receiver's check above is the
  only place that is tested. If the numbers in a certificate differ from the
  record its id names, the ledger's check fails and the fee is not paid; that
  is the issuer's error and costs no holder anything.
- Once per payment. The chain keeps, for each payer row, the set of sequence
  numbers whose fee it has paid, stored as ranges. A device signs one
  SendSplit per sequence number, so the pair of device id and sequence number
  identifies the payment. A SendSplit at a sequence number already in the set
  is refused. The set is never pruned, because under R5 a SendSplit can arrive
  after any delay; it stays when the row is retired. It costs one range per
  gap, and one entry per fee at worst. A counter cannot replace it, because
  SendSplits arrive in any order. The row also holds the total of fees paid
  for it.
- Order and rate of payout. A fee is paid only when the payout queue of §8.2
  is empty, so a fee is never paid ahead of an amount a holder is waiting for.
  A fee that is paid still takes pool cash, like any payout, and a fee on
  counterfeit value brings a shortfall closer (§8.3). Fee payouts for one payer
  row are capped per unload window by a tier parameter, `fee_limit`. A
  settlement that cannot be paid for either reason is refused, records
  nothing, and can be submitted again later. The cap is per payer row, so one
  row's fees do not delay another's.
- Beneficiary gone. A fee is paid to the account in the record. If that
  account can no longer receive, the root may name a replacement payout
  account for that record. It cannot change the record's numbers. Until it
  does, settlements for that record are refused and can be submitted later.
- A payment whose record never reaches the issuer pays no fee. Under R5 nobody
  has to sync.
- What the chain sees. Each settled fee puts a whole SendSplit and the
  receiver's credit on-chain: both device ids, the amount, the device time and
  the payer's totals. With a fee policy in use the chain sees every payment
  whose fee is settled, not loads and unloads only (§14), and more than §5.7
  lists. Each one costs the chain two device-signature checks.
  `SettleKagemushaFee` is accepted only from the issuer's registry authority
  (§6). That restriction is for privacy, not for money: the objects prove
  themselves, and anyone who filmed the codes of a payment holds them.
- A compromised phone. It can sign payments that never happened, each with the
  highest fee any record allows, to a second device it controls. That gains
  the attacker nothing unless a beneficiary colludes. `fee_limit` bounds it
  per payer row per window, and a fraud hold on either row stops it. A fee on
  counterfeit value is as unbacked as the value: the operator bears it (§8.4).
- Under B1 the fee and the fee policy id are part of the transition preimage
  and the relation debits the amount plus the fee, so the shape of the formula
  is fixed in Q0.
- Exposure model. Let F be fee payouts paid. In §8.3, pool cash is
  `L + B − P − I − F` and liabilities, which include fee claims not yet paid,
  are `L − P − F + C − Λ`. The covered condition `B + Λ ≥ C + I` is the same
  with or without fees. A fee that is never settled adds to Λ. A fee on a
  payment that never happened, or on counterfeit value, adds to C.
- Size, estimated and not measured: about 65 bytes on a SendSplit (`fee`, the
  id, `cum_fee_after`), about 16 bytes on any other transition, and about 70
  bytes on a certificate that holds a fee policy. A Request and a Payment each
  carry one certificate.
- Open (§12 item 17): who pays, the payer on top of the amount as drafted
  here, or the receiver out of it; one beneficiary per scheme or one per
  issuer; taxes. If the receiver pays, the fee must be taken off the credit
  before the ReceiveFold commits and never afterwards. One more choice is
  open. The issuer could settle fees in aggregate, one instruction per
  beneficiary per period, with no SendSplit on-chain. The chain then sees no
  payment and keeps no per-payment set, and the fee total rests on the
  issuer's word instead of on device signatures.

### 5.9 User authentication

Owner, 2026-10-02: PIN or biometric is a UX function, and "generally it should
be related to the secure hardware". The owner named no mechanism. The two
options below, and the choice between them, are this document's reading.
Nothing in this section is device-tested.

The options differ in what refuses to sign when the user has not
authenticated.

- **Bound key.** The device key is generated so that the secure hardware
  refuses to sign unless the user has authenticated with the screen-lock
  credential or a biometric. On Android this is a user-authentication-bound
  Keystore key; the key attestation shows the setting and the issuer checks
  it. On iPhone it is an access-control flag on the Secure Enclave key;
  nothing attests it, so it is app-vouched like the key (§4). Apple's flags
  have no time window; on iPhone a window would be kept by the app.
- **Prompted use.** The device key carries no authentication requirement.
  Before the app asks the key to sign, it shows the platform's authentication
  prompt and continues only on success: Android `BiometricPrompt` with the
  device credential allowed, iPhone `LAContext` with the device-owner policy.
  The platform checks the PIN or biometric in its secure hardware and tells
  the app the result. The key would sign without it.

| | Bound key | Prompted use |
|---|---|---|
| What refuses to sign without authentication | The secure hardware | The app |
| Thief holding the phone locked | Cannot pay | Cannot pay. Under either option the app cannot be opened |
| Thief holding the phone unlocked, without the PIN or biometric, phone not compromised | Cannot pay, except inside a window the owner left open | The same |
| Thief without the PIN or biometric who compromises the phone | Cannot pay | Can pay. Modified code skips the prompt |
| Thief who knows the PIN | Can pay | Can pay |
| Double spend by the owner | No effect | No effect |
| What the issuer can verify | Android: the setting, in the attestation. iPhone: nothing | Nothing. The prompt is the app's own rule, like every rule under T3 |
| Screen lock removed, or forcibly reset (for example by a device administrator) | Android: the key is invalidated for good (documented). iPhone: not verified. A dead key cannot pay, unload or migrate, so the balance is lost | The key is unaffected |
| A biometric enrolled, or the last one removed | Android: a key that needs a biometric at every use is invalidated, unless the device credential is also allowed (documented) | The key is unaffected |
| Signing with the user absent (renewal in the background, an unattended receiver) | Not possible, except in mode "none" | Possible |
| Receiver prompted during a payment | Yes if its window has run out. On Android under "each use", once for each of its signatures | No |
| Changing the mode | A new key and a Migrate. Android fixes the setting at key generation | A tier field. The app reads the new value at renewal |

The bound key has one cost that needs a listed exception in §3.1. A key that
the platform invalidates takes the balance with it. What triggers it is a
settings change on the phone, not a control the scheme switched on. How often
users remove a screen lock is not measured. Keeping the bound key and avoiding
the loss would need a way to move a balance without the key that holds it.
That is recovery, and recovery can only be capped insurance (§7.3).

Prompted use gives up one case: a thief who does not know the PIN and who
compromises the phone. That is the compromised phone of §4.1. This document
claims nothing against it anywhere else.

Recommended: prompted use, for every tier in the first cut. It loses no
balance. It lets a wallet renew and receive with the user absent; on iPhone
only while the phone is unlocked, because the marker and the payment key
cannot be read on a locked phone (§5.10, §7.2). On iPhone the
issuer can verify neither option, so the bound key adds nothing there that a
verifier can see. The bound key can be added later as an opt-in tier value for
Android, with its loss listed in §3.1. On Android the choice is fixed when the
key is generated, so it is made before the first enrollment (§12 item 23).

Whether prompted use meets the owner's words is for the owner to judge. It
relates the PIN or biometric to the secure hardware in one respect: the
hardware verifies it. It does not make the hardware refuse to sign.

Rules under prompted use:

- The mode is a tier field: none, a window in seconds, or each payment.
  Recommended: a short window. The app keeps the window. "None" is for an
  unattended payer.
- The app prompts before it signs a SendSplit, a RedeemSplit or a Migrate, and
  before the declaration of loss of §7.2. It does not prompt before a Request,
  a ReceiveFold, an Outcome, a RefundFold, a MintFold, a MigrateFold, a
  Recertify or a sync. Receiving and renewal therefore need no user present.
- The payer's successful authentication is the payer's confirmation. The 2 s
  target (§2) runs from that success, not from the prompt appearing. While a
  window is open the platform asks for nothing, and the confirmation shown is
  the app's own.
- A phone with no screen lock cannot show the prompt. The wallet then asks for
  a plain confirmation and tells the user that the phone is unprotected. A
  tier may instead refuse to pay until a screen lock is set. The user ends
  that stop on the phone, offline. It needs no sync and loses nothing.
- Neither the device key nor the Android marker key (§5.10) is generated with
  a user-authentication requirement or with the unlocked-device requirement.
  AOSP documents that on Android 12 to 14 removing the screen lock deleted
  every key that had the unlocked-device requirement.
- Enrollment needs no screen lock on Android. On iPhone it needs a passcode,
  because the marker of §5.10 sits in a keychain class that exists only while
  a passcode is set. Removing the passcode discards that marker under either
  option (§7.2). That loss does not come from the choice made here.

What authentication is and is not. It is a confirmation step and a theft
control. It is not a double-spend control: the holder authenticates willingly,
and authentication does not bind what is signed. A thief who can authenticate
spends the balance under either option. Under prompted use so does a thief who
compromises the phone. The statement that an honest user loses nothing through
anyone else's action does not cover a stolen phone in those two cases.

If the owner chooses the bound key instead, these also hold:

- The mode is fixed at key generation: none, a window in seconds, or each use.
  Changing it needs a new key and a Migrate on every Android phone.
- Every mode except "none" stops signing in the background. Renewal ends in a
  device-signed Recertify, so it runs only while the user is in the app and
  has authenticated. With R8 on, the user must then open the wallet online at
  least once per lease.
- The receiver signs the ReceiveFold and the Outcome inside the 2 s. If its
  window has run out since it signed the Request, it is prompted again, and
  that time counts against the target.
- Enrollment needs a screen lock on both platforms. The wallet warns before
  the first load that removing the screen lock destroys the balance.

A third arrangement was considered and is not proposed: a second,
authentication-bound key that co-signs every payment, checked by receivers.
Its death would cost a renewal and not a balance. It adds a hardware signature
and about 64 bytes to every payment (estimate), a second P-256 check per hop
under B1, and a forced sync after a screen-lock change that no regulatory
control asked for.

### 5.10 Marker and crash recovery

The marker is how an unmodified wallet refuses to continue from an earlier
copy of its own files (§4.1). This section gives its rules. The rules apply to
every object a wallet signs and commits (§5.2). They bind only an unmodified
app: a compromised phone ignores them (§4.1). Nothing in this section has been
run on a device.

**Terms.**

- A commit is one durable journal transaction. It holds one signed object, its
  signature and its state change; a ReceiveFold and its Outcome are one commit.
  Each commit has a digest over: the previous commit's digest, the object's
  digest, a digest of the wallet's state after the commit (balance, counters,
  sequence number, certificate in use, the Requests on record with their
  Outcomes), and the epochs and block list version held. The head is the last
  commit. The wallet never uses a block list or an epoch older than the one
  its head commit names.
- A marker is an entry in the phone's key store that no backup carries. Only
  its existence is used. Its name is a hash of the device id, the digest of
  the commit it belongs to, and a 16-byte random salt stored beside that
  commit. The name is tied to the digest so that files changed or cut back
  outside the wallet name no existing marker. The salt is there so that a name
  is never used twice.
- On Android a marker is a symmetric key in Android Keystore, generated in the
  TEE and not in StrongBox, with no user-authentication binding. The key is
  never used, so the cheapest kind the key store can make is enough; the
  timing test of §10.4 confirms the choice. On iPhone a marker is a keychain
  item of class `kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly`. §4.1 says
  why no backup carries either. The Android manifest opts the app out of
  backup and device transfer and sets `rollbackDataPolicy` to `retain`, so
  that a package rollback does not put older app data back. The marker does
  not depend on these settings; §10.4 tests with and without them.
- An extra marker is any marker that exists and is not the head's.

**What the journal stores.** Per commit: the object, its signature, the state
change, the previous commit's digest and the marker salt. The head and the
name of its marker follow from these. Two things are not stored.

- A marker that has been created and is not yet named by a commit. It exists
  only in the key store. Recovery finds it by listing.
- A list of markers still to delete. Every extra marker is to be deleted, and
  recovery finds them by listing. A stored list would be replaced together
  with the journal by any restore, so it cannot be the source.

**Three rules.**

1. The wallet signs and releases only when it is ready: the head's marker is
   present and no extra marker exists.
2. The wallet deletes only extra markers, and only while the head's marker is
   present. No rule in this section deletes the head's marker, the device key
   or the journal.
3. A marker name is never used twice.

What they give: an object is released only after every marker of an earlier
commit is gone. So if files are put back whose head names a marker that still
exists, nothing was ever released from any later state, and continuing from
those files contradicts nothing anyone holds. This holds as long as a deleted
marker never comes back; the one known way it could is the exposure stated
below.

**Steps for one signed object.** These are the six steps of §5.2, with what
makes each one done. Start: head k with marker `old`.

1. The wallet is ready.
2. Create the marker `new` for commit k+1. Done when the call returns success
   and a read of `new` returns present.
3. Sign in memory.
4. Commit k+1. Done, and durable, when the transaction returns.
5. Delete `old`. Done when the call returns success and a read of `old`
   returns definitely absent.
6. Release.

If any call or read in steps 2 and 5 returns unknown, the wallet is no longer
ready. It runs recovery before it signs or releases anything else. Within one
process the wallet knows which markers it created and deleted, so the six
steps need no listing. One process at a time holds the journal.

**Durability.**

- Journal. A commit is durable when its transaction returns (§5.2). The forced
  power-off test of §10.4 checks that the phone's storage honours this.
- Key store. The wallet cannot force a key-store write to storage. It takes a
  creation or deletion as done when the call has returned and a read confirms
  it. Whether that survives a power cut differs by platform.
  - Android. AOSP's key-store service opens its database with SQLite's default
    settings, which sync each transaction before it returns (`keystore2`
    `database.rs`; Android's SQLite build flags set no other default). Read
    from AOSP source on 2026-10-02. Vendor builds were not checked.
  - iPhone. Apple's published keychain source opens its database in
    write-ahead-log mode and sets no synchronous or full-sync option
    (`SecDb.c`). Whether a returned add or delete survives a power cut is not
    established.
- Recovery therefore does not assume it. A lost creation is repaired (step
  R6). A lost deletion is done again (step R5). One exposure remains and is
  stated under "What an ordinary user can still do". Recovery assumes one
  thing only: a key store that loses writes in a power cut loses its latest
  ones, and never keeps a later write while losing an earlier one. Both key
  stores are single SQLite databases in the sources read. If a phone breaks
  that assumption, an honest wallet can end up stopped after a power cut.

**Read outcomes.** Every read has three outcomes. Unknown is never treated as
absent.

| Platform call | Present | Definitely absent | Unknown |
|---|---|---|---|
| Android, read: `KeyStore.getKey(alias, null)` | returns a key | returns null. AOSP returns null only for the key-not-found code | throws any exception |
| Android, delete: `KeyStore.deleteEntry(alias)`, then a read | | returns without an exception, and the read says definitely absent | throws, or the read says anything else |
| iPhone, read: `SecItemCopyMatching` for one item | `errSecSuccess` | `errSecItemNotFound`, while the app reports protected data available before and after the call | `errSecInteractionNotAllowed` (the phone is locked); any other status; `errSecItemNotFound` while protected data is not available |
| iPhone, delete: `SecItemDelete`, then a read | | `errSecSuccess` or `errSecItemNotFound`, and the read says definitely absent | any other status, or the read says anything else |

A listing has two outcomes: the list, or unknown.

- Android: `KeyStore.aliases()`. The result counts as the list only if it
  contains the device key's alias; otherwise it is unknown. AOSP returns an
  empty or shortened list on a key-store error, without an exception. The key
  store lists aliases in sorted order, and the device key's alias is chosen to
  sort after every marker alias, so a result that contains it is complete.
- iPhone: `SecItemCopyMatching` for all items of the marker service.
  `errSecSuccess` gives the list. `errSecItemNotFound` while protected data is
  available is the empty list. Any other status is unknown.

Notes on the table.

- Android applies to version 12 and later, the version the Pixel 6 shipped
  with. `containsAlias`, `isKeyEntry` and `size` are not used: AOSP returns
  false or zero from them on any key-store error (AOSP
  `AndroidKeyStoreSpi.java`, `AndroidKeyStoreProvider.java` and `keystore2`
  `database.rs`, read 2026-10-02).
- iPhone. Apple documents that the marker class behaves like the
  when-unlocked class, so no marker can be read while the phone is locked.
  Apple's published keychain source ends a query with
  `errSecInteractionNotAllowed` when a matching item cannot be decrypted
  because the phone is locked, and with `errSecItemNotFound` only when the
  query had no error and matched nothing (`SecItemDb.c`, read 2026-10-02).
  That source may differ from what a given iOS version ships. Not tested.
- iPhone, no passcode. Apple documents that removing the passcode discards the
  keys of the marker class. If the phone reports that no passcode is set,
  every marker is definitely absent. What a read returns then is not tested.
- The journal is read the same way. Readable. Unknown: the storage is locked,
  or any input-output or database-busy error. Definitely absent: the storage
  is available, its directory can be listed, and it holds no journal file.
  Inconsistent: the file opens and its content fails its own checks (a commit
  that does not chain, or stored state that does not match the head's state
  digest).

**Recovery.** It runs inside `open` at every start. It runs again before any
operation if the previous marker step did not end with a confirmed result.
Until it ends in "ready" the wallet signs nothing and releases nothing. The
steps below are numbered R1 to R6 in this section and in §10.4. They are not
the requirements R1 to R9 of §1.

- R1. Read the journal. Unknown: wait. Definitely absent or inconsistent: stop
  (§7.2).
- R2. Read the device key's entry. Unknown: wait. Definitely absent: stop; the
  key is gone (§7.2). A key that the hardware reports as permanently
  invalidated is §5.9's case.
- R3. Compute the name of the head's marker and read it. Unknown: wait.
- R4. List the markers. Unknown: wait.
- R5. If the head's marker is present: delete every extra marker and confirm
  each one definitely absent. Any unknown: wait. Then the wallet is ready.
- R6. If the head's marker is definitely absent: look among the listed markers
  for one that an earlier commit of this journal names.
  - None: stop. Nothing is deleted.
  - Otherwise take the latest such commit. Every commit after it must chain
    by digest, carry a valid device signature over its object, and hold the
    state digest the wallet core computes by applying that object. If one
    does not: stop. If all do: choose a new salt for the head, create the
    marker with the new name, store the salt durably, and go to R5.

R6 is there so that a power cut which loses a key-store write does not cost an
honest user the wallet. The signature check in it is there so that records
added to a copy of the files are not adopted. Every step can be repeated after
an interruption.

A new enrollment starts with the Bootstrap commit and its marker. Markers left
in the key store by an earlier installation whose key and files are both gone
are then extra, and R5 deletes them. While an earlier wallet's key or files
are still on the phone, the markers found at enrollment are not extra: the new
wallet records their names in its Bootstrap commit and leaves them. They are
deleted only where §7.2 deletes that wallet's key and files. Otherwise
enrolling again would end a wallet that a repair or an unload could still
reach (§7.1, §7.2).

**States.** §7.2 describes the same conditions under other names. Ready is its
consistent state. Waiting is its unavailable condition. Stopped is its
inconsistent condition; §7.2 adds one case to it, which only the issuer's copy
can show.

- Ready. The wallet signs and releases. §7.2 calls the three states
  consistent, unavailable and inconsistent, in the order given here.
- Waiting. Some read, list, create or delete returned unknown. The wallet
  signs nothing, releases nothing and deletes nothing. It tries again when the
  phone is unlocked, when the app comes to the foreground, and on each call.
  It never concludes that anything is lost. An object that is committed and
  not released stays that way until the wallet is ready. If the wait does not
  end, the app shows the platform error.
- Stopped. The head's marker is definitely absent and no earlier commit's
  marker exists; or the journal is definitely absent or inconsistent; or the
  device key is definitely absent. The wallet signs nothing and deletes
  nothing. The state is evaluated again at every start, so a reading that
  later proves wrong costs nothing: if the matching files and the marker are
  there at a later start, the wallet is ready again. What a user can do from
  this state is §7.2. §7.2 names the same conditions from the user's side:
  its unavailable state is waiting here, and its inconsistent state is stopped
  here, with one more cause that only a sync can show.

**Crash points.** Object k+1 on head k. `old` is head k's marker and `new` is
the marker for commit k+1. Copy A is the wallet's files as they were at head k.
An older copy is the files from any earlier head. Step numbers are those of
§5.2.

| Interrupted after | Process killed, then start | Power cut, then start | Copy A restored, then start | Older copy restored, then start |
|---|---|---|---|---|
| Step 2 or 3 (marker created, nothing committed) | Head k. `new` is extra and is deleted. Ready at k. The signature was only in memory | The same. `new` may already be gone | The same as a plain start | Stopped |
| Step 4 (committed, `old` not deleted) | Head k+1. `old` is extra and is deleted. Ready at k+1. The object is unreleased and `open` returns it | The same. If the key store lost `new`, `old` is named by commit k and R6 repairs. Ready at k+1 | Head k, `old` present, `new` deleted. Ready at k. Commit k+1 is gone; nothing of it was released, so this is not a reset | Stopped |
| Step 5 (`old` deleted, not released) | Ready at k+1. The object is unreleased and `open` returns it | The same. If the key store lost the deletion, `old` is deleted again. If it lost both writes, R6 repairs | Stopped. Nothing had been released, but the wallet cannot tell this from the next row | Stopped |
| Step 6 (released) | Ready at k+1. The object can be presented again | As the row above | Stopped, unless the key store lost the deletion: then `old` exists, copy A is accepted, and the released object is undone. This is the one exposure | Stopped |

If the key store lost more than one operation in a power cut, R6 finds the
marker of an earlier commit and the wallet continues at its head.

Three sequences these rules are built to stop, each with an unmodified app.

- Copy the files; commit an object; kill the app before the deletion; start
  again and make a later payment; restore the copy. Recovery at the start
  deletes the first marker before anything is signed, so the copy names a
  marker that is gone.
- Commit an object and kill the app before the deletion; copy the files;
  restore copy A and pay someone else; restore the second copy. Recovery
  after the first restore deletes the marker of the abandoned commit as an
  extra marker, so the second copy names a marker that is gone.
- Receive a payment and refuse it; let the payer refund; restore files from
  before the refusal; accept the same payment. The `Refused` Outcome is a
  commit with its own marker, so the restored files name a marker that is
  gone.

**What an ordinary user can still do.**

- Stop their own wallet: by restoring older files, or on iPhone by removing
  the passcode. The balance is then out of reach (§7.2). This cannot be
  avoided. A wallet that accepted files without their marker would accept a
  reset, and the only iPhone keychain class that Apple documents as never
  backed up needs a passcode.
- Use the exposure in the last row of the table, on a phone whose key store
  loses a returned deletion in a power cut: copy the files, pay, force the
  power off at once, restore the copy before the wallet starts. Whether any
  target phone has that window is not tested. Reading the source, the Android
  reference does not and the iPhone is open.
- Use any vendor backup or clone tool that restores key-store entries. None
  is known. No vendor tool has been tested (§4.1).
- Edit a copy of the files and restore it. The marker name is tied to the
  commit digest, so changed state names no existing marker and the wallet
  stops. Data the wallet writes without a commit is outside the digest: a
  newer block list, time records, and an Outcome received from the other
  side. An edited copy can take the block list and the epochs back to the
  version the head commit names and no further. It can remove a received
  Outcome; the payer then presents the Payment again and gets the same
  Outcome. The last signed time is inside the digest; what the other time
  records allow is bounded by §5.4.
- One step beyond the unmodified app: a copy of the files taken between steps
  4 and 5 holds a signed, unreleased object. The unmodified app never shows
  it; a restored copy leads only to the states in the table. A person who
  shows that object with other software, after restoring copy A and paying
  someone else, has spent twice. That needs a second program, a backup tool
  whose output can be read, and stopping the app inside a window of
  milliseconds. It leaves two digests at one sequence number (§5.3). Closing
  it means keeping the signature out of the journal until step 5 is confirmed
  and writing it durably before step 6: a second durable write on every
  payment, and signing again after a crash. Not adopted here; §12 asks.

**What a compromised phone can do.** Everything. It keeps, copies or recreates
markers as it likes and signs from any state. The marker is the paying app
policing itself; a receiver cannot check it (§4.1).

**Cost.** Per signed object, on the phone that signs it: one marker creation,
one deletion and two confirming reads. That is once on the payer and once on
the receiver inside the two-second window of §2, and once more on the receiver
earlier, for the Request. On Android a creation is a Keystore key generation.
At each start: two reads, one listing, and a check of the stored state
against the head's state digest. None of this is timed. The effect of many
thousands of creations and deletions on a key store is not tested.

### 5.11 Versions, policy changes, key rotation and closure

A scheme changes after phones hold value: the rules get a new version, a tier
row is tightened, a key is replaced, the scheme closes. This section says how
each change reaches a phone that is offline and what the phone can do before
it arrives. Two rules hold throughout. No change reduces value a wallet has
already received, and that value stays redeemable at face value with no time
limit. No change makes a wallet delete a key or a balance. Nothing in this
section is implemented or tested.

**Rules versions**

- A rules version is one integer per scheme. It fixes the transition layout,
  the checks of §5.2 steps 2 and 4, the time rules of §5.4, the fee
  computation and the evidence rules of §5.3. Every transition carries the
  version it was signed under as its first field, and the place and width of
  that field never change.
- The descriptor allows a range for payments, `rules_floor` to `rules_max`.
  A wallet signs no SendSplit, and lists no version in a Request, outside the
  range of the newest descriptor or rules-range notice it holds.
- Release rule for the app: every build supports every version from the
  scheme's `rules_floor` at the time of its release up to its own highest.
  Nobody can check this offline; it is a rule for the operator's releases.
  With it, two apps that are both at or above the floor always share a
  version.
- **Choosing the version for a payment.** The Request states `rules_lo` and
  `rules_hi`: the versions the receiver's app can judge and its descriptor
  allows. The payer signs the SendSplit under the highest version in that
  range that its own app supports and its own descriptor allows. If there is
  none, the payer signs nothing and nothing is debited; both apps say which
  one must be updated. This is so that a payer is never debited by a
  transition the receiver cannot judge. Checked by the payer's unmodified
  app, with the signed Request as input.
- The receiver refuses a SendSplit whose version is outside the range its own
  Request stated. The signed preimage is `tag ‖ scheme id ‖ rules version ‖
  payload` under every version, so the receiver can verify the device
  signature over a payload it cannot read and answer with a signed `Refused`.
  The payer then refunds. Only a modified or faulty payer sends such a
  SendSplit.
- A wallet signs its own other transitions (ReceiveFold, RefundFold,
  RedeemSplit and the rest) under the highest version it supports that is not
  above `rules_max`. The floor does not apply to them. It limits SendSplits
  and Requests only, so a wallet below the floor can still refund and unload.
  A journal therefore mixes versions. Each transition is judged under its own
  version, and each new version states how it follows a state left by an
  earlier one.
- The Request, the Payment envelope, the Outcome, the certificate, the
  receipt, the notices and the list segments each have one layout for the life
  of the scheme, with an extension area. A reader ignores an extension it does
  not know, unless the extension is marked critical; then it treats the object
  as one it cannot judge, and no payment is made. This is so that a Request
  can always be read far enough to find a common version. About 2 to 4 B per
  object (estimate).
- The issuer's replay and the ledger keep the checks of every version that
  was ever allowed, without time limit. A RedeemSplit, a fee settlement or
  evidence signed under a retired version is judged under that version. Under
  B1 a rules version also fixes the relation; how value proven under an
  earlier relation is accepted after a change is decided in Q0 (§2.3).
- Raising `rules_max` switches a new version on. It changes nothing for a
  wallet that does not have it: the two wallets use the highest version they
  share.

**A version floor is a forced update**

- Raising `rules_floor` retires the versions below it from a date,
  `effective_from_ms`. From that date a wallet that holds the notice signs no
  SendSplit under a retired version and lists no retired version in a
  Request. An app whose highest version is below the floor can then neither
  pay nor be paid by a wallet that holds the notice. It still works with
  wallets that do not hold it.
- The holder's balance is not changed. The holder updates the app, which
  needs a connection to an app store and no issuer sync, and the updated app
  reads the same journal and key. Without updating, the holder can still
  unload at face value: the ledger accepts a RedeemSplit under any version
  that was ever allowed.
- A floor is not a regulatory control. It forces a holder online, to update,
  for a reason that is not R6, R7 or R8. It is therefore an exception to the
  rule that nothing forces a holder online except a regulatory control, and
  the owner decides whether it exists (§12).
  - Why it cannot be avoided if a version must be retired offline: a version
    with a defect, for example one that accepts a payment it should refuse,
    stays acceptable to every receiver that supports it until those receivers
    stop listing it. The operator underwrites whatever that version lets in.
  - The cost of avoiding it: versions are then retired only where the issuer
    can refuse to certify, at enrollment and renewal. With R8 on, an old
    version dies out within one lease. With `Never` certificates it lives as
    long as its holders stay offline.
- Recommended: keep the mechanism, set no floor in normal operation, and use
  one only to retire a version with a defect that creates value.
- The floor is on the rules version, which wallets declare to each other.
  Nothing offline shows which app build a wallet runs. The issuer sees
  platform evidence of the build only at a sync, and only as far as §4 says,
  so a minimum build can be required only at issuance or renewal.

**How a change reaches an offline phone**

- At a sync the issuer hands over the current descriptor, the notices and
  the list.
- Peer to peer, in the three messages of §5.2. Each message states what its
  sender holds (§5.1). A Payment attaches the notices the Request shows the
  receiver lacks. An Outcome attaches the notices the Payment shows the payer
  lacks, and always the notice behind a refusal. A Request attaches the
  issuer key certificate and root succession its own certificate needs while
  those are less than 90 days old (proposed; a descriptor value), because the
  receiver cannot know what the payer holds. That adds about 0.2 KB to the
  Request for that period, and about 0.3 KB more after a root succession
  (estimates). All of this is subject to the message's size bound.
- If a wallet still cannot verify the other's certificate, no payment is
  made, and the other wallet's app can show its notices as a separate code.
  That costs one extra scan, once per wallet per change.
- With an app update, which may carry the newest notices.
- There is no push to an offline phone. A wallet that has received a notice
  from nobody applies the older terms, and nothing relies on it having done
  otherwise: the counterparty that does hold the notice applies it.
- Adopting a notice or a list segment is not a signed object. A wallet
  restored to a state from before an adoption (§5.10 says when that is
  possible) is a wallet that has not received it.

**A tightened tier row**

- A tier-row notice replaces one row. A wallet that holds it applies, to
  every certificate of that tier, the stricter of the certificate's value and
  the row's, term by term (§5.1): the lower amount, the earlier date, the
  smaller tolerance, `require_anchor` over `wall_clock`, a `Cap` over `None`.
  A payer applies it to itself; a receiver applies it to the payer in its
  checks and in its tally.
- A tightened row therefore binds a payment as soon as either side holds it.
  If only the receiver holds it and the payer's counters already exceed the
  new limit, the receiver answers `Refused` with the notice attached, and the
  payer refunds and adopts it.
- Amount terms apply at once. A term that brings in or shortens expiry
  applies to certificates already issued no earlier than the notice's
  `effective_from_ms`: for them the lease ends at the later of that date and
  `not_before` plus the row's longest lease, or at the certificate's own
  `not_after` if that is earlier. This is so that a holder is not cut off by
  a date that passed before the notice existed. The root chooses
  `effective_from_ms`; an immediate date is possible and is the operator's
  decision.
- A loosened row changes nothing for a certificate already issued. Looser
  terms reach a phone only in a new certificate, at a sync.
- No holder is stranded. A tightened row is R7 or R8 with new values. At its
  strictest (a limit of zero, or a lease that has ended) the holder cannot
  send until the next window or the next sync. The holder can still receive,
  the balance is unchanged, and it unloads at face value.
- A change of fee policy does not alter a payment already signed: the
  SendSplit names the id of the fee policy record it used, and the ledger
  keeps every record ever installed (§5.8). A wallet pays under the policy in
  its certificate until its next renewal.

**Planned key rotation**

- Replacing a key on schedule uses no revocation. The old key stops signing.
  Everything it signed stays valid on its own terms: a certificate until its
  own expiry, a `Never` certificate without limit of time, a receipt while its
  quorum stands. Wallets keep every issuer key certificate and every root key
  they have held that has not been revoked. A planned rotation therefore
  forces nobody online. Revocation is for compromise only (§5.1).
- **Issuer keys** (certificate, voucher, list, witness). The root signs an
  issuer key certificate for the new key. New certificates, vouchers,
  segments and receipts are signed with it. A peer that lacks the new key
  certificate receives it as a notice.
- **Root key.** The descriptor holds `next_root_digest`. A root succession
  notice carries the succession number, the new root key, the digest of the
  key after it, and the last descriptor epoch the old key signed. It is signed
  by the old key and by the new key. A wallet accepts it only if the new key
  hashes to the `next_root_digest` it holds and both signatures verify. After
  that it accepts the new key, and the old key only for epochs up to the one
  named. The commitment is there so that someone who steals the current root
  key cannot name a successor of their own. A wallet that has not yet received
  the succession still accepts whatever the old key signs, so a replaced root
  key must be destroyed or guarded like a live one. About 0.3 KB per
  succession (estimate). This construction has not been reviewed.
- A wallet takes the first root key of a scheme from the descriptor it
  receives at enrollment, checked against a digest in the app build. Every
  later root key reaches it only through a succession notice.
- **A phone that has been offline for years.** It holds an old root and old
  issuer keys. Until it learns the new ones it can do everything it could
  before: pay and be paid by wallets whose certificates it can verify, under
  its own certificate, which other wallets still accept. It cannot verify a
  certificate issued under an issuer key it has never seen. The other wallet
  hands it the succession notices and the issuer key certificate, in the
  exchange or as a separate code. It verifies the chain link by link from the
  root it holds, and then pays or is paid in the same meeting. Several
  successions cost about 0.3 KB each. Where R8 is on, its own lease ended
  long ago, so it must renew before it sends; that is R8, not the rotation.
- **Compromise.** A stolen issuer key is revoked (§5.1), with the forced sync
  that §3.1 puts to the owner. A stolen root key can sign notices and issuer
  key certificates until wallets hold the succession to the committed next
  key; it cannot forge that succession. What a root compromise voids is part
  of the same decision.
- The scheme cell on the ledger must accept a new descriptor epoch, a new
  root and a status change. The existing governance pattern for KAGEMUSHA
  installs a policy once against an empty predecessor and has no path to
  replace it (`crates/iroha_data_model/src/governance/types.rs:755-781`), so
  this is new ledger work (§6, §8.5).

**Closure**

A received payment stays redeemable at face value with no time limit (§3),
so closing a scheme cannot end redemption. Closure means the following and no
more.

- **Closed to loads.** From a descriptor epoch on, the ledger refuses Load
  and the registration of new devices, and the issuer issues no voucher and
  enrolls no device. Payments between wallets, renewals, unloads, fee
  settlement and evidence go on as before. A wallet that learns of the status
  shows it to the user and changes nothing else. Closure by itself forces
  nobody online.
- **Bringing value home.** The operator can wait, or it can switch R8 on for
  every tier with a tier-row notice and stop renewing after a date. Sending
  then stops at each certificate's lease end, and holders unload. That is a
  regulatory control used at closure. It stops sending. It does not stop
  receiving, and it does not stop unloading.
- **What never ends.** An unload pays at face value whenever it is presented.
  The ledger keeps the registry, the pooled reserve, the unload instruction,
  fee settlement, evidence, the checks of every rules version and the fee
  schedule of every epoch. The operator's obligation under §8.4 continues.
- **The cost.** Value that will never be presented cannot be told from value
  still held (§8.3), so the reserve behind unredeemed offline value can never
  be released on the scheme's own evidence. If the chain itself is to be
  retired, the last descriptor must name where unloads are presented
  afterwards and who pays them. A deadline for redemption would make holders
  go online by a date. The owner's statement is that there is "no need to
  ever go online again unless there are regulatory controls". This document
  reads that as excluding a deadline of the scheme's own making; whether a
  legal dormancy rule counts as a regulatory control is for the owner (§12).
- A phone that has been offline since before closure can still pay other
  wallets, subject to its own certificate, and can unload at any time. A
  holder whose phone is failing cannot move to a new phone after closure,
  because no device is enrolled; that holder unloads instead.

## 6. Roles

No role below takes part in an offline payment except the two wallets (R1).

| Role | Keys it holds | What it does | What it learns |
|---|---|---|---|
| Ledger (validators, and anyone who can read the chain) | Consensus keys. Under B1 also the existing mint-finality keys | Holds the pooled reserve, the device registry, the scheme cell, the fee policy table and the block index; executes the instructions below | Each registry row: device key, tier, bound account, provenance class, status, totals. Each load and unload with amount and time. The time of each renewal and each sync of each device, with its sequence number. Successions. Every transition submitted as evidence. With fees, every payment whose fee is settled (§5.8). Not wallet balances, and no other offline payment |
| Scheme root | Root key, kept offline | Signs the scheme descriptor, issuer key certificates and revocations, and fee policy records | Nothing from operation |
| Ledger governance | The ledger's existing governance procedure | Installs the scheme cell and fee policy records after checking the root signature | What the ledger learns |
| Block authority | Whatever §12 item 9 names | Sets and clears account blocks (R6) | What the ledger learns |
| Issuer service (off-chain) | Certificate key, voucher key, list key, registry authority key. Four separate governed roles | Verifies attestation; signs certificates, vouchers, lists and registration authorizations; accepts sync; submits anchors, retirements and fee settlements | Each device's attestation and bound account. Every transition of a wallet that syncs, so both sides of each of its payments and its balance. When and from where each wallet connects |
| Registration witnesses | One witness key each | Sign receipts and record them in the registry (§8.1) | Under model (A), each device's platform attestation and registration. Under (B), the registration only |
| Operator treasury | Funding account key | Funds the backstop (§8.4) | What the ledger learns |
| Fee beneficiary | Its online account key | Receives fees | The payments whose fee it was paid, from the chain |
| Bound account (the user online) | Its account key | Registers the device, loads, receives unload payouts | Its own row |
| Wallet | Device key in secure hardware; journal in no-backup storage; marker (§5.10) | Pays, receives, unloads | What §5.7 lists |

The issuer re-checks stored Android attestation serials against Google's
revocation list daily. Apple publishes no revocation for App Attest keys; the
only Apple-side signal is the receipt risk metric.

The pool account has no signer. Only the instructions below move its balance.
The existing reserve account is already built that way: its id is derived from
a key nobody can sign with, and a direct transfer out of it is refused
(`crates/iroha_core/src/smartcontracts/isi/domain.rs:613-621`,
`crates/iroha_core/src/smartcontracts/isi/asset.rs:695-708`; read from code,
not run).

**Ledger instructions.** These are all the instructions the flows of §5 to §8
need. None exists today. Each touches at most two registry rows and the pool
record; none iterates over rows or claims.

| Instruction | Submitted by | The chain checks | Effect |
|---|---|---|---|
| `SetKagemushaScheme` | Ledger governance | Root signature over (epoch, digest); epoch above the cell's | Installs or replaces the scheme cell: role keys and accounts, tier table, ledger parameters, load open or closed. A key revocation is a new epoch |
| `AddKagemushaFeePolicy` | Ledger governance | Root signature; id not yet present | Adds a fee policy record. A second form replaces only a record's payout account (§5.8) |
| `SetKagemushaAccountBlock` | Block authority | That authority | Sets or clears `send_blocked` and `receive_blocked` for an account; consensus derives the device entries. Not needed if §12 item 9 names an existing ledger fact |
| `RegisterKagemushaDevice` | The account being bound | Authorization signed by the registry authority key; submitter equals the account in it; account not blocked; device id is new and is the hash of the key; registration cap | Creates the registry row |
| `RecordKagemushaReceipt` | Anyone; in practice a witness | At least k witness signatures under unrevoked keys, over the row's scheme id, device id, tier and registration height | Records the receipt in the row (§8.1, §8.2) |
| `AnchorKagemushaSerial` | Registry authority | Row is live; bound account not blocked; new serial is the row's serial plus one | Raises the row's anchored serial. The issuer releases a certificate only after this is final |
| `AnchorKagemushaHead` | Registry authority | That the device key in the row signed this head, and that its sequence number is not below the row's | Records the acknowledged head and its sequence number |
| `RetireKagemushaDevice` | Registry authority | Either the old key's signed Migrate naming a successor row bound to the same account, or the bound account's signature where the device key cannot sign (§7.2) | Sets the row to retired with its final redeemed total and successor; consensus emits the final block entry |
| `LoadKagemusha` | The bound account | Row is live and not under a hold; account not blocked; load is open; load id is new | Moves the amount into the pool; adds it to the row's loaded total |
| `RefundKagemushaLoad` | Anyone | The load exists and is not refunded; a statement signed by the voucher key that no voucher was or will be issued for that load id | Returns the load to the bound account as §7.1 says |
| `UnloadKagemusha` | Anyone | With a RedeemSplit: its signature against the row's key, and the row's status (§8.2). With a device id only: nothing more | Records the claim, if any; pays what is due to the bound account or queues it (§8.2) |
| `FundKagemushaReserveBackstop` | The funding account named in the scheme cell | That account | Moves the amount into the pool; adds it to the backstop counter; serves the payout queue |
| `SettleKagemushaFee` | Registry authority | §5.8 | Pays one fee to the record's beneficiary |
| `SubmitKagemushaEvidence` | Anyone | One of the predicates of §5.3, with every signature it relies on checked against the registry or the scheme cell | Places the fraud hold that §12 item 11 decides |
| `ReinstateKagemushaDevice` | Whoever §12 item 11 names | To be defined with that decision | Moves a held balance to a new device id |
| `ClaimKagemushaRecovery`, `PayKagemushaRecovery` | The bound account files; the payout is submitted as §7.3 defines | §7.3. Refused unless the scheme enables recovery; a payout changes no row's status | Pays an insurance claim within the cap and budget |

Under B1, `LoadKagemusha` and `UnloadKagemusha` also carry what the relation
fixed in Q0 requires, and the existing top-up path stays (§8.5). Under witness
model (B) one more instruction is needed, by which the issuer flags a
registration it cannot match to its log.

The head anchor has a cost that the normative spec must settle. The chain can
check the device's signature over a head in two ways. If the transition
preimage is laid out so that the sequence number and a digest of the rest are
enough to verify the signature, the anchor carries the head transition's own
signature and the wallet signs nothing more. Otherwise the wallet signs a
separate sync statement, which is one more hardware signature per sync and,
under §5.9, needs the user to have authenticated.

**What depends on the chain being live.**

| Operation | Issuer service | Chain finality | Witness quorum |
|---|---|---|---|
| Offline payment | no | no | no |
| Enroll | yes | yes | yes |
| Load | yes | yes | no |
| Unload | no | yes | no |
| Renewal (R8 on) | yes | yes, for the serial anchor | no |
| Sync without renewal | yes | no; the head anchor may follow | no |
| Migrate | yes | yes | yes, for the new device |
| Block list refresh | yes | no | no |

- A renewal waits for chain finality. The issuer anchors the new serial before
  it releases the certificate, because a block entry covers serials up to the
  anchored one (§5.1). With R8 on, a chain outage longer than what is left of
  a lease plus `expiry_grace` stops that phone sending until the chain is
  back. With R8 off no wallet renews, and a chain outage stops nothing
  offline.
- Unload needs a ledger node and nothing else. It does not need the issuer, a
  valid certificate or a fresh list (§8.2).
- The dependence of renewal on the chain can be removed. The issuer would
  anchor several serials ahead and issue certificates up to the anchored
  serial without a further write. A block entry would then cover serials not
  yet issued, which is harmless. This is not adopted here; it changes the
  serial rule of §5.1 and is an owner decision (§12).

**What the chain learns about syncs.** Every renewal and every sync writes to
the device's registry row. Anyone who reads the chain sees when each device
id, and so each bound account, went online, and from the sequence number how
many operations the wallet made between two syncs. The amounts and
counterparties are not on-chain. Two changes would reduce this, and neither is
adopted here. Anchoring serials ahead, as above, hides renewals. Anchoring
heads as one digest per period over all devices that synced hides which device
synced; the flows that read a row's acknowledged head (§7.2, §7.3) would then
carry a membership proof, and the check that a head was signed by the device
would move from the chain to the issuer. Privacy is not among R1–R9 (§5.7), so
this is an owner decision (§12).

## 7. Flows

A payment needs no network (R1). Every flow in this section needs it. A wallet
starts a flow only in the consistent state defined in §7.2. The two exceptions
are the repair and the declaration of loss in §7.2.

### 7.1 Enroll, load, unload, sync

- **Enroll.** Android generates the payment key with the attestation
  challenge. iPhone creates the payment key, attests a separate App Attest key
  over a transcript naming it, and signs that transcript with the payment key.
  The issuer verifies and signs the certificate and registration
  authorization; the account registers the device on-chain; the witnesses sign
  the receipt. The wallet cannot pay or request before it holds both.
  Enrollment therefore depends on the issuer, chain finality and the witness
  quorum, and every user needs an on-chain account able to submit the
  registration. The chain learns the device id, the account it is bound to,
  the tier and the provenance class (§8.1). Enrollment creates a new key under
  a new alias. It never deletes or overwrites a key or a journal already
  on the phone (§7.2). Markers are the one exception. Once the new Bootstrap
  commit's marker exists, markers left by an earlier wallet are extra and
  §5.10 deletes them, without waiting for the old id's retirement. After that
  the earlier wallet on that phone can be neither repaired nor unloaded, and
  the app says so before the user confirms.
- **Load.** Device-generated load id; `LoadKagemusha` signed by the bound
  account; voucher id is the hash of the operation id; the issuer records a
  voucher as issued before releasing it; the wallet writes MintFold. A
  repeated request for the same load id returns the same voucher.
  A load is committed on-chain before its voucher exists, so a load can be
  left without one. Such a load is refunded on-chain to the bound account. The
  chain cannot see vouchers, so the refund needs the issuer's signed statement
  that the load id is void. The issuer marks the load id void in its own
  record before it signs, and issues no voucher for a void id. The chain
  checks that signature, that the load is committed to that account, and that
  it was not refunded before. The refund does not wait for the device id to be
  retired. The rule is for a failed load: it must not hold the user's money
  until the wallet is given up. An issuer that signs a void statement and also
  releases a voucher has created unbacked value, as a stolen voucher key does
  (§8.1). If the issuer answers with neither, the amount stays in the pool as
  the account's claim and no rule here ends the wait: a time limit enforced by
  the chain would need every voucher anchored on-chain before release, which
  is one more write per load.
  The refund first removes that load from the row's loaded total. If the row's
  paid total is above what remains, the refund up to that excess is released
  at no more than `unload_limit` per unload window, as a claim above the
  row's own loads is (§8.2); the rest is paid at once. None is forfeited: a
  device that received more than it loaded has a paid total above its loads
  without having cheated.
- **Unload.** RedeemSplit carries the increment and the cumulative total; the
  chain records a claim against the registry row (§8.2). The wallet reads its
  row before signing, shows what will be paid now and what will wait, and
  signs for no more than §8.2 will record. An unload pays only the account
  bound to the device id in the registry. The RedeemSplit names no payee, and
  the chain takes the payee from the registry row. Anyone may present a signed
  RedeemSplit, at any time; the presenter gains nothing. Send expiry, the
  clock-reset state and retirement of the device id (§7.2) do not stop an
  unload.
- **Sync.** A direct exchange with the issuer, authenticated by the device key
  with a signature over an issuer nonce (on iPhone also an App Attest
  assertion). That signature changes no state and is not a journal object. The
  wallet uploads its journal above the acknowledged head. The issuer replays
  it and returns what is due: a renewed certificate, the block list, issuer
  key certificates and revocations, the descriptor, and a time anchor (§5.4).
  The issuer then acknowledges the head it has replayed. Where the sync itself
  adds a transition (a `Recertify`), the wallet uploads that too and the
  issuer acknowledges it. If that last step is lost, the acknowledged head
  stays one transition behind. That costs only the repair of §7.2, until the
  next sync.
- **Renewal.** A sync that returns a certificate with a higher serial under
  the same device id. The wallet adopts it with a device-signed `Recertify`.
  A renewal does not reset the day and month counters (§7.2). It depends on
  three things:
  - the issuer being reachable;
  - the device key signing the `Recertify`; under the recommendation of §5.9
    that needs no user present;
  - the chain finalizing one write. Before it releases the certificate, the
    issuer writes the new serial and the acknowledged head into the device's
    registry row and waits for that to be final. The serial is written because
    an R6 block entry covers certificates up to the last serial on-chain
    (§5.1); a certificate released first would escape an entry made in
    between. The head is written because the acknowledged heads are the only
    check against a stale journal (§7.2).

  It does not depend on the witnesses unless the tier changes; the receipt
  names the registered tier (§5.1). A repeated request returns the same
  certificate, so a lost response uses up no serial.
  Two consequences follow from the chain write. If the chain does not
  finalize, no certificate is renewed, and under R8 a phone whose lease runs
  out in that time stops sending. And the chain learns, for each device id and
  so for each account, when it renewed and how often, with a digest of its
  journal head. It learns no amount and no counterparty. A sync that renews
  nothing also writes the head, so the chain shows every sync. With R8 on that
  is at least one visible write per device per lease.
  Two changes would remove the write per renewal. Neither is adopted here;
  both are for the owner. The issuer could anchor in advance the highest
  serial it may issue for a device id, with block entries covering up to that
  ceiling; renewals below the ceiling would then need no chain write. And it
  could anchor one commitment to its whole table of acknowledged heads per
  interval, instead of one head per sync. Both hide the time of each sync from
  the chain.

Sync is optional. A wallet that never syncs keeps paying and receiving, except
in the cases below. These are all the cases that the rules of §5 and §7
create.

| What stops | When | Cause | What ends it |
|---|---|---|---|
| Sending | The certificate is past `not_after + expiry_grace` | R8, lease | A renewal |
| Sending | After a reboot, under `require_anchor` | R8, reboot policy | Any direct issuer exchange |
| Requesting | `receive_not_after` has passed | R6, receive freshness | A renewal |
| Sending under a lease or limits. Being paid by a payer whose certificate has a lease or limits | The clock-reset state (§5.4) | R7 and R8 need time | The wall clock returning within the tolerance, where §5.4 allows that; otherwise a re-anchor |
| Paying and being paid, among holders of the entry | The device id has an R6 block entry | R6 | The ledger unblocking the account, then a renewal above the entry's serial (§5.1) |
| Sending beyond the tightened terms, once either side holds the tier-row notice | A tier-row notice lowers a limit, or brings in or shortens a lease (§5.11) | R7 or R8 with new values | The next window; a renewal where the new lease has ended |
| Paying and requesting | A key revocation voids the certificate or the receipt | Listed exception (§3.1) | A sync |
| Paying, requesting, unloading | The wallet's own state is inconsistent (§7.2) | Listed exception (§3.1) | The repair of §7.2, where it applies |
| Paying and being paid, among holders of the entry | The bound account retired the device id (§7.2), or a fraud hold was placed on it (§5.3, §5.5) | Listed exceptions (§3.1) | Nothing for that device id. A retired id can still unload; a held id waits for the governed reinstatement |
| Paying or being paid, among holders of a forged entry | A thief of the list key signed a block entry against the device (§5.5) | Listed exception (§3.1) | A root-signed epoch bump reaching those holders |
| Paying and being paid, among wallets that hold the notice | The app's highest rules version is below a raised `rules_floor` (§5.11) | Listed exception (§3.1), if the owner allows a floor | An app update. No sync |
| Everything that signs | A read of the key, the journal or a marker fails, or a marker delete is not confirmed (§5.10) | Listed in §3.1 | The store answering again on the phone. No sync |

The first five rows are regulatory controls that the scheme has switched on.
The sixth is the same controls with new values (§5.11). With R6, R7 and R8
off none of the six occurs. The other rows are the exceptions of §3.1. A
recovery claim (§7.3) forces no sync.

Enrolling, loading, unloading, a Migrate, a repair, a declaration of loss, an
iPhone re-attestation and a recovery claim need the network. Each is something
the holder chooses to do. None is needed to keep paying.

### 7.2 Key and phone changes that add no risk beyond T3

These rely on a live device key and on the marker of §5.10, so they are
exactly as sound as T3. On a compromised or rolled-back phone each is the
ordinary fork of §3.

Common rules.

- Every journal the issuer accepts must extend its last acknowledged head for
  that device id. The acknowledged heads are anchored on-chain at sync (§7.1),
  because that table is the only check against a stale journal.
- A device id is retired by an on-chain action signed by the account bound to
  it, or as part of an accepted Migrate. In one transaction the registry row
  becomes `retired` and consensus emits a final block entry for the id
  (§5.1). A retired row accepts no Load. It still records and pays unload
  claims under §8.2 for RedeemSplits signed by its key. For a row retired by
  Migrate that holds only up to the redeemed total the Migrate states; a
  RedeemSplit above it is evidence (§5.3). Retirement therefore stops the id
  paying and being paid among holders of the list, and it destroys nothing: a
  retired phone that still works can unload everything it holds. The rule is
  for a declaration of loss that turns out to be wrong.
- A wallet never deletes its device key, its journal, or the marker its
  journal head names, on its own reading of the phone. It deletes a marker
  only where §5.10 says so, and never while it is unavailable or inconsistent
  (below); a marker that looks left over may be the one a repair needs. It
  deletes an old key and old files only in the two places named below, each
  after a final on-chain fact.

**The wallet's own state.** A wallet has three parts on the phone: the device
key, the journal and the marker (§5.10). A read of a part gives one of three
results. Present. Absent: the store answered that the item does not exist.
Error: any other answer, including a locked store. An error is never read as
absent.

| Condition | Evidence | Who decides | What the wallet does |
|---|---|---|---|
| Unavailable | A read of any part returned an error | The wallet, at each open and each use | Signs nothing, creates nothing, deletes nothing. It reads again at the next unlock or launch. No length of time turns this into another condition |
| Inconsistent | Every read answered present or absent, and one of these holds: the journal is absent or fails its own verification while the device key is present; the device key is absent while a journal is present; the head names a marker that is absent (§5.10); the issuer shows a transition signed by this device key that the journal does not contain | The wallet. In the last case it decides on the transition and its own signature on it, not on the issuer's word | Creates no Request. Signs no payment, no unload and no Migrate. Keeps the key, the journal file and every marker. Still returns objects from commits below its head. It does not show the head commit's object: the journal does not record whether that object was released, and a restored copy can hold a head that never was (§5.10). Offers the repair below |
| Lost | The user states that the wallet's state is gone or cannot be used | The user, by confirming in the app; then the bound account, by signing on-chain | Enrolls a new key under a new device id. Deletes nothing until the old id's retirement is final |

- Consistent is the remaining case: all three parts are present, the journal
  verifies, and the check of §5.10 passes after §5.10 has completed any
  interrupted commit.
- §5.10 and §9 name the same conditions by what the core does. Ready is
  consistent. Waiting is unavailable. Stopped is inconsistent, as far as the
  phone alone can tell it.
- The condition is worked out at each open and each use. It is not stored as a
  verdict, so a wrong reading corrects itself at the next one.
- iPhone. The marker's keychain class, and the class the existing code uses
  for the payment key, can be read only while the phone is unlocked (Apple
  documents both). A read while locked is expected to fail with the keychain's
  "interaction not allowed" error and not with "item not found". Not
  device-tested.
- Android. The Keystore alias lookup (`containsAlias`) returns false on every
  Keystore error, not only when the key does not exist (AOSP source reading;
  not run). The wallet does not use it. It reads the key entry, which reports
  "key not found" apart from other failures. From Android 13 the platform
  error also says whether a failure is transient and whether it needs the
  phone unlocked.
- The existing attested suite does not make this distinction. Its Android key
  store uses the alias lookup
  (`kotlin/client-android/src/main/java/org/hyperledger/iroha/sdk/crypto/keystore/KagemushaAndroidHardwareAppKeyStoreV1.kt:61, 81, 164`).
  Its iPhone key store reads every keychain error as "no key" and every
  failure to open the key as a lost key
  (`IrohaSwift/Sources/IrohaSwift/KagemushaAttested/KagemushaAttestedHardware.swift:67-78`).
  The Layer A core does not copy either.
- A wallet in the inconsistent state forces its holder online for a reason
  that is not a regulatory control, and where no repair applies its balance is
  stranded. Both are exceptions for §3.1. The reason they cannot be avoided: a
  wallet that cannot show its state is the latest and pays anyway is the reset
  that the marker exists to stop. The cost of avoiding them is to accept that
  reset by an ordinary user, which the operator then underwrites (§8.4).

The flows:

- **Repair of an inconsistent wallet.** Online, and only while the device key
  is present. The wallet asks the issuer for the issuer's copy of its journal,
  which ends at the last commit the wallet uploaded. That is the acknowledged
  head, or a Request or `Refused` Outcome committed after it: a sync uploads
  those too, each with its marker salt. Without them the copy would end at a
  transition whose marker a later Request had already replaced, and the
  repair would fail although nothing was signed since the
  sync. The wallet verifies the copy as it
  verifies its own journal. It installs the copy only if the copy's head names
  a marker that is present on the phone, and it keeps the old journal file.
  The issuer checks the device-key signature on the request and records the
  request. It decides nothing.
  The rule rests on the marker, not on the issuer's record. A marker that is
  still present shows that the wallet has released nothing signed after that
  head (§5.10). The copy is then the wallet's latest state, and installing it
  cannot bring back a balance that was spent. Extending the acknowledged head
  is not enough: a journal restored from a backup extends it too.
  The repair therefore works only when nothing was signed since the last
  sync. It covers an iPhone app that was deleted and installed again, where
  the key and the marker stay in the keychain and the journal is removed
  (Apple engineers describe that survival as an implementation detail, not a
  guarantee), and a damaged journal file. Requests on record and stored
  Outcomes return only as far as the last sync uploaded them; a Payment
  presented again for one that did not return gets no answer.
  It does not work when the marker is gone. That is the case after anything
  was signed since the last sync. It is also the case on iPhone after the
  passcode was removed, which discards the marker's keychain class (§5.10),
  even if nothing was signed. The wallet then stays inconsistent and keeps
  everything. The user may leave it so, or declare the loss.
  A compromised phone gains nothing from the repair. It can keep an old marker
  alive, and it can already fork (§3).
- **Declared loss and re-enrollment.** The wallet never concludes by itself
  that its state is lost. The user says so. The cases are a new phone with no
  old phone, a phone that was reset, and an inconsistent wallet that the user
  gives up on. An unavailable wallet is not declared lost on the same phone.
  A failed read may pass, and enrolling again there deletes the old markers
  (§5.10), which would end a wallet that was only locked (F5).
  The issuer and the chain cannot check the
  statement, which is why it restores nothing. The app first shows what it
  found on the phone and what is given up: the offline balance of the old
  device id, unless that phone or its state turns up again. If the user still
  has the old phone working, the app offers a Migrate instead. The new wallet
  then enrolls a new key under a new device id (§7.1), and the bound account
  retires the old id on-chain. The enrollment does not wait for the retirement
  unless the per-user device cap requires it (§8.1).
  The wallet deletes the old key and the old files only after the retirement
  is final and the user has confirmed a second time. That deletion is tidying.
  It has no security function.
  If the old phone or its state turns up later in the consistent state, it can
  unload its whole balance (common rules). It cannot pay holders of the list.
  Whoever holds the account key can retire a live device id. That stops the
  phone paying and being paid among holders of the list and takes nothing from
  it. A thief of the account key already receives every unload (§7.1).
- **Migrate** (replacement or key rotation, both keys alive).
  1. The new device enrolls and holds its certificate and receipt.
  2. The old wallet syncs. It then asks the issuer whether a Migrate to the
     named new device id would be accepted. The issuer makes every check of
     step 4 except the Migrate's own signature, and answers. On a refusal the
     old wallet signs nothing. The step is there so that the old journal is
     not closed by a transition the issuer then refuses.
  3. The old key signs `Migrate` as its terminal transition, naming the new
     device id and the whole balance. The new key signs the same request. The
     old wallet does not sign while one of its own Requests is still open for
     new payments (§5.2). The Migrate is committed like any signed object and
     is sent to the issuer by either phone. Until the issuer has it the
     balance is in neither wallet. The old wallet keeps its key and journal
     and sends the Migrate again at every open.
  4. The issuer accepts it only if the registry binds both device ids to one
     account, the new key has signed the same request, the old journal extends
     the acknowledged head and replays, and neither id is under a fraud hold.
     It then retires the old id, records the succession and the Migrate's
     redeemed total in the registry, and countersigns. It keeps the
     countersigned Migrate and returns it to the new device key on request,
     with no time limit.
  5. The new wallet folds the countersigned Migrate exactly once
     (`MigrateFold`).
  6. The old wallet deletes its key once it has verified the countersignature
     and the issuer has shown the retirement final. The balance has then moved
     by a terminal, countersigned transition, and the old key has no further
     use. Until it is deleted, whoever controls the old phone can sign above
     the Migrate. That is evidence, and evidence against a retired id counts
     against the live id at the end of its succession chain (§5.3, §8.2).

  What a Migrate leaves behind. SendSplits with no stored Outcome are
  abandoned: a `Refused` Outcome that arrives later cannot be refunded,
  because the RefundFold would sit above the Migrate. The app lists them with
  their total before the user confirms. A payment the receiver credited is
  unaffected. RedeemSplits signed before the Migrate stay payable (common
  rules). The old phone still returns stored Outcomes. A Payment for one of
  its expired Requests that arrives after the Migrate gets no Outcome, so its
  payer gets no refund. That is the undelivered payment of §3 item 4; the
  Migrate makes it permanent.
  Under B2 the migrated amount is whatever the journal claims. A governed
  reinstatement (§5.5) records no succession.
- **iPhone whose App Attest key died** while the payment key and journal
  survive: one online re-attestation. A new App Attest key attests a transcript
  holding the device id, payment public key and head; the payment key signs the
  same transcript. Device id, journal and balance are unchanged; the
  certificate serial advances. It shows a live instance of the app and
  possession of the payment key, not the same device. Rate-capped. A normal
  sync carries an assertion, not a new attestation. The wallet asks for this
  only in the consistent state, that is, when its journal head names a marker
  that is present. Without that check the path is a reset an ordinary user can
  perform: sync, back up, pay, restore the backup to the same phone, which
  kills the App Attest key and returns the journal and the payment key, then
  re-attest and hold the old balance again. The wallet treats the App Attest
  key as dead only when the platform reports the key itself as invalid, not on
  a failure to reach Apple or any other error. The re-attestation deletes
  nothing, so a wrong trigger costs one rate-capped call.
- **Limits across certificates (R7).** The limit subject is the account
  (§5.4). The day and month counters belong to a journal, so to a device id,
  and not to a certificate. The issuer gives each device certificate of an
  account a share of the account's day limit and month limit; the
  certificate's limit fields are that share. For each UTC day and each UTC
  month the issuer keeps one invariant per account: over the account's device
  ids, what each id is known to have sent in the window plus the most it could
  still send in it adds up to no more than the account's limit. What the
  issuer knows about a device id is its journal up to the acknowledged head.
  Above that head it knows nothing. It must then assume that the id uses its
  whole share in every window in which its certificate can still send.
  - Renewal. Same journal, so the issuer knows the counters at the head. The
    `Recertify` carries the day and month counters of the transition before
    it. A new certificate never resets them. If the new share is below what
    the counters already show, the wallet sends nothing more in that window.
  - Moving share between two live devices of one account. The issuer raises
    one device's share only after it holds the other device's `Recertify` to a
    certificate with the lower share. A head declared in a sync request is not
    enough (§5.3).
  - Migrate. The issuer has replayed the whole old journal, so it knows what
    the old id sent. The `Migrate` transition carries the old journal's day
    and month counters. The `MigrateFold` adds them to the new journal's
    counters where the day index is the same, and separately where the month
    index is the same. The new wallet signs the `MigrateFold` at a time no
    lower than the Migrate's `device_time_ms`: for the rule of §5.4 the
    Migrate counts as the new wallet's last signed time. A clock set back on
    the new phone therefore cannot open an earlier window. The issuer then
    renews the new id's certificate with the old id's share.
  - Re-enrollment after a declared loss. The old id's journal above the
    acknowledged head is unknown. The old certificate's share stays counted
    against the account for as long as that certificate can send: until the
    end of the UTC day, and for the month limit the end of the UTC month, in
    which `not_after + expiry_grace` falls. Until then the new certificate
    carries only what was not allocated, which is nothing if the lost phone
    held the whole limit. An issuer that keeps part of each account's limit
    unallocated lets a re-enrolled wallet send at once, at the price of a
    lower usable limit before the loss.
  - Re-enrollment when the old certificate is `Never`. There is no such date.
    The final block entry stops the old certificate only among holders of the
    list. The scheme chooses one of two rules in the tier row. Never return
    the share: the account's usable limit stays reduced by the lost device's
    share. Or return it after a set period: until the entry has reached every
    receiver, the account can send up to the old share per window through the
    old phone, on top of its limit. Owner decision.

  Who checks. The wallet's core applies the counter rules. The issuer checks
  them by replay at the next sync, and it alone enforces the split across an
  account's devices. A receiver sees one certificate and its tier row; it
  cannot see the account or its other devices. The chain can check only that a
  device id is registered to the account and tier. The most that a faulty
  issuer or a stolen certificate key can give one account is therefore the
  tier row's limit times its registered device ids. A compromised phone
  ignores its counters. The receiver's tally (§5.4) is the only check on it,
  and that tally is keyed on the payer's device id, so it starts again after a
  Migrate. Each Migrate needs a new enrollment, which the device and
  registration caps bound (§8.1). The limit of §5.4 stands: among wrong
  clocks an expired certificate still sends, and the dates above move with
  it.

### 7.3 Recovery is insurance

An authentic journal need not be the latest: a journal at balance 100 verifies
even if a later transition paid that 100 away, and a dead key cannot sign that
nothing later exists. A journal copied to another phone is indistinguishable
from a journal whose key died. So "journal without a live key" and "state lost"
are the same class. On an uncompromised phone one thing shows that a state is
the latest: a marker still present on the phone. Where it shows that, the
repair of §7.2 applies and nothing needs insuring. Everything else is a claim
nobody can check, paid from a budget.

Recovery is off by default. The default scheme sets `recovery_cap` to zero, and
a lost wallet is then a lost balance (§3). An operator may switch it on. The
insurer is whoever funds `recovery_budget`; in this document that is the
operator.

Rules:

1. On-chain scheme parameters: `recovery_cap` per account per period and
   `recovery_budget` per pool per period.
2. A claim is an on-chain instruction signed by the account bound to the
   device id. One paid claim per device id. A row under a fraud hold accepts
   no claim. A row retired by Migrate accepts none, because its balance moved.
   A row retired by its account (§7.2) accepts one, like a live row.
3. A claim changes nothing about the device id. Neither filing nor payout
   retires, blocks or suspends it. The registry row keeps accepting Load and
   Unload, no block entry is emitted, and a phone that still works is
   unaffected online and offline. The rule is for the phone that is not lost
   after all, and for the phone whose account key someone else holds: a claim
   takes nothing from either.
4. Reference head H: the head of the presented journal, which must extend the
   acknowledged head; with no journal, the acknowledged head itself.
5. Amount = min(remaining cap, remaining budget, balance at H minus any outflow
   by that id above H that has reached the issuer or the chain by payout). The
   issuer computes the amount and signs it. The chain checks the account's
   signature, the issuer's signature, the cap, the budget and the one-claim
   counter. Neither can check that the balance at H still exists.
6. A journal above H uploaded by the device key before payout ends the claim:
   the wallet is in use. An optional `recovery_delay` before payout, which may
   be zero, gives time for that and for outflow above H to arrive. It suspends
   nothing. It is not a bound: under R5 no receiver has to sync within it.
7. Payout is on-chain to the bound account, booked against the device id in a
   separate recovery counter, never re-issued as offline balance and never
   counted in the row's loaded total or paid total (§8.2).
8. A claim is not final value in the sense of §3. It is refused when the cap
   or the budget for the period is used up, and it may be filed again in a
   later period. §8.4 does not apply to it.

What this costs the insurer:

- A payout takes nothing back. If the phone still works, its balance stays
  spendable and redeemable in full, and the account has been paid twice. The
  second payment is the insurer's cost. The protocol has no way to recover it.
- An ordinary user with the unmodified app can do that on purpose: claim, keep
  the phone, keep spending. One paid claim per device id and the cap per
  account per period bound it for one account. A Migrate gives the account a
  new device id and so another claim in a later period. An account costs a
  keypair unless it is bound to a verified identity (§8.1).
- The insurer should therefore plan for the whole `recovery_budget` to be
  drawn in every period. Total exposure per period is `recovery_budget`. An
  unmodified phone can take `recovery_cap` on every account in every period
  until the budget is gone.
- Honest claims draw on the same budget as false ones. When false claims use
  it up first, an honest claim is refused for that period.
- A true claim costs the pool nothing: the lost balance is never presented
  (Λ in §8.3). A false claim adds its payout to I with nothing against it.
- Whoever holds the account key can file a claim and is paid on that account.
  That takes from the insurer and not from the phone.

## 8. Ledger integration

### 8.1 Device registry, witnesses, and what they bound

A registry row per device: key, tier, bound account, attestation provenance
class (Android factory chain, Android remotely provisioned chain, iPhone
app-vouched), status (live, retired or fraud-held), loaded total, claimed and
paid redemption totals, release period and queue entry (§8.2), final redeemed
total (retired rows), recorded receipt, recovery payout counter, fee payout
total and settled-fee set (§5.8), acknowledged head and its sequence number,
anchored certificate serial, successor id, fee payouts in the current unload
window (§5.8). Registration carries
the issuer-signed authorization and is capped by a scheme-wide per-window
per-tier cap. Where the asset has an issuer-attested retail identity, the
per-user device cap is keyed on that identity. A row is never deleted: a claim
or a fee can arrive after any delay.

**Witness model — owner decision.**

- (A) *Independent verification.* Each witness verifies the platform
  attestation itself, against its own pinned roots and revocation data; checks
  that the registered key is the attested key (on iPhone, the key the attested
  transcript names and that signed it) and that the registered tier is one the
  root-signed admission policy allows for that provenance class; and signs only
  after seeing the registration final on its own node. A certificate-key thief
  then needs, per device, an attestation the honest issuer would also accept at
  that tier. On Android that is a forged attestation or a genuine phone
  compromised later. On iPhone one jailbroken phone can attest any number of
  software keys (§4), so for that class (A) limits the tier and nothing else.
- (B) *Notary.* The receipt is a transparency record, not an authorization. A
  registration gets a receipt only after the issuer has matched it to its log
  of issued certificates and a seasoning delay has passed. The delay is a
  scheme parameter, no shorter than the interval at which the issuer reconciles
  registrations against that log. Under (B) every enrollment waits that long
  before the wallet can pay or request, including the new device of a Migrate
  and a re-enrollment after state loss (§7.1, §7.2). An unmatched registration
  never gets one; it pauses registration under that key and blocks the device.

Recommended: (A), with at least one witness outside the issuer operator's
administrative control.

Under either model the witnesses also record the receipt in the registry row
(`RecordKagemushaReceipt`, §6). The ledger pays a row above its own loads only
while the row holds a valid receipt (§8.2). The rule is there so that the
registry authority key alone cannot open the unload door; it makes the witness
quorum a second lock on it.

What is bounded, stated narrowly:

- The registry alone bounds nothing offline; receivers do not read the chain.
- Accounts cost a keypair and at most a fee, so a per-account cap does not gate
  a key thief. The gate is the scheme-wide registration cap, which honest
  enrollment shares; a thief can also exhaust it to deny enrollment.
- The receipt bounds the **number of new device ids** a certificate-key thief
  can make acceptable, and under (A) requires an accepted attestation for each.
  It does nothing for ids already registered: with the holder's cooperation the
  thief renews an expired certificate and, by signing a serial above
  `dead_through_serial`, clears any block entry below the maximum serial, which
  includes every R6 entry. It does not bound value: each id carries the terms
  of its registered tier, and its creation bound is §8.3 with the block list as
  the only stopping event. If that tier permits `Never` and `Unlimited`, one
  rogue registration is unbounded.
- Witness quorum stolen alone: certified devices become payable without
  registration, escaping caps and registry-derived block entries. No value is
  created without a certified key.
- Certificate key plus witness quorum: unbounded offline counterfeit until
  root-signed revocations spread.
- Voucher key: unbacked value minted onto genuine phones. Receipts do not help.
  Mitigation: vouchers signed by the same chain-watching quorum. The voucher
  key also signs the statement that releases a load refund (§6), so its thief
  can refund a load whose voucher was issued and spent; the bound is that
  load.
- List key: no value created; honest devices can be blocked until a root-signed
  epoch bump spreads. Merge-only entries stop a forged list lifting a block.
- Registry authority key stolen alone. This is the issuer's on-chain account.
  With the checks of §6 its thief creates no value and can deny service:
  - It signs registration authorizations, so it registers rows for keys that
    no phone holds, up to the registration cap, and can exhaust the cap. Such
    a row has no certificate, so no honest receiver accepts a payment from
    it. It gets a receipt only as the witness model allows: under (A) with an
    attestation the honest issuer would also accept, under (B) never, because
    no certificate was logged. Without a receipt it is paid no more than it
    loaded.
  - It raises anchored serials, one per instruction. A row whose anchored
    serial runs ahead of its issued certificates still renews, at the next
    serial.
  - It cannot anchor a head the device did not sign, or a sequence number
    below the row's, because the chain checks both (§6).
  - It cannot retire a row without the device's signed Migrate or the bound
    account's signature. With a stolen account key as well, it retires that
    account's live phones, and each then holds a final block entry. It
    supplies a retired row's final redeemed total; set too high, that leaves
    the row's unload door as open as a live row's and no wider.
  - It cannot refund a load without the voucher key's statement.
  - It submits fee settlements but cannot forge the device signatures in
    them.
  - If §12 item 11 lets the issuer place a hold without on-chain evidence,
    the thief can freeze honest rows; that is one cost of allowing it.
  Without those checks the same key creates value: rows with no phone behind
  them unload at `unload_limit` per window each, and a refunded load is paid
  twice.
- Registry authority key plus witness quorum: rows with a receipt and no
  phone. Each drains `unload_limit` per window through unload (§8.3) until it
  is held. No offline value is created without the certificate key.
- Funding account key: its thief can only add cash to the pool. No instruction
  takes cash out of the pool except a payout.
- Root key: its thief signs notices and issuer key certificates that peers
  accept offline without the chain, and nothing revokes the root. Peers stop
  accepting the stolen key for later epochs only when they hold the
  succession to the committed next root key, which the thief cannot forge
  (§5.11). What a root compromise voids is not decided (§12 item 34).

### 8.2 Unload

- Unload verifies no balance; nothing on-chain can. A device that re-submits a
  redemption earns nothing, because the chain pays only the increase in its
  cumulative total. A rolled-back or forked device that redeems value it also
  paid away offline is paid.
- What the chain checks. To record a claim the chain checks the RedeemSplit's
  device signature against the key in the registry row, and the row's status
  as set out below. The ledger does not refuse an unload because of the
  issuer, the certificate, its lease, the phone's clock state, a key
  revocation or a stale list. Whether the wallet signs a RedeemSplit in each
  of those states is §5.4 and §7.1; the design intends that it does, so that a
  wallet which can no longer pay offline can still unload.
- Payee. An unload pays only the account bound to the device id in the
  registry. The RedeemSplit names no payee, and anyone may present it. The
  registry has no instruction that changes a row's bound account. A holder who
  loses the account key can still pay offline. Its unload payouts go to an
  account it no longer controls, and it cannot Migrate.
- **Recording.** A valid RedeemSplit is recorded in full: the row's claimed
  total becomes its `redeemed_total_after`, if higher. No parameter caps what
  a row may record, and no pool condition refuses a claim. A recorded claim is
  never reduced or cancelled. Recording by itself moves no cash, so a
  compromised phone that records an enormous claim gains nothing by its size;
  what it is paid is set by the release rule below.
- **Release.** The part of a row's claims that keeps its paid total at or
  below its own loaded total is due at once. This part is the holder taking
  back what it loaded. The part above the row's own loads is released at no
  more than `unload_limit` per unload window, in the order recorded, starting
  in the window of recording. `unload_limit` is a tier parameter; the unload
  window is a scheme parameter measured in ledger time. A row is released
  above its own loads only while it holds a valid recorded receipt (§8.1).
  After a witness-key revocation the witnesses record a receipt under the new
  keys for every row that held a valid one, retired rows included, with no
  action by the holder. Until then the release above own loads waits; no claim
  is reduced (§12 item 43).
  Precisely: let `own = min(claimed, loaded)` and `above = claimed − own`. The
  row keeps a release period `(start, base)`. In window `w` the released
  amount is `own + min(above, base + unload_limit × (w − start + 1))`, and the
  amount due is the released amount less the paid total. A claim recorded in a
  window later than `start`, when everything recorded earlier was already
  released by the end of the previous window, starts a new period: `base`
  becomes the earlier `above` and `start` becomes `w`. Recording a receipt for
  a row that had no valid one also starts a new period, from what was released
  so far. The new-period rule is there so that unused allowance does not carry
  over: a row is released at most one `unload_limit` per window however its
  claims are split or timed. A change to `unload_limit` applies from the next
  window and never takes back what was released.
- A row's release dates depend only on that row's own claims and its tier's
  `unload_limit`. They are known when the claim is recorded and change only if
  the tier's `unload_limit` does. No cap is shared between rows, so the number
  of other rows that claim, honest or not, does not change them.
- **Payment.** Any `UnloadKagemusha` that names the row pays what is due to
  the bound account, whether or not it carries a new RedeemSplit. The holder
  need not be online for later parts. The issuer service presents each row
  that has an amount due once per window, and anyone else may.
- **When the pool is short.** The pool is short when an amount that is due
  cannot be paid from pool cash. The amount then joins the payout queue. The
  queue is strictly first in, first out. Cash that reaches the pool is set
  aside for the queue in order, and an entry is paid when the cash set aside
  reaches it. A row holds one queue entry at a time; amounts that fall due
  while it waits join when that entry is paid. The pool record keeps three
  counters for this: the total ever queued, the total set aside, and the total
  paid from the queue. The total queued less the total set aside is the
  shortfall, and it is public.
  The rule is there so that a place in the queue, once taken, is never lost:
  rows that claim later, however many, cannot pass it. While the queue is not
  empty the ledger pays no fee and no recovery claim, and Load is closed.
  Closing Load keeps new holders' cash from being spent on old claims; whether
  to close it is an owner decision (§12). Offline payments are unaffected,
  because no wallet can know that the pool is short.
- What the brake is. `unload_limit` is the operator's only brake on the rate
  at which a compromised phone takes cash through unload: one `unload_limit`
  per window for each row it controls that holds a receipt, plus, once, what
  those rows loaded. The operator has no pool-wide cap. A cap that honest and
  attacker rows share is what makes an honest row's wait depend on other
  rows, so the two cannot both be had. Across the pool the most that can be
  released above own loads in one window is the sum, over tiers, of rows with
  a valid receipt times that tier's `unload_limit`. The operator reads both
  factors from the registry and controls both, through the registration cap
  and the tier table.
- What the brake costs honest net receivers. A row that has received more
  than it loaded, such as a merchant, is paid at most `unload_limit` per
  window above its own loads, with or without any attacker. A merchant whose
  net takings per window exceed `unload_limit` falls further behind every
  window. It needs a tier with a higher `unload_limit`, and a compromised
  phone enrolled in that tier drains at that higher rate. Value in a recorded
  claim is no longer spendable offline, so a holder who wants to keep spending
  unloads one window's worth at a time. The wallet reads its row and shows the
  date of each part before the user signs.
- A row under a fraud hold accepts no Load and records no new claim. Its
  recorded claims are kept and are not paid while the hold stands. Its queue
  entry leaves the queue: no cash is set aside for it, the shortfall falls by
  its amount, and the entries behind it move up. Otherwise cash set aside for
  a held entry would keep honest rows behind it waiting while the pool has
  cash. §12 item 11 decides what a reinstatement does with the claims; an
  amount it makes due joins the queue at the back. Value the
  row already paid to others is unaffected. This is an exception to the rule
  that nothing strands a balance. It applies only to the holder whose own key
  signed the evidence. Where that key's device id was retired by Migrate, the
  hold falls on the live id at the end of its succession chain, because the
  balance moved there (§7.2). It is the only event that closes the unload
  door for a compromised phone. Without it a device proven to have forked keeps
  drawing `unload_limit` per window for ever. Evidence against a row retired
  by Migrate places the hold on the live row at the end of its succession
  chain as well, because the balance moved there (§7.2).
- A row retired by Migrate records a claim only up to its final redeemed
  total, which is fixed when it is retired: the redeemed total in the journal
  the issuer accepted. The rule is there so that a RedeemSplit signed before
  a Migrate and presented after it still pays, and so that a migrated key
  cannot go on recording claims. A row retired by its account has no final
  redeemed total. It records and pays claims as a live row does, so that a
  phone declared lost by mistake can still unload everything it holds (§7.2).
  A claim already recorded still pays.
- A row whose bound account is blocked on the ledger records claims as any
  other row. Whether the payout reaches a blocked account is decided by the
  ledger fact behind the R6 list (§12 item 9). The claim stays recorded either
  way.
- On Migrate, after any refund under §7.1, the old row's loaded total less its
  claimed total, if positive, moves to the new row; otherwise a user who
  changes phones would lose the right to take back own loads at once.
- Uploaded inbound payments give attribution, not a bound: signatures show who
  authorized a payment, not whether it was funded.

**Worked example.** The unload window is one day and `unload_limit` is 1,000.
Pool cash is 50,000 at the start of day 1. Twenty attacker rows, each with a
receipt and no loads, each record a claim of 1,000,000 on day 1. Merchant M
has no loads and records 2,500 on day 1. Holder H loaded 800 and records 800
on day 3.

| Day | Released to the 20 attacker rows | Released to M | Released to H | Pool cash after the day |
|---|---|---|---|---|
| 1 | 20,000 | 1,000 | | 29,000 |
| 2 | 20,000 | 1,000 | | 8,000 |
| 3 | 20,000 | 500 | 800 | 0, and 13,300 queued |

- M's dates are 1,000 on day 1, 1,000 on day 2 and 500 on day 3. They are the
  same with 20 attacker rows or with 2,000. That M waits until day 3 at all is
  the cost of the brake.
- On day 3 the pool has 8,000 and 21,300 is due. Suppose the attacker rows are
  presented first. Eight are paid. Twelve join the queue with 12,000, then M
  with 500, then H with 800. The shortfall is 13,300 and Load closes.
- On day 4 the eight attacker rows without an entry queue another 8,000,
  behind H. The twelve that already hold an entry cannot add to it.
- If the operator funds 13,300, the twelve entries, M and H are paid in full
  and the day-4 entries wait. If it funds 12,400, the twelve entries and 400
  of M's 500 are paid; M's last 100 and H's 800 are paid by the next 900.
- M and H are paid in full once the operator has funded the amount ahead of
  them, which is a number on the ledger. What a shortfall costs them is the
  time that takes. The 20,000 a day that the attacker rows take until they
  are held is the operator's loss (§8.4).

### 8.3 Exposure model

Let L be total loads less refunded loads, P redemptions paid excluding
recovery payouts, F fee payouts paid, I recovery payouts paid from the pool (a
separate counter), and B the backstop funded. Let C be the counterfeit that
has entered liabilities: value with no load behind it that a rule-following
wallet accepted, that was recorded as a claim, or that was charged as a fee.
Let Λ be value that will never be presented: balances on phones that are lost
or never used again, payments that were debited and never delivered, and fees
never settled.

- Pool cash is what came in less what went out: `L + B − P − I − F`.
- Liabilities are wallet balances, recorded unpaid claims and fee claims not
  yet paid. Loads created L of them and counterfeit added C. Each unit paid as
  a redemption or a fee removed one, and Λ will never be asked for. So
  liabilities that will be presented are `L − P − F + C − Λ`.
- Pool cash less liabilities is `B − I − C + Λ`. Every claim is covered if and
  only if `B + Λ ≥ C + I`.

Λ cannot be observed: under R5 a dead phone and an idle phone look the same
and a balance never lapses. The operator plans with Λ = 0, so the condition to
fund against is `B ≥ C + I`. Under the loss rule of §8.4 the operator owes
whatever part of `C + I − Λ` the backstop does not cover.

What the operator can observe on the ledger: L, P, F, I, B, pool cash, each
row's loaded, claimed and paid totals, the amounts due and queued, and the
shortfall. What it cannot observe: C, Λ, the balance of any wallet that has
not synced, and how many receivers a compromised phone has reached. The total
of recorded unpaid claims is not a measure of liabilities, because any device
key can record a claim of any size (§8.2).

1. **Counterfeit circulates through honest wallets.** One compromised payer can
   pay a hundred honest wallets, and each redeems within its own allowance.
   Per-device limits bound the rate per wallet, not the loss.
2. **Creation bound, by door, under both options.** *Receiver door*, per
   compromised device: the sum over receivers reached of that receiver's tally
   cap per window, times the windows until that receiver holds the block entry
   or judges the certificate expired. The number of receivers reached has no
   protocol bound; it is an assumption the operator supplies. A compromised
   phone that keeps renewing does not expire; its only stop is the block entry.
   B1 does not lower this number: the attacker loads a seed balance and
   multiplies it through forks absorbed by instances it controls. B1 changes
   what evidence exists. *Unload door*, per attacker-registered row that holds
   a receipt: `unload_limit` per window until the row is under a fraud hold,
   plus, once, the row's own loads if it also paid them away offline. *Fee
   door*, per attacker payer row: `fee_limit` per window, to a beneficiary in
   the root-signed table (§5.8). *Recovery*, where enabled: `recovery_budget`
   per period (§7.3).
3. **Drain rate.** No pool-wide cap exists (§8.2). The most that can leave the
   pool above devices' own loads in one window is the sum over tiers of rows
   with a valid receipt times `unload_limit`, plus fees and recovery payouts
   within their own caps. The operator can lower it only by lowering
   `unload_limit`, which slows honest net receivers by the same factor, or by
   holding rows on evidence.
4. **No on-chain early warning.** Pool cash minus `(L − P − F)` equals `B − I`
   whatever C is, so no on-chain ratio shows counterfeit. Redemptions plus fee
   payouts exceed loads only after counterfeit exceeds Λ plus every balance
   still outstanding, and with a backstop the pool runs short later still.
   Counterfeit consumes reserves backing genuine balances long before either.
   In today's code that state cannot occur at all; a redemption above loads is
   rejected.
5. **Issuer-side signal.** At a common past instant T, replay every journal
   synced after T up to T and sum the balances. Counterfeit at T is at least
   that sum minus `(L(T) − claimed(T))`. The bound stays valid, and is
   tighter, if the fees charged in the replayed journals up to T are also
   taken off the bracket. It is a lower bound only on a consistent cut: a
   ReceiveFold inside the cut pulls its SendSplit inside, and a MintFold
   inside pulls its load into L(T); a cut by device time alone can count one
   payment twice. It needs R8 on, lags one lease, and is reduced by every
   wallet not synced since T. Summing each wallet's last synced balance is
   unsound. With R8 off this signal does not exist. Every signal has a
   threshold and an action fixed in the scheme cell before load opens.
6. **Sizing.** The backstop is cash paid in ahead of a loss. It is not a limit
   on what the operator owes (§8.4). Size it to the loss the owner chooses to
   absorb without the pool running short over the response horizon (time to
   detect plus time for block entries to reach receivers) and refill after
   each recognised loss. Inputs the protocol does not provide: devices
   compromised at once (compromises are correlated: one exploit or one leaked
   attestation batch hits many), receivers each reaches per window, windows
   until blocked, the share of receivers that never refresh lists. Add the
   other sources of unbacked value: `recovery_budget`, a vendor restore tool
   that rolls back unmodified phones, stolen issuer keys, and under B2 a wallet
   bug. No empirical compromise rate for phone key stores was found. With R7
   and R8 on the result is finite only for an assumed reach.

### 8.4 Loss rule

The scheme has one loss rule: the operator underwrites. The scheme descriptor
states it. A holder never bears a loss caused by counterfeit.

- What is owed. A received payment is final value from the moment the
  receiver's wallet commits it. The operator owes its face value whenever it
  is presented, with no time limit. On the ledger that means every recorded
  claim is paid in full (§8.2). A claim is a RedeemSplit that verifies against
  the device key of a registry row. The chain cannot tell a claim for
  counterfeit value from any other, so the operator also pays counterfeit that
  reaches the ledger, through honest receivers or through an attacker's own
  rows, until the row is under a fraud hold.
- "Final" for a holder therefore means: a received payment is redeemable at
  face value whenever it is presented. `unload_limit` and a pool that is short
  can delay a payout. Nothing reduces or cancels it: no sync by anyone, no
  evidence against the payer, no block of the payer, no key revocation, no
  change of policy.
- Who pays, from what, in what order. Every payout is made from pool cash.
  Pool cash is one balance; a payout is not marked as genuine or counterfeit.
  Counterfeit that is paid out spends cash that backed genuine balances. The
  backstop is operator cash already in the pool, and it is what lets the pool
  still pay those balances afterwards. When pool cash cannot pay an amount
  that is due, the operator must fund the shortfall from its own resources
  outside the pool. The order is: holders' loads and the backstop first,
  because they are already in the pool; then new funding by the operator.
  Unload claims are paid before fees and recovery claims (§8.2).
- What "funded" means. Units of the asset that are in the pool account on the
  ledger, placed there by `FundKagemushaReserveBackstop`. A promise, a credit
  line or a balance held elsewhere is not funded. The backstop counter B is
  the total ever placed. No instruction takes cash out of the pool except a
  payout, so the backstop cannot be withdrawn.
- A minimum backstop, `backstop_minimum`, is paid in before load opens; the
  chain does not open Load below it. The operator refills after each
  recognised loss (§8.3 item 6).
- When the backstop is exhausted. The operator's obligation is unchanged.
  Meanwhile the ledger records every claim in full, releases it on the same
  per-row dates, puts what is due and cannot be paid into the payout queue in
  order, publishes the shortfall, pays no fee and no recovery claim, and
  closes Load (§8.2). No claim is cancelled or reduced. Each funding pays the
  queue from the front. Offline payments go on as before.
- What the ledger cannot do. It cannot make the operator fund. If the operator
  does not, the queue is paid in order as cash arrives and those behind are
  not paid; the rule is then first in the queue, and holders who stayed
  offline, as R5 entitles them to, are last. The rule is only as good as the
  operator's ability and duty to pay. What gives it force outside the ledger,
  such as a guarantee, capital or statute, is an owner decision (§12).
- The size of the obligation. It is the counterfeit of §8.3, which the
  protocol does not bound. Only the regulatory controls the scheme switches
  on, R6, R7 and R8, limit it, and with R7 and R8 both on it is finite only
  for an assumed number of receivers reached (§3, §8.3 item 6).
- No balance lapses, so the obligation never ends. The pool can be closed to
  new loads. It cannot be wound up, and the backstop cannot be returned, while
  any loaded value may still be presented.
- Funding by issuing new money. Where the operator is also the issuer of the
  asset, it could fund the backstop with newly issued units. Honoured
  counterfeit then becomes new supply, and its cost falls on every holder of
  the asset through dilution, not on the operator's own funds. This document
  does not assume it. Whether it is allowed is an owner decision (§12). If it
  is, the issue should be an ordinary on-chain mint into the funding account,
  so that the supply figures show it.
- The wallet shows, before the user signs an unload, what will be paid at once
  and the date of each later part, and whether the pool is short. Unloading
  turns spendable value into a waiting claim.

### 8.5 What the existing ledger code can and cannot carry

Reusable: pool key, custody transfer, the non-signing reserve account (§6),
the index pattern (its admission is marked incomplete), the Torii command
surface, the governance proposal pattern (install-once, no rotation).

Not reusable as is, under B1 as well as B2:

- Request, record, receipt and planner types embed recursive fields.
- The pool holds two counters, total top-ups and total redemptions. The
  invariant `available = loads − redemptions` is enforced in the pool record,
  every receipt and the receipt chain, and a redemption above it is rejected
  whole with no record
  (`crates/iroha_core/src/smartcontracts/isi/kagemusha/kagemusha_v1_reserve.rs:256-297, 1986-1996`).
  §8.2 to §8.4 need a redemption above loads to be recorded and paid. That
  adds to the pool record a backstop counter, fee and recovery payout
  counters, and the three payout-queue counters, and it changes the receipt
  layout and those checks.
- New state with no counterpart today: the device registry with the row
  fields of §8.1, the scheme cell, the fee policy table, the block index with
  its account-to-device index, and the per-row settled-fee set. The scheme
  cell and the fee policy table are replaced or extended over time, which the
  install-once governance pattern does not allow.
- None of the instructions of §6 exists. The existing instructions are the
  top-up and the redemption of Recursive V1
  (`crates/iroha_data_model/src/isi/kagemusha_v1.rs:2853, 2863`).
- On a non-boundary block, a load that writes a top-up receipt under the
  existing witness tag gives a non-zero top-up count with no attestation flag,
  and the node enters recovery. So does any write under that tag that does not
  decode as the existing receipt type, which covers a reshaped unload, claim or
  backstop receipt. A new tag avoids both.

Under B1 the existing top-up path stays and gains the missing mint-credit
producer.

## 9. Implementation shape

- **One wallet core in Rust**, sans-IO. Two pure steps: `prepare(state, input)
  → (pending, bytes to sign, marker name)` and `finish(pending, signature) →
  (commit record, messages)`. `finish` verifies the signature under the device
  key and normalizes it to low-S. The core drives the steps of §5.2 through
  three traits the platform implements: `Signer`, `Store` and `Markers`.
  Messages are returned only after the commit has succeeded and the previous
  marker is confirmed absent. The issuer's replay and `iroha_core` use the same
  crate.
- `Markers` has four calls: create, read, delete and list. Read returns
  present, absent or unknown; list returns the names or unknown. The platform
  maps its own return values as the table in §5.10 says and never reports
  absent for an error. The core, not the platform, decides what follows.
- Recovery (§5.10) runs inside `open`. It runs again before `prepare` whenever
  the previous marker step did not end with a confirmed result. No other entry
  point signs or releases.
- `open` and `status` return one of three states: ready, waiting or stopped,
  with the reason. Only ready permits `prepare`. In the waiting state every
  call that would sign returns "try again" and changes nothing. The core never
  deletes the device key or the journal on its own. §7.2 names the two places
  where an old key and old files are deleted: after a declared loss and after
  a Migrate. The core does each only as a separate call that the app makes,
  after the final on-chain fact §7.2 requires. Neither needs the stopped
  state.
- One process at a time. `open` takes an exclusive lock on the journal, and
  the journal stays open for the life of the process.
- Failure contract. Every error before the commit leaves the wallet's state
  unchanged; a marker created for the failed attempt is removed by recovery.
  After the commit nothing is thrown. A call then ends in one of two results:
  released, or committed and not yet released because the previous marker's
  deletion is not confirmed. `open` and `status` return every object that is
  committed and not yet released, once the wallet is ready. They also return
  every committed SendSplit without a stored Outcome and every RedeemSplit
  without a ledger receipt, so they can be presented again.
- Under B1 two shapes are open, and Q0 chooses between them (§2.3).
  - Proof before the commit. The proof of a transition the wallet releases is
    produced before the commit. That cannot meet the 2 s target unless proving
    takes a fraction of a second. The one exception is ReceiveFold: it commits
    after verifying the payer's proof, in state `unproven`; the amount is
    reported apart from spendable balance; the next send proves pending folds
    first. "Unproven received value" is a measured target.
  - Proving off the payment path. The Payment carries a proof made beforehand
    and the signed SendSplit; nothing is proven while the two people wait. A
    wallet then cannot make a second proof-carrying payment until its new
    state is proven, and received value is spendable only after the receiver's
    fold is proven.

  Under either shape the steps of §5.2 and the marker rules are the same. A
  proof is attached to an object; it is not part of the marker step. A
  received payment is final at the receiver's commit under either shape. What
  B1 delays is the receiver's own next offline spend, by its own proving
  time. An unload does not wait for a proof (§2.3, "Redemption without a
  phone proof"). The second shape needs one more property that only the
  relation can give: whatever the receiver's native check accepts can be
  proven. Without it a receiver holds committed value it can never spend
  offline, and that value can leave only by unload (§2.3, §3.1).
- Public API: `open`, `enroll`, `load`, `request`, `preview`, `pay`,
  `handle(message)`, `unload`, `sync`, `migrate`, `status`. `unload` takes no
  payee: an unload pays only the account bound to the device id in the
  registry (§7.1).
- Test mode: a separate testing artifact with software keys, an in-process test
  issuer and ledger, and a test scheme with its own root key. Its `Markers`
  implementation can return unknown, lose a creation and undo a deletion on
  command, so every row of the crash table in §5.10 is an automated test of
  the core. On iPhone the artifact uses a different bundle identifier, because
  before iOS 27 Apple's attestation cannot distinguish builds of one App ID.
  No build signed under the production App ID contains a software key store or
  a software marker store.
- Add a bridge-level feature so a wallet build can exclude the prover while B
  is open.

## 10. Platforms

### 10.1 First cut

iPhone and Pixel. Samsung and other brands' Google-certified builds are
expected to chain to Google roots, but no such chain has been captured for this
project; each is a claim only after a captured chain and a passing rollback
test.

### 10.2 Not in the first cut

- **HarmonyOS NEXT**: does not run Android apps natively. Needs an ArkTS shell
  over the Rust core, a HUKS attestation verifier and a pinned root identified
  from a real device.
- **Huawei EMUI / HarmonyOS 2–4 and mainland-China Android ROMs**: nothing
  captured shows what these devices return. A device with no chain to a pinned
  root is unsupported, not "lowest tier".
- **Meizu**: announced in February 2026 that it had suspended in-house
  hardware development of new domestic phones; no primary information on its
  key attestation was found.

This narrows R2 and needs the owner's agreement.

### 10.3 Verifier and client changes

- Anchor on Google's root public keys, not certificate bytes; classify factory
  versus remotely provisioned chains as Google's reference verifier does;
  ignore expiry only for factory chains; never check leaf validity; replace the
  exact version-pair set. Decide the boot-key rule from a captured production
  chain per device.
- The Layer A verifier accepts no operator-configured Android roots. Nothing is
  removed from the existing suite (§11).
- Tier Android by chain class and patch level, not by StrongBox. Tier iPhone
  by the distribution extension, not by environment alone; until that
  extension is shown unforgeable on jailbroken legacy hardware, iPhone limits
  are no higher than the lowest Android tier.
- iPhone enrollment and re-attestation require a payment-key signature over the
  transcript.
- The app is opted out of Mac availability and the verifier accepts only iOS
  attestations.
- Google's update commitment for Pixel 6 ends in October 2026. Decide whether
  patch floors apply to it.

### 10.4 Minimum physical tests before a support claim

Per Android device: capture TEE and StrongBox chains; the rollback script
(clear data, reinstall, backup-manager restore, the vendor's backup and clone
tool, second profile, OS update); unlocked-bootloader negative; forced
power-off immediately after a payment is displayed; signing with a
caller-supplied digest; latency; the time tests of §5.4; one QR payment each
way against every other platform. Per iPhone: a production-environment build;
reinstall, offload, restore; forced power-off immediately after a payment is
displayed; the time tests of §5.4; the Mac negative; assertion latency and size;
signing a caller-supplied non-SHA digest with the Secure Enclave key; one
enrollment from a mainland-China network.

Tests for §4.1, none of them run:

- Pixel 6 and one phone per vendor: generate an ordinary TEE key limited to one
  use, with an attestation challenge, and record whether the limit is listed
  as hardware-enforced. The repo's probe never did this.
- Per Android vendor: back up with the vendor's tool, pay, restore onto the
  same phone; list the Keystore aliases before and after; the wallet must
  stop (§5.10). On a Pixel, the same with a package rollback, run with
  and without the manifest settings of §5.10, and with the local backup
  transport. In each case the wallet must not come back with an older balance
  and a live key.
- Kill the process, and separately force a power-off, between each pair of
  marker steps and just after a payment is released. Start the app; then
  repeat with an older backup restored before the app starts. The wallet must
  recover or stop, never reset, and a power cut alone must never leave the
  named marker missing.
- Time one marker creation and deletion on each platform. It is on the payment
  path (§2).
- iPhone: restore an encrypted backup to the same phone without erasing; erase
  then restore; iCloud restore; migration to a second phone; restore of one
  app's data with a desktop tool; removal of the passcode. Record what happens
  to the journal, the payment key and the marker.
- iPhone counter, only if co-signing is reconsidered: 1,000 assertions in a
  row to see whether the count steps by exactly one; and, on a jailbroken or
  research device, an attempt to produce two assertions with the same count.

Further tests, none of them run. They are listed by the section
that needs them.


Finality and trust (§3.1, §4):

- Per Android vendor, for a TEE key and again for a StrongBox key: generate a
  key limited to one use with an attestation challenge and record whether tag
  405 is in the hardware-enforced or the software-enforced list.
- Where the limit of one is hardware-enforced: begin two signing operations on
  the key before finishing either and count the signatures produced, calling
  KeyMint below the keystore service on a rooted or engineering build of the
  same model.
- Where the limit of one is hardware-enforced: install a security-patch update,
  upgrade the key, and record whether the pre-upgrade copy of the key still
  signs.
- iPhone: read the marker item while the phone is locked and again after a
  reboot before first unlock; record the exact keychain error code and confirm
  it differs from the code for an item that does not exist.
- iPhone: remove the passcode with an enrolled wallet; record separately what
  happens to the marker, the journal, and the Secure Enclave payment key
  created with and without an access-control flag; then set a passcode again
  and record whether anything returns.
- iPhone: back up, pay, remove the passcode, restore the backup to the same
  phone; the wallet must end inconsistent and must not offer the old balance.
- Android: with the device key and the marker key generated without a
  user-authentication or unlocked-device requirement, remove the secure lock
  screen, enroll a new biometric, and apply a device-administrator lock reset;
  both keys must still sign after each.
- Two phones: present a Payment to a receiver after its Request has expired,
  after the receiver has rebooted, and after a long idle period; the receiver
  must answer `Refused` each time and the payer must be able to fold the
  refund. Present a credited Payment again after the same three events; the
  answer must be `Credited` each time.
- Forced power-off immediately after a payment is released, then a second
  payment after restart: record whether the phone ever signs two transitions at
  one sequence number. This decides whether an honest phone can bring a fraud
  hold on itself.

Ledger (§5.8, §6, §8):

- Per platform: time a renewal end to end, from the sync request to the
  committed Recertify, including the wait for the serial anchor to be final on
  the ledger. Run it in the foreground and in the background, under each
  user-authentication mode of 5.9. It must finish inside one authentication
  window; record how often it does not.
- Per platform: with a fee policy in the certificate, measure the Request and
  Payment sizes and the QR frame count and time per pass (estimates:
  certificate +70 B, SendSplit +65 B). Compare with the no-fee case used for
  the two-second figure.
- Per platform: a refused payment with a fee. Kill the app, and separately
  force a power-off, between scanning the `Refused` Outcome and the RefundFold
  commit. After restart the balance must be back by the amount plus the fee
  exactly once.
- Fee formula agreement: run the same boundary vectors (amount 0, 1, the amount
  at which the ceiling binds, the largest amount, a rate that gives a
  fractional result) through the iPhone build, the Android build and the node.
  All three must give the same fee.
- Per platform: sign a RedeemSplit and kill the app, and separately force a
  power-off, before the ledger accepts it. On restart the wallet must list it
  and present it again, and the ledger must pay the increase once.
- Per platform: unload with an expired lease, in the clock-reset state, after a
  key-revocation notice, and with a stale block list. The wallet must sign the
  RedeemSplit and the ledger must record it in each case (8.2 states the ledger
  refuses none of them).
- Per platform: before an unload is signed, the wallet shows what is paid at
  once, the date of each later part, and whether the pool is short, read from
  the registry row. With the ledger unreachable it must sign nothing.
- If the head anchor uses a separate device-signed sync statement (section 6):
  time that extra hardware signature per sync on each platform and confirm how
  it behaves when the user-authentication window has run out.

Marker, exchange and evidence (§5.2, §5.3, §5.10):

- First sequence of §5.10, per platform: copy the files at S0; commit a Request
  or payment; kill the app before the marker deletion; start and make a later
  payment; restore the S0 copy. Pass: the wallet stops; the S0 marker is absent
  from the key-store listing after the second start.
- Second sequence of §5.10, per platform: commit an object and kill before the
  deletion; copy the files; restore the pre-object copy and pay a second
  phone; restore the copied files. Pass: the wallet stops; no second object at
  the same sequence number can be produced.
- Third sequence of §5.10, per platform: receive a payment and refuse it
  (expired Request, and separately date set ahead under R8); payer refunds;
  restore the receiver's files from before the refusal; present the same
  Payment. Pass: the receiver's wallet stops and credits nothing.
- Read mapping, Android 12 and later, per vendor: confirm `KeyStore.getKey`
  returns null for a deleted marker and throws when the key-store service fails
  (inject by killing keystore2 on a debug build); confirm `containsAlias`
  returns false in the same failure; confirm `aliases()` returns sorted order
  and includes the device key alias last.
- Read mapping, iPhone: read, delete and list the marker class while unlocked,
  while locked (background task), before first unlock after reboot, and during
  app prewarming. Record every OSStatus. Pass: no case returns
  errSecItemNotFound for an item that exists.
- iPhone passcode: remove the passcode, record what a marker read, a listing
  and LAContext return; set a passcode again and repeat. Also change the
  passcode without removing it. Record what happens to the Secure Enclave
  payment key in each case.
- Key-store durability, per platform and vendor: create a marker and force
  power off within 100 ms of the call returning, 100 times; the same for a
  deletion; the same after three payments in quick succession. Record whether a
  returned creation or deletion is ever lost, and whether a later write is ever
  kept while an earlier one is lost.
- Crash table of §5.10 on real devices: kill the process and, separately, force
  power off after each of steps 2 to 6 of §5.2; then start plainly, start after
  restoring the pre-object files, and start after restoring older files. Pass:
  the state the table gives in every cell.
- Waiting state: lock the iPhone between the commit and the marker deletion;
  confirm the Payment is not released, nothing is deleted, and the payment
  completes after unlock. On Android, the same with a simulated key-store
  error.
- Stopped state is not sticky: restore older files (wallet stops), then restore
  the current files; the wallet must be ready again with nothing lost.
- File edit: change the balance, remove a stored `Refused` Outcome, and
  truncate the last commit in a copy of the journal, restore each; the wallet
  must stop. Replace the block list with an older signed list; the wallet must
  refuse a list older than the head commit names.
- Timing, per platform: marker creation (symmetric key in TEE on Android;
  keychain add on iPhone), deletion, single read, listing with 1, 2 and 10
  entries, and the start-time state-digest check with 100, 10,000 and 100,000
  Requests on record. Creation in parallel with a hardware signature.
- Endurance: 10,000 marker creations and deletions on one phone per platform;
  record latency drift, key-store database size and any failure.
- Core tests in test mode (no device): the `Markers` fake returns unknown,
  loses a creation, undoes a deletion and returns a short listing at every
  step; every cell of the crash table and every branch of R1 to R6 is an
  automated test.
- Installation leftovers: delete and reinstall the app on iPhone (keychain
  items persist); enrol again; confirm markers from the earlier installation
  are left in place while its key is still in the keychain, and are deleted
  only where §7.2 deletes that key.

Lifecycle and user authentication (§5.9, §7):

- Per platform: read the device key, the marker and the journal (a) with the
  phone locked, (b) after a reboot before the first unlock, (c) on Android with
  the Keystore service failing or restarted. Record the exact error codes. The
  wallet must report 'unavailable', sign nothing, delete nothing, and resume
  unchanged after unlock.
- Android, per vendor: confirm on a device that `KeyStore.containsAlias`
  returns false under an injected or naturally occurring Keystore error while
  `getKey`/`getEntry` separates KEY_NOT_FOUND from other failures; on Android
  13+ record `isTransientFailure` and `requiresUserAuthentication` for each
  failure seen.
- iPhone: read the passcode-class marker item and the
  WhenUnlockedThisDeviceOnly key item while locked from a background launch;
  record the status code (expected errSecInteractionNotAllowed, not
  errSecItemNotFound).
- iPhone: delete the app and install it again with (i) nothing signed since the
  last sync: the repair of §7.2 must return the wallet at the same balance;
  (ii) one payment or one Request since the last sync: the repair must be
  refused and the wallet must stay inconsistent with key and marker untouched.
  Record whether the payment key item and the marker item survive app deletion
  on each supported iOS version.
- iPhone: remove the passcode with a funded wallet. Record what happens to the
  marker item, the payment key and the journal. The wallet must report
  'inconsistent', keep key and journal, and offer only the declaration of loss.
  Set a passcode again and confirm nothing changes and nothing is deleted.
- Repair negative tests, both platforms: restore an older backup after syncing
  a later state (repair must return the later state, never the older one);
  restore an older backup after an unsynced payment (repair must be refused);
  kill the app between marker creation, journal commit and marker deletion,
  then delete the journal file and run the repair (must return a state whose
  marker is present, and no released object may be lost).
- Marker clean-up versus repair: with the journal file removed and the head's
  marker present, start the app repeatedly; confirm §5.10's clean-up never
  deletes that marker while the wallet is unavailable or inconsistent.
- Android: with the recommended key settings (no user-authentication
  requirement, no unlocked-device requirement), remove the screen lock, change
  it, and have a device administrator reset it; the device key and the marker
  key must still sign. Repeat with a user-authentication-bound test key and
  record KeyPermanentlyInvalidatedException, to document the bound-key cost. On
  Android 12 to 14 confirm no wallet key carries the unlocked-device
  requirement.
- iPhone, only if the bound key is considered: create Secure Enclave keys with
  userPresence, devicePasscode and biometryCurrentSet; remove the passcode,
  re-enroll biometrics, set a passcode again; record whether each key still
  signs.
- Prompted use: time the platform prompt (BiometricPrompt, LAContext) from
  display to success on each floor device; confirm a background renewal
  (Recertify) and a receive (Request, ReceiveFold, Outcome) complete with no
  prompt; confirm behaviour with no screen lock set under both tier settings
  (plain confirmation, or refuse to pay until a lock is set).
- Migrate interrupted at every step (kill and forced power-off): after the
  pre-check; after the Migrate is committed and before upload; after upload and
  before the countersignature; after the countersignature and before the old
  key is deleted; before MigrateFold. The balance must be in exactly one place,
  nothing may be destroyed, and each case must resume. Also: the issuer refuses
  at the pre-check and the old wallet must remain fully usable.
- Migrate and limits: spend part of the day and month limit, Migrate, and
  confirm the new wallet's remaining allowance equals the old one's; repeat
  with the new phone's clock set one day back and one month back (the
  MigrateFold must not open an earlier window).
- Re-enrollment and limits: declare a loss with limits and a lease on; confirm
  the new certificate carries none of the old share until the end of the UTC
  day and month in which the old lease plus grace ends, and the full share
  afterwards.
- Declared loss: confirm the old key and files are untouched until the
  retirement is final on-chain and the user confirms a second time; then bring
  back the 'lost' phone (never deleted) and confirm it can unload its whole
  balance from a retired device id and cannot pay a holder of the list.
- Renewal with a lost response: repeat the request and confirm the same
  certificate (same serial) is returned; lose the final acknowledgment of the
  Recertify and confirm the next sync brings the acknowledged head up to date.
- iPhone App Attest: provoke `DCError.invalidKey` and `serverUnavailable`
  separately; the wallet must ask for re-attestation only on the former, and
  only in the consistent state.

Time, limits, block list, versions (§5.4, §5.5, §5.11):

- Each row of the worked-cases table in 5.4 on Android and iPhone: record the
  signed time, the window used and the floor after each step. Row 1 is the
  sequence: idle four days, reboot, date set back, receiver correct.
- A Request dated just under and just over window_future_tolerance ahead of the
  payer's clock: paid and refused respectively, with the payer's floor checked
  after each.
- Twelve payments in a row to receivers each dated W ahead of the payer's time:
  the payer's floor never passes its clock reading plus W.
- A payment just before and just after UTC midnight and a month end: the
  payer's counters and the receiver's tally are in the window of the signed
  time; the receiver refuses above one limit per window.
- A wallet in the clock-reset state under a certificate with no time-dependent
  control pays and creates marked Requests; a payer with limits refuses the
  marked Request with nothing signed; a payer with none pays it.
- Date set ahead, one Request created, date corrected: the wallet enters the
  clock-reset state; record when sending resumes without a sync and that a sync
  with Recertify resumes it at once.
- The date check: a wallet unanchored with the clock more than 30 days ahead of
  its floor prompts once, then signs.
- Opening counters: after a renewal, after a Recertify with the floor ahead of
  issuer time, after a Migrate, and after re-enrollment following state loss,
  the first payment starts from the certificate's opening values.
- Under require_anchor on iPhone: how often the shell reports a reboot when
  none happened, over a week of normal use (the shell treats any doubt as a
  reboot).
- Clock error after long power-off and after months without network time, per
  device: the measured disagreement between phones sets
  window_future_tolerance.
- Version choice: two builds with no rules version in common sign nothing and
  debit nothing; a SendSplit under a version outside the Request's range is
  answered with a signed Refused and refunded; an object with an unknown
  critical extension is not paid.
- Tier-row notice by peer relay: a receiver holding a stricter row refuses a
  payer whose counters exceed it, attaches the notice to the Outcome, and the
  payer refunds and applies the row to its next payment.
- Root succession by peer relay: a phone holding only the old root receives the
  succession notice and issuer key certificate from the other phone, verifies
  the chain and completes a payment in the same meeting. Record the extra scans
  and time.
- Request with key notices attached (about 0.9 to 1.2 KB): frame count and scan
  time against the plain Request.
- Block list: an entry covering a certificate serial; renewal above the serial
  after the ledger clears the flags; a wallet holding an entry on itself
  creates no Request or does not pay; lookup time and storage with 10,000
  entries.

Decision B measurements (§2.3):

- Q0b carrier throughput: payloads of 0.9 KB and 7.4 KB, each direction, each
  pair of Pixel 6, one low-memory Android phone and one iPhone, on the repo's
  QR framing at 5 and 12 frames per second, a denser QR framing, NFC where the
  pair allows it, and each radio carrier §5.6 keeps; time from first frame or
  tap to complete decode, second passes included; median and p95 over at least
  100 transfers.
- Q0b hardware signing latency: one P-256 signature by the device key under
  each authentication mode §5.9 keeps; Android TEE key and StrongBox key;
  Secure Enclave key on iPhone; median and p95 over at least 100 operations.
- Q0b signing a caller-supplied 32-byte digest that is not a SHA-2 output:
  Android DIGEST_NONE on TEE and StrongBox per vendor; Secure Enclave digest
  overload on iPhone. Settles whether the algebraic-digest relation choice is
  open on each phone.
- Q0b marker step latency: one marker creation and deletion with the durable
  commit between them, per platform (Android Keystore key generation and
  delete; iPhone keychain item).
- Q0b exchange without a proof: Request, Payment and Outcome with real hardware
  signatures, marker steps and a carrier, timed from the payer's confirmation
  to the receiver's credit. This is the baseline every option spends.
- Q0b background execution: a stand-in computation sized to the proving and
  memory targets, started after a payment; record whether it completes with the
  app in the background, screen locked, in low-power mode and under memory
  pressure, and after how long the platform suspends or ends it.
- Q0b one-use keys: generate an ordinary TEE key limited to one use with an
  attestation challenge and record whether the limit is hardware-enforced (the
  repo's probe exits before generating it,
  `AndroidKeyMintSingleUseProbeV1.kt:79-81`); repeat for StrongBox on each
  vendor; where hardware-enforced, begin two operations before finishing
  either, and repeat after a security-patch update.
- Q0b, if the owner allows it as an option: the proof-sized payload (about 6.5
  KB) crossing before the payer confirms, to see whether the receiver can scan
  the payer's screen during the confirmation step.
- Q2 addition: time from a committed transition to its proof being ready on one
  phone (the wait before paying again or spending received value), p95.
- Q2 addition: verification time of an incoming proof on each floor device,
  p95, measured separately from transfer.
- Q2: proof completion failures in foreground, background and under memory
  pressure, against the proposed target of zero.
- Q3 addition for the proof-made-beforehand shape: every per-constraint
  mutation is also run against the receiver's native last-hop check; the native
  check and the circuit must reject the same inputs.

One design (§11):

- Profile isolation, on each platform, over QR and over NFC where the pair
  supports it: show a wallet build that holds Layer A value a V1 profile-1
  Request, Payment and Acknowledgement, and an attested-suite message under
  profile code 2 and under the text prefix `kga1:`. Each must be rejected
  before the payload is decoded, with nothing committed and nothing signed.
  Repeat in reverse with a V1 build shown Layer A messages.
- Prerequisite for the §10.4 one-use test, not a new test: change the existing
  probe so that it generates and attests a one-use key even when the feature
  flag is false. Today it returns at
  kotlin/client-android/src/main/java/org/hyperledger/iroha/sdk/offline/probe/AndroidKeyMintSingleUseProbeV1.kt:79-81
  before generating a key.
- Before any removal of the device probes (§11.1 row 7): record the §10.4
  results they produced, per device, with the build fingerprint, so the
  measurement outlives the code.

Carriers and platform facts (§5.6, §10):

- QR timing, each pair of platforms and each direction: time one pass of the
  Request (about 6 frames), the Payment (about 7 frames at 0.9 KB; about 48 at
  7.4 KB) at 5, 8 and 12 frames per second; count the passes needed; measure
  from the payer's confirmation to the receiver's decode, including the time
  the receiver takes to aim at the payer's screen.
- QR density: the largest still code a floor-device camera reads reliably from
  another phone's screen at arm's length, to decide whether a 0.9 KB payment
  can be one still code instead of about 7 frames.
- NFC: time to move 0.9 KB and 7.4 KB over ISO 7816 commands Android to Android
  and iPhone (reader) to Android (card); confirm an iPhone app reads an Android
  phone's card emulation; if tap from Android to iPhone outside the EEA is
  wanted, test the reversed flow (Android payer as card, iPhone receiver as
  reader).
- Bluetooth LE, only if the owner wants the carrier: on iPhone-iPhone,
  iPhone-Android in both role assignments, Android-Android, and one Android
  phone without Google Play services, confirm a store app can advertise a
  service, connect and move 0.9 KB, 7.4 KB and 10 KB; record connection setup
  time and transfer time; record on each Android target phone whether
  getBluetoothLeAdvertiser returns an advertiser; iPhone app in the foreground.
- Google Nearby at the pinned revision: whether an iPhone and an Android phone
  connect with no common network (the README says Wi-Fi LAN only on Apple
  platforms; the checkout contains Apple Bluetooth code).
- One-use keys, per Android vendor: generate a key limited to one use with an
  attestation challenge as a TEE key and as a StrongBox key; record where tag
  405 is listed. Where it is hardware-enforced, on a rooted or development unit
  at the KeyMint interface: begin two operations before finishing either and
  record whether both sign; after a patch-level change, upgrade the key and
  record whether the old copy still signs.
- Marker read failures: on iPhone, read the marker item while the phone is
  locked and record the error returned, and confirm it differs from the error
  for a missing item; on Android, record what the alias lookup returns when the
  keystore service is unavailable or returns an error other than key-not-found
  (AOSP source returns 'absent' for both).
- Vendor backup tools against the wallet's app data, same phone: Xiaomi local
  backup, Honor cloud restore, Meizu local backup, Huawei HiSuite and Phone
  Clone (record whether the wallet is classed as a financial application and
  excluded), Samsung Smart Switch (record whether private-storage data is
  carried). List Keystore aliases before and after.
- iPhone: the desktop tool for the one-app restore test listed earlier in this
  section is iMazing (evidence appendix, section 0).

## 11. Order of work

The owner said: "there should be only one design. we need to remove other
stuff and standardize on one design". This proposal names that design and
gives every other track an end state (§11.1). It does not itself remove
anything: each removal is a separate owner decision, taken after its own
dependency check, and the removal of the proof code waits for Decision B.

**The number of tracks.** A track is a set of specifications and code that
defines its own authority for an offline payment. The tree holds three today.

1. Recursive V1 on qualified hardware. Authority is a recursive proof plus
   hardware that allows one successor per state
   (`specs/kagemusha_v1.md:181-203`). No target phone is shown to offer that
   to an app (§4.1).
2. The ordinary app profile. It uses the same proof stack with a stock-phone
   key and, as read here, a fresh issuer approval for each operation
   (`specs/kagemusha_v1_production_readiness.md:8-25`,
   `specs/kagemusha_app_owned_hardware_v1.md:25`). That reading was not traced
   end to end.
3. The attested-app suite. Authority is a device signature under an issuer
   certificate, with no proof. It landed on 2026-10-02 in commit `465f1b5920`.
   It is 26 files and 10,435 lines in Swift, Kotlin, JavaScript, Python and
   two stub Rust crates. It has no payment engine, no test and no caller in
   the repository.

Device probes and testnet probes are diagnostics. They carry no payment
authority and are not counted.

Layer A is a fourth track from the day its first code lands, because all its
wire objects are new (§5). Until a removal is approved, the tree holds one
track more than it holds today. The count goes down only by these decisions,
each the owner's:

- Replacing the attested-app suite in place with Layer A (§11.1 row 1). This
  returns the count to three. It needs no measurement, because nothing in the
  repository consumes the suite. The owner first says whether any app outside
  the repository uses it.
- Withdrawing the ordinary profile's online-control path (row 4). This needs
  the owner's statement that this proposal replaces the record of 2026-10-01,
  a migration item for the adapters that record names, and the end-to-end
  trace. It waits for Decision B, because the files this path shares with the
  ordinary circuits (row 3) have not been separated.
- Ending Recursive V1 on qualified hardware as a separate design. Under B1
  that happens when the relation of §2.3 is specified against Layer A's
  objects and the Guard for qualified hardware is retired (rows 2, 3 and 6).
  Under B2 it happens only if the owner approves the separate consensus change
  of row 2.

The end state is one track under either option: Layer A, with a proof on every
payment under B1 and without one under B2. Under B2 the recursion code then has
no consumer. Whether it is removed is a separate owner decision, and this
document does not recommend it.

**While more than one track exists.**

- A wallet build admits one payment profile. The profile code in the envelope
  (§5.6) selects it, and the decoder rejects every other code before it reads
  the payload. The rule exists so that value accepted under one track's checks
  is never credited under another's.
- Layer A does not use profile code 2 or the text prefix `kga1:` unless the
  attested-app suite's objects are withdrawn in the same change. The suite
  claims both
  (`IrohaSwift/Sources/IrohaSwift/KagemushaAttested/KagemushaPeerMessage.swift:23-28`),
  and the shared envelope registers only codes 0 and 1
  (`IrohaSwift/Sources/IrohaSwift/IrohaPeerWireV1.swift:6-11`). The rule exists
  so that no decoder can read a suite object as a Layer A object.
- No existing track is recorded as carrying production value. The recursive
  protocol is recorded as not production qualified
  (`specs/kagemusha_v1_production_readiness.md:3-6`), the node keeps a
  reject-all verifier while no release keys exist (§2.2), and the suite cannot
  make a payment. On that record no removal in §11.1 strands a production
  balance. This was read from the repository; no live network was queried.
  Value on a testnet is test value.
- The repository forbids parallel implementations in the first release
  (`AGENTS.md:26-27`), and the recursive specification allows one decoder and
  no protocol selector (`specs/kagemusha_v1.md:3-6`). Coexistence breaks both
  until the removals happen. Accepting this proposal accepts that exception
  for that period, and the owner should say so (§12).

**Order.** Steps 1 and 2 run in parallel. Step 2 and the device tests of
§10.4 start at once. The measurements of step 1 are Q0b of §2.3 and start as
soon as the owner has fixed the targets. Step 3 is the end of Q0; it waits for
the results of step 1 that the relation depends on. Step 4 is Q1, Q2 and Q3
and follows step 3.

1. **Early measurements and device tests** (§2.3, §10.4). These need no
   Layer A format, so they come first. They reuse existing code: the Android
   key probes, the iOS App Attest probe and the animated QR framing (§11.1
   rows 7 and 9). The Android one-use probe is changed first: today it exits
   on the feature flag before it generates a key
   (`kotlin/client-android/src/main/java/org/hyperledger/iroha/sdk/offline/probe/AndroidKeyMintSingleUseProbeV1.kt:79-81`).
   A result here can change §4.1, §5.10 and the targets of §2.3 before any
   code depends on them.
2. **The parts of Layer A that do not depend on the proof.**
   - The normative specification. It replaces the statements listed in §11.1.
   - The wallet core with its store, commit rule and marker (§5.2, §5.10,
     §9), and the issuer service (§6). If row 1 of §11.1 is approved, both are
     written in the two existing stub crates and no further crate is added
     for them.
   - The attestation verifiers with the changes of §10.3.
   - The time rules, the block index, the registry, receipts, evidence and
     backstop.
   - One envelope and one QR framing under a new profile code (§5.6).
   - The test artifact and test scheme of §9.
   - A finite-state model of the commit, marker and restore rules, in
     `formal/`. Those rules can fail on a crash or a restore between two
     steps, and a model enumerates such interleavings within its bounds.

   The circuit-facing surface (§2) is written behind one module boundary and
   is provisional. Provisional formats are used in a test scheme only. No
   conformance vector is published as stable, and no enrollment or balance in
   a provisional format outlives the test scheme that made it. The rule exists
   so that a later change of format never strands a balance or re-enrolls a
   real user. The cost is accepted: if the relation changes that surface, the
   code and vectors behind it are redone.

   Load and unload are built against a test scheme only until Decision B.
3. **The relation is fixed** (§2.3). This freezes the circuit-facing surface.
   Two properties of the device key are fixed when the key is generated: the
   digests it may sign (§2.3) and its user-authentication mode (§5.9). On
   Android both are attested at key generation. Both are decided no later than
   this step, and before the first enrollment outside a test scheme. Changing
   either afterwards needs a new key and a Migrate for every enrolled phone
   it affects.
4. **Proofs** on the host, then on the floor devices, then soundness
   qualification (§2.3). Decision B follows and is the owner's.
5. **Before Layer A carries production value.** These are necessary, not
   sufficient: Decision B is taken in writing (§2); the owner has replaced the
   normative statements listed in §11.1; the tests of §10.4 have passed on
   each device for which support is claimed; the two key properties of step 3
   are decided; and row 1 of §11.1 is resolved, so that one signature-only
   design exists and not two.
6. **Removals.** §11.1 gives, per row, the earliest point and whose decision
   it is. The check before each removal is a search of the repository for
   callers, a build of the workspace and of the Swift and Kotlin packages, and
   the owner's statement about consumers outside the repository. The evidence
   appendix cites several of these files by line at commit `b2a3cd05bc`; its
   citations hold against that commit and not after a removal.

No effort estimate exists for Layer A or for any removal. One is produced with
the Layer A normative specification.

### 11.1 End state of every existing track

The facts below were read from the working tree at commit `b2a3cd05bc` on
2026-10-02. Nothing was built or run. File and line counts are of tracked
files; the repository holds 916 tracked files with "kagemusha" in the path,
about 482,000 lines. KAGEMUSHA code in files without that name, for example
`crates/iroha/src/client/ordinary_native.rs`, is not counted, so the counts
are lower bounds. "Replaced" means that a named part of this proposal takes
over the function. The code stays until its removal is approved. Every removal
decision is the owner's.

| # | Component and where | Consumed today by | End state under B1 | End state under B2 | Replaced by | Earliest removal |
|---|---|---|---|---|---|---|
| 1 | Attested-app suite, signature only. 26 files, 10,435 lines. Swift `IrohaSwift/Sources/IrohaSwift/KagemushaAttested/` (9 files, 3,064 lines). Kotlin `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/attested/` (7 files, 3,149 lines). JavaScript `javascript/iroha_js/src/kagemushaAttestedV1.js` (2,424 lines). Python `python/iroha_app_attestation/src/iroha_app_attestation/attested_enrollment.py` and `attested_selection.py` (1,561 lines). Rust crates `crates/iroha_kagemusha_attested` and `crates/iroha_kagemusha_issuer` (7 files, 237 lines; a layout test, an empty vector generator, two empty tests, an issuer that exits with "service implementation pending") | Nothing in the repository. No test names it. The JavaScript package exports no entry for it. No file under `kotlin/client-android` or `kotlin/kagemusha-wallet-android` refers to it. The shared envelope does not register its profile code. Its Swift and Kotlin types are public, so a consumer outside the repository cannot be excluded from the repository alone | Replaced in place. Its wire objects are withdrawn. Kept and changed: the enrollment verifier (§10.3) and the Secure Enclave and App Attest key code (`KagemushaAttestedHardware.swift`). The two crates become the Layer A wallet core and issuer service | Same as B1 | Layer A objects (§5.1), wallet core (§9), issuer service (§6) | When the Layer A normative specification is accepted. Needs no measurement |
| 2 | Recursive proof stack. `crates/iroha_core_zk/src/kagemusha_v1_recursion/` (193 files, 151,759 lines, including row 3); `kagemusha_polynomial_store_v1` (7 files, 4,141 lines); `kagemusha_p256_curve_gadget.rs`, `kagemusha_v1_poseidon.rs`, `kagemusha_v1_crypto.rs`; the data model in `crates/iroha_data_model/src/kagemusha/` (59 files, 47,031 lines, including row 4's types) | Node block execution (`crates/iroha_core/src/smartcontracts/isi/kagemusha.rs:2617-2633`), which keeps a reject-all verifier while no release keys exist. The mobile bridge, which links the prover in every build (`crates/connect_norito_bridge/Cargo.toml:42, 49`). The Rust client (`crates/iroha/Cargo.toml:30`). Row 14. No build produces a State proof (§2.2) | Kept. The relation fixed in §2.3 is built from it and specified against Layer A's objects. Keys are generated for that relation | No consumer. Kept until a separately approved change removes it together with the consensus coupling of row 10 | Under B2, nothing: a receiver makes the Layer A checks only | Never under B1. Under B2 only as a separately approved consensus, genesis and finality-proof format change. This document does not recommend it |
| 3 | Ordinary circuits: the P-256 Guard and its State consumer. 84 files, 30,859 lines with "ordinary" in the name inside row 2's directory, for example `ordinary_guard_composition.rs` and `production_ordinary_guard.rs` | The production prover is built from them (§2.2). The production construction refuses the ordinary State (`crates/iroha_core_zk/src/kagemusha_v1_recursion/composite.rs:1961-1968`) | Kept. The per-hop relation needs their P-256 equations. The Guard is specified again against Layer A's objects; the existing one is bound to the online profile's approval objects | With row 2 | Under B1, the Guard specified against Layer A's objects | Not while B is open. Then as row 2 |
| 4 | Ordinary online-control path: an issuer approval for each operation. Spec `specs/kagemusha_app_owned_hardware_v1.md`. Code: the ordinary files of `crates/iroha_core_zk/src/kagemusha_v1_state/` (50 files, 33,042 lines); `kagemusha_ordinary_*` in the data model (24 files, 18,196 lines); `crates/iroha/src/client/ordinary_native.rs`; the route `/v1/kagemusha/ordinary/current-wallet` (`crates/iroha_torii_shared/src/ordinary_wallet_current.rs:11`); Kotlin files named `*Ordinary*` (18 files, 2,048 lines); Swift ordinary and approval files (17 files, 2,415 lines); Python `ordinary_*.py` (8 files, 1,831 lines). Which of these files serve row 3 has not been traced | The owner record of 2026-10-01 names thin BPNG, BOI and CBSI adapters (`specs/kagemusha_v1_production_readiness.md:8-25`). No adapter code under those names is in the repository | Not used: it needs the network to authorize each operation, which R1 excludes. Not traced end to end. Its enrollment half (hardware key, key attestation, Play Integrity, App Attest) continues in row 12 | Same as B1 | Layer A enrollment (§7.1) and the offline exchange (§5.2) | Not while B is open: its split from row 3 has not been traced. Then after the owner says this proposal replaces the 2026-10-01 record (§12), the adapters have a migration item and the trace is done. Under B1 also after the Guard stops using its approval objects |
| 5 | V1 native wallet: coordinator, durable state, bootstrap. `crates/connect_norito_bridge/src/kagemusha_core_coordinator_v1.rs` and its directory (56 files, 34,840 lines); the other files of `crates/iroha_core_zk/src/kagemusha_v1_state/` (49 files, 43,702 lines); the bridge's bootstrap, reserve-finality and hardware-evidence files | The Swift, Kotlin and Java coordinator bridges and wallet classes of row 13. The generic bridge installs no coordinator and returns device-unavailable (`specs/kagemusha_device_bridge_v1.md:635-644`) | Replaced by the Layer A wallet core as the owner of wallet state. Which prover-facing parts are reused is decided when the relation is fixed (§2.3) | Replaced by the Layer A wallet core. No consumer | Wallet core (§9) with the commit rule of §5.2 and the marker of §5.10 | When the Swift and Kotlin shells over the wallet core pass conformance and, under B1, the prover runs behind the new core |
| 6 | Secure-element device path. Specs `specs/kagemusha_device_bridge_v1.md`, `specs/kagemusha_device_sender_v1.md`, `specs/kagemusha_receiver_admission_v1.md`, `specs/kagemusha_guard_bundle_v1.md`, `specs/kagemusha_pixel6_ese_service_contract_v1.md`; the Apple route in `specs/kagemusha_v1_phone_algorithm.md:524-538`. Code: `crates/connect_norito_bridge/src/kagemusha_device_bridge_v1.rs` and its directory (4 files, 3,518 lines); Kotlin `KagemushaOmapiDeviceLifecycleV1.kt`, `KagemushaSecureElementApduV1.kt`, `KagemushaDeviceLifecycleBridgeV1.kt`, `KagemushaDeviceOperationCodecV1.kt`; Swift `KagemushaSecureElement*.swift`, `KagemushaDeviceLifecycleBridgeV1.swift`, `KagemushaDeviceOperationCodecV1.swift` (5 files, 4,072 lines); Java mirrors | Nothing that runs. No applet exists in the repository, both routes need access the project does not have, and stock dispatch returns unavailable (`specs/kagemusha_device_sender_v1.md:5-7`) | Not used by Layer A. R2 excludes a custom applet | Same as B1 | Nothing. Layer A has no hardware route | Candidate. Not before the host proofs of §2.3 complete or Decision B is taken for B2, whichever comes first. The decision says whether the one-successor rule and the command framing are kept for cards |
| 7 | Device probes. Android key probes in `kotlin/client-android/src/main/java/org/hyperledger/iroha/sdk/offline/probe/` (`AndroidKeyMintSingleUseProbeV1.kt`, `AndroidKeyMintOneUseSelectionCandidateV1.kt`, `KeyMintRestartDiagnosticV1.kt`, `AndroidPixel6TestnetStrongBoxObservationV1.kt`) and their device tests; iOS `examples/ios/KagemushaAppAttestProbe/` (250 lines of Swift) | The Pixel 6 measurements (`specs/kagemusha_v1_production_readiness.md:342-379`) | Kept and extended for the tests of §10.4 | Same as B1 | Not replaced | Not before the tests of §10.4 have run and their results are recorded |
| 8 | Testnet probes. `crates/connect_norito_bridge/src/kagemusha_testnet_*.rs` and `platform_jni/kagemusha_testnet_*.rs` (11 files, 5,272 lines); Swift `KagemushaTestnet*.swift` (5 files, 774 lines); Kotlin `probe/KagemushaTestnet*.kt` and `probe/Pixel6Testnet*.kt`; `scripts/prepare_kagemusha_testnet_observation_bundle.py` | Testnet diagnostics only | Not used by Layer A | Same as B1 | The test artifact and test scheme of §9 | Candidate. Same earliest point as row 6 |
| 9 | Peer transports. The envelope `IrohaPeerWireV1` (Swift, Kotlin, C#); animated QR `IrohaPeerQRV1`; NFC `IrohaPeerNfcV1` with the CoreNFC and Android carriers; Nearby `IrohaPeerNearbyV1`. Together: Swift 7 files, 9,416 lines; Kotlin 6 files, 5,051 lines. A second QR framing in Rust (`crates/iroha_data_model/src/qr_stream.rs`, `specs/qr_stream.md`). `specs/peer_transport_v1.md` | The V1 wallet classes. The envelope registers profile 1 only (`IrohaSwift/Sources/IrohaSwift/IrohaPeerWireV1.swift:6-11`) | Kept. Layer A uses one envelope and one QR framing under a new profile code (§5.6). The Layer A normative specification names the framing; the other framing is then a removal candidate. Profile 1 goes with row 13 | Same as B1 | Not replaced | The carriers are not removed. The duplicate framing: when the normative specification names one |
| 10 | Ledger. Instructions `TopUpKagemushaV1` and `RedeemKagemushaV1` (`crates/iroha_data_model/src/isi/kagemusha_v1.rs:2853-2870`); execution and reserve in `crates/iroha_core` (12 files, 9,824 lines; `smartcontracts/isi/kagemusha.rs`, `smartcontracts/isi/kagemusha/kagemusha_v1_reserve.rs`); mint-finality seals; the governed verifier registry (`specs/kagemusha_v1_production_readiness.md:124-130`) | Every node: block execution, genesis and the finality-proof format | The top-up path stays and gains the missing mint-credit producer (§8.5). Redemption, receipts and the pool invariant change shape (§8.5). The instructions of §6 are added under a new witness tag | The instructions of §6 replace both. The existing ones stay until the separately approved change of row 2 | The instructions of §6 | Under B1, only the parts §8.5 lists as not reusable, when their replacements exist. Under B2, with row 2. A consensus change either way |
| 11 | Torii routes and SDK Torii clients. `/v1/kagemusha/readiness`, `/top-up`, `/redeem`, `/operations/{operation_id}` (`crates/iroha_torii/src/lib.rs:4956-4959`); `/authority-state/{asset_definition_id}` (`crates/iroha_torii/src/kagemusha_state.rs:14`); 11 files, 4,542 lines in `iroha_torii` and `iroha_torii_shared`; clients in Swift, Kotlin, Java, JavaScript, C# and Python | The SDK wallet classes and operators | The command surface is reused (§8.5). Schemas follow row 10. The ordinary route goes with row 4 | Same as B1 | Routes for the instructions of §6 | With row 10 |
| 12 | Attestation verifiers and enrollment clients. Python `python/iroha_app_attestation/` (25 files, 7,605 lines; raw verifiers `attestation.py`, `apple_receipt.py`, `play_integrity.py`, `revocation.py`, `provider.py`). The Android hardware key store (`kotlin/client-android/src/main/java/org/hyperledger/iroha/sdk/crypto/keystore/KagemushaAndroidHardwareAppKeyStoreV1.kt`). Swift App Attest evidence and enrollment (`KagemushaAppAttestEvidenceV1.swift`, `KagemushaAppAttestEnrollmentVerifierV1.swift`) | Ordinary enrollment (row 4) and the attested-app suite (row 1) | Kept and changed as §10.3 lists. The `ordinary_*` workers go with row 4 | Same as B1 | Not replaced | Not a candidate |
| 13 | V1 wire types and SDK wallet classes. Swift (59 files, 21,172 lines outside row 1, including the Swift files of rows 4, 6 and 8); Kotlin `core-jvm` (33 files, 12,991 lines), `client-android` (36 files, 6,266 lines), `kagemusha-wallet-android` (9 files, 786 lines); Java duplicates in `java/iroha_android` (24 files, 3,484 lines with tests); JavaScript (13 files, 4,745 lines with tests); C# (16 files, 6,598 lines with tests); Python (5 files, 5,055 lines with tests) | Apps built on the SDKs. The cross-SDK fixture tests (`scripts/tests/kagemusha_hard_cut_test.py`) | Replaced by thin Swift and Kotlin shells over the wallet core (§9). There is one payment format (§2), so the V1 Request, Payment and Acknowledgement types are withdrawn. What the JavaScript, C#, Python and Java surfaces keep is not designed here | Same as B1 | The shells and the Layer A objects | When the shells exist and no consumer of profile 1 remains. The Java duplicates follow `specs/jvm_consolidation_inventory.md` |
| 14 | Release, key-artifact and qualification tooling. Specs `specs/kagemusha_v1_compact_keys.md`, `specs/kagemusha_v1_native_profile_binding.md`, `specs/kagemusha_v1_provider_policy_binding.md`, `specs/kagemusha_v1_physical_evidence.md`, `specs/kagemusha_v1_release_runner_validation.md`. Code: the KAGEMUSHA scripts and their tests under `scripts/` and `pytests/scripts/` (21 Python files, 17,851 lines); `crates/iroha_kagami/src/kagemusha.rs` and its module (3 files, 3,727 lines); the governance release schemas in Swift, JavaScript, C# and Python | The release process of the recursive stack. No release keys exist (§2.2) | Kept. The key codec and layout authentication serve the keys generated for the new relation. Provider policy and physical evidence describe an OEM hardware provider; they are re-scoped or withdrawn when the relation is fixed | No consumer. With row 2 | For a device support claim, the tests of §10.4 | Under B2, with row 2 |
| 15 | Formal model. `formal/kagemusha_v1/` (5 files, 1,763 lines, TLA+) | `scripts/tests/kagemusha_formal_proof_gates_test.py` | Kept while row 6 exists. It models one hardware-enforced successor per state (`formal/kagemusha_v1/README.md:8-13, 21-26`), not Layer A | Same as B1 | The model of the commit, marker and restore rules listed in §11 step 2 | With row 6 or row 2, whichever is later |

**The existing specifications.** Fourteen KAGEMUSHA specifications exist under
`specs/` besides this proposal and its evidence appendix. "Superseded" means
that the text must be rewritten or withdrawn if this proposal is accepted.
"Kept" means that it stays normative for a component that remains.
"Unaffected" means that nothing here changes it.

| Specification | Lines | Status if this proposal is accepted |
|---|---|---|
| `kagemusha_v1.md` | 346 | Superseded as the definition of the first-release protocol, under either option. Under B1 its recursive operations (`:53-65`) and reserve rules (`:253-286`) are the starting point for the relation of §2.3. Under B2 it is withdrawn with row 2 |
| `kagemusha_guard_bundle_v1.md` | 241 | Superseded. Under B1 the Guard is specified again against Layer A's objects. Under B2 it has no consumer |
| `kagemusha_receiver_admission_v1.md` | 110 | Superseded by the Receive and Outcome steps of §5.2, under either option |
| `kagemusha_app_owned_hardware_v1.md` | 88 | Superseded in part. Its identity enrollment is kept as input to §7.1. Its approval message and its capability boundary (`:19`) are superseded |
| `kagemusha_v1_phone_algorithm.md` | 928 | Superseded in its goal statement (`:10-24`). Its device measurements (`:501-538`) are kept as evidence. Its Claim split (`:735-844`) is kept as input to §2.3 under B1 |
| `kagemusha_v1_production_readiness.md` | 1,027 | Superseded in its current-profile record (`:8-25`), if the owner says so, and in its device gates (`:119-122`), which the targets of §2.3 replace. The rest is a dated record of work on the recursive stack and is kept as such |
| `kagemusha_v1_provider_policy_binding.md` | 147 | Superseded under B2 by the admission policy of the scheme descriptor (§5.1, §8.1). Under B1 decided when the relation is fixed |
| `kagemusha_v1_physical_evidence.md` | 241 | Superseded as the contract for a device support claim; §10.4 takes that role for stock phones. Kept while row 6 exists |
| `kagemusha_device_bridge_v1.md` | 651 | Kept unchanged while row 6 exists. Not used by Layer A. Its peer-protocol section (`:13-43`) is superseded by §5.2 |
| `kagemusha_device_sender_v1.md` | 86 | Kept unchanged while row 6 exists. Not used by Layer A |
| `kagemusha_pixel6_ese_service_contract_v1.md` | 89 | Kept unchanged while row 6 exists. Its statement that a Pixel 6 monetary profile needs an internal secure-element service (`:3-6`) is superseded |
| `kagemusha_v1_native_profile_binding.md` | 62 | Kept under B1; its layout table changes when the relation is fixed. No consumer under B2 |
| `kagemusha_v1_compact_keys.md` | 98 | Unaffected under B1. No consumer under B2 |
| `kagemusha_v1_release_runner_validation.md` | 79 | Unaffected. It is a dated validation record of the release scripts |

**Normative statements this proposal would supersede.** Accepting Layer A, with
stock-phone keys holding offline authority, contradicts each statement below.
Each has to be rewritten or withdrawn in the same change that makes the Layer A
specification normative. Unless marked, the contradiction holds under B1 and
under B2.

In `specs/kagemusha_v1.md`:

- `:3-6`. Recursive V1 is the sole first-release protocol, with one decoder and
  no protocol selector. Change: Layer A becomes the first-release protocol;
  while both exist, the profile code is a selector (§11).
- `:26-45`. One hardware-controlled private state per lane, with the balance
  private and qualified hardware holding the authoritative root. Change: the
  state is a journal the app keeps, and the receiver sees the payer's totals
  (§5.7).
- `:64-65` and `:317-319`. Rotate moves the balance to the next hardware epoch
  with no online step. Change: Migrate needs the issuer (§7.2).
- `:171-174`. Any number of payments against one Request are all accepted.
  Change: a Request is single-use (§5.2).
- `:176-179`. Request expiry is judged on the sender's trusted hardware commit
  time. Change: no phone gives a trusted time; §5.4 applies.
- `:183-194`. Offline authority requires an attested non-forking provider that
  meets nine listed requirements, the last being no software fallback. Change:
  withdrawn; §4 and §4.1 state what a stock phone gives.
- `:196-203`. A host-side signature or certificate alone grants no monetary
  authority, and stock KeyMint, StrongBox, Secure Enclave and App Attest stay
  online-only. Change: withdrawn. Under B2 a device signature under a
  certificate is the authority. Under B1 it is that signature plus the proof.
- `:230-233`. A committed amount is bound to the receiver and an exposed
  credit cannot be cancelled. Change: under §5.2 the receiver may answer
  `Refused` before it credits, and the payer then refunds.
- `:248-251`. Stable credentials and counters are not visible to a peer.
  Change: §5.7 lists what each side learns.
- `:257-259` and `:273-278`. The reserve equals top-ups less redemptions, a
  redemption pays the requested account, and a redemption above the reserve is
  rejected. Change: the backstop, recorded claims and partial payment of §8,
  and an unload pays only the account bound to the device id (§7.1).
- `:280-281`. The pool has no claims and no buckets. As read here, that
  excludes the recorded unload claims of §8.2. Change: reworded.
- `:288-296`. Four online routes with V1-only schemas. Change: routes for the
  instructions of §6.
- `:90-92`, `:300-303` and `:344-346` (B2 only). A payment carries one
  constant-size proof of at most 6,528 bytes, and host-only signatures do not
  establish offline monetary authority. Change: withdrawn under B2.

In the other specifications:

- `specs/kagemusha_guard_bundle_v1.md:3-7`. The GuardBundle is the only offline
  hardware-authority path, and a platform signature or attestation chain is no
  substitute. `:29-46`. The capability set is indivisible, and a profile
  missing any capability is online-only. Change: both withdrawn.
- `specs/kagemusha_device_bridge_v1.md:3-6` and `:635-644`. The only ABI is to
  a qualified non-forking hardware service, with no software fallback. Change:
  re-scoped to the secure-element path of row 6.
- `specs/kagemusha_device_sender_v1.md:5-7`. Stock dispatch returns unavailable
  until a qualified provider is installed. Change: re-scoped to row 6.
- `specs/kagemusha_receiver_admission_v1.md:3-6` and `:26-28`. A payment is
  accepted into a rollback-resistant hardware inbox, there is no cancellation
  protocol, and a host signature without Guard verification grants no
  authority. Change: superseded by §5.2.
- `specs/kagemusha_pixel6_ese_service_contract_v1.md:3-10`. A Pixel 6 monetary
  profile requires a provisioned internal secure-element service, and the stock
  Pixel 6 is not qualified. Change: the Pixel 6 is in the first cut with its
  stock key (§10.1).
- `specs/kagemusha_v1_phone_algorithm.md:10-24`. An ordinary app may transfer
  value offline only when a hardware primitive authorizes at most one
  successor. `:508-511`. An attested app and a StrongBox key alone cannot be
  substituted for that. Change: withdrawn; §4.1 says no target phone has the
  primitive and what follows.
- `specs/kagemusha_app_owned_hardware_v1.md:19`. App enrollment does not grant
  offline spending, and a reusable P-256 signature is not a one-use
  authorization. Change: the second half stays true and §4.1 says so; the
  first half is withdrawn.
- `specs/kagemusha_v1_production_readiness.md:8-25`. The owner requirement of
  2026-10-01: the ordinary app profile with BPNG, BOI and CBSI adapters, and
  Play Integrity required at enrollment and refresh. Change: replaced by this
  proposal if the owner says so (§12). `:351-354`. A Pixel 6 needs a
  provisioned hardware counter or another proven no-fork primitive for
  production offline money. Change: withdrawn. `:119-122`. Device gates of
  10 s proving, 1 s verification and 30 s handoff. Change: the targets of §2.3.
- `specs/peer_transport_v1.md:3-15`. Exactly three message kinds, the third an
  Acknowledgement sent after staging in a rollback-resistant inbox, with no
  cancellation kind. `:65-71`. The size table. `:78-79`. A smaller limit cannot
  be advertised as an offline-capable profile. Change: the messages of §5.2
  and the sizes of §5.1.
- `specs/qr_stream.md:13-17`. The payload is one of the three V1 values.
  Change: decided when the normative specification names one framing (row 9).

Outside `specs/`:

- `roadmap.md:94-96` (outcomes S7, S8 and S9) and `roadmap.md:151-154`, and
  `status.md:24` and `status.md:218-221`. The first production app profile
  requires genuine monetary proofs and current-owner authority, and durable
  money requires an exact-next successor and trusted time with no software
  fallback. Change: rewritten to Layer A's outcomes. "Genuine monetary proofs"
  stays under B1 and goes under B2.
- `AGENTS.md:26-27`. Parallel implementations are prohibited in the first
  release. Change: none to the text; the coexistence of §11 is an exception to
  it until the removals. `AGENTS.md:30-32`. Hardware-dependent guarantees must
  not be relabelled as software guarantees, and a KAGEMUSHA offline
  monetary-authority policy is said to follow "below". No such policy text is
  in that file. Change: the policy is written there, stating §4's weaker
  guarantees and no more.
- `formal/kagemusha_v1/README.md:8-13, 21-26`. The model assumes one
  hardware-enforced successor per state. Change: it stays the model of row 6
  only.
- `docs/bpng-retail-daily-limit-admission.md:47-50`. KAGEMUSHA top-up and
  redemption stay closed for a retail-governed asset until the owner approves a
  typed treatment. Not contradicted. It gates Load and Unload for such an
  asset (§12).
- `scripts/tests/kagemusha_hard_cut_test.py:35-40`. The names `KagemushaV2`,
  `KagemushaV4`, `KagemushaV5` and their lower-case forms are retired and
  guarded. Not contradicted. Layer A's objects cannot take those names.

**Smart cards.** Smart cards are a future option, not a plan (§1). Nothing is
built for them: the repository has no applet, this proposal defines no card
enrollment class, and no Layer A object has a card field. If the option is
taken up, a card applet would reuse four parts:

- the one-successor rule of the secure-element contract: one reserved
  predecessor, one stored outcome, the same bytes on a retry, and a conflict
  for any second successor
  (`specs/kagemusha_pixel6_ese_service_contract_v1.md:30-46`);
- the command framing: the command and response frames of
  `specs/kagemusha_device_bridge_v1.md` and the APDU transport of
  `specs/kagemusha_pixel6_ese_service_contract_v1.md:20-28`;
- the NFC carrier over ISO 7816 commands (§5.6);
- under B1, the Guard for hardware that enforces one successor
  (`specs/kagemusha_guard_bundle_v1.md:29-41`), which is the case of §2.1
  item 3.

A card would also need an enrollment class that is not phone attestation
(§12). This document's reading is that such a class would enter as a new
provenance class and tier (§8.1). That has not been designed. The removal
decision for row 6 says whether the first two parts are kept.

## 12. Decisions needed from the owner

Items 1 to 31 keep their numbers from revision 4, because other sections refer
to them. Items 32 to 57 are new in revision 5. §0 names the six to take first.
An item marked "answered" records the owner's words; what this document makes
of them is stated separately.


1. Decision B (§2): whether every payment also carries a proof. Until it is
   decided, a per-hop proof on every payment is the requirement and Layer A
   carries no production value.
2. Which relation the proof must show: the per-hop relation, or the lean one,
   which contains arithmetic bugs, makes creating value need one real load and
   gives no attribution (§2.1). See item 55.
3. The qualification targets, the budget and date of each stage, and their
   owners (§2.3).
4. Whether "no trusted setup" is a requirement.
5. The staged device list in §10, and whether this proposal replaces the
   2026-10-01 ordinary-profile record (§11.1). See item 56.
6. Defaults for R7, R8 and the reboot policy. See item 45.
7. The loss rule. This document reads the owner's statement of 2026-10-02 as
   settling it: one rule, the operator underwrites (§8.4). Open: the minimum
   backstop, and what stands behind the promise (item 41).
8. The witness model, k and n, and who holds the keys.
9. Which ledger fact defines the R6 list (§5.5). See item 44 for what a block
   on the holder's own account does to an unload.
10. Recovery insurance. Off by default in this revision (§7.3). See item 51.
11. What accepted evidence does to a device, and the reinstatement path (§5.3,
    §8.2). See item 36.
12. iPhone custody: accept app-vouched custody as in §4 with the marker of §4.1
    (recommended), or also co-sign every transition with an App Attest
    assertion (estimated 0.15–0.25 KB per payment, more on iOS 27; not
    measured). Co-signing does not stop a compromised phone if the OS sets the
    counter. One third-party source suggests it does; no test has settled it
    (§4.1). Co-signing strands the balance whenever an assertion is lost or
    Apple invalidates the key.
13. Treatment of load and unload under the ledger's retail daily limit (the DAY
    policy for retail-governed assets). The repo records both as closed for
    such an asset until the owner approves a typed treatment
    (`docs/bpng-retail-daily-limit-admission.md:47-50`).
14. Whether patch floors apply to Pixel 6.

Raised by the June 2025 document (§14):

15. The owner said on 2026-10-02: "1-2 s should be good ux". This document
    reads that as the meaning of "instant"; the end points and the percentile
    are its own reading (§1, §2.3). Open: what R9 bounds, the Payment alone or
    the whole exchange; and whether the figure is a gate for a proof-carrying
    payment too (item 55).
16. Which carriers a wallet must support. Of the carriers the repo implements,
    only QR works on every pair of phones; tap cannot work iPhone to iPhone
    outside the European Economic Area. The platforms also allow a Bluetooth
    connection between store apps, which is not implemented and not measured
    (§5.6). See item 57.
17. Answered 2026-10-02: fees are optional and are received only into an online
    account when someone syncs (§5.8). Open: who pays, the payer on top of the
    amount or the receiver out of it; one beneficiary per scheme or one per
    issuer; taxes; and the form of settlement (item 53). The schedule's shape
    is fixed with the relation (§2.3).
18. Privacy (§5.7). Is a stable wallet pseudonym, with the payer's totals
    visible to every receiver, acceptable? Is an un-identified low-limit tier
    wanted? Should the Payment be encrypted against bystanders? Item 57 asks
    that last question again, with its cost.
19. The default for R8, forced sync on or off. This is item 6 again with one
    more input. §3 proposes on. The 2025 document's summary recommended
    transaction limits only, with no holding limit and no forced sync; its
    policy section lists a balance ceiling, a forced resync and an automatic
    freeze. See item 45.
20. Optional controls this proposal does not have. The existing signature-only
    code has two: a balance ceiling, which only the holder's own app can
    enforce; and a cap on value sent since the last sync, which needs no clock
    but is a forced sync and was left out because of R5. The 2025 document
    names a third, which no code has: a cap on the number of payments.
21. One issuer per scheme, or several PSP issuers sharing one pool under a
    central-bank root. If several, who bears counterfeit made under one PSP's
    key.
22. A lost-or-stolen report. §7.2 now lets the account bound to a device id
    retire it on-chain. Retirement emits a final block entry, so the phone can
    no longer pay or be paid among holders of the list; it can still unload
    what it holds. Whoever holds the account key can therefore stop a live
    phone paying offline, and cannot take its balance. Confirm.
23. Answered 2026-10-02: PIN or biometric is a user-experience function and
    "generally it should be related to the secure hardware". How §5.9 reads
    that is item 50.
24. Play Integrity on Android: required at enrollment and renewal, or an
    optional signal. The 2026-10-01 record says required; the issuer code
    treats it as optional. Requiring it excludes phones without Google Play.
25. The recovery default. The 2025 position was that a lost phone is lost cash.
    See items 10 and 51.
26. Undelivered payments: whether the issuer relays a Payment or an Outcome
    that never crossed, when both sides later sync. See item 37.
27. Confirm withdrawn: replaying every offline transfer on the ledger; paying
    the first receiver to sync and rejecting later ones; voiding value paid out
    of a wallet later reported stolen. This document reads the owner's
    statement of 2026-10-02 on finality as withdrawing these three (§14). That
    the current requirements override the 2025 document in general is this
    document's assumption (§1). Confirm that as well.
28. ATMs, merchant terminals and machine-to-machine devices: out of scope for
    now, or wanted. Each needs an enrollment class that is not phone
    attestation.
29. The longest outage through which a phone must keep sending with no contact.
    The default lease follows from it.
30. Whether the limit day is the fixed UTC day, as here and in the ledger's
    daily limit, or follows local midnight.
31. Whether "prevent double spend" must hold against a compromised phone. No
    way to do that on a stock phone was found, and none of the candidates has
    been tested on a device (§4.1). The choices are to accept limits, expiry,
    evidence and a backstop for that case, or to require hardware that runs
    wallet logic, such as a card or an applet, which is set aside for now.

Raised by the owner's statement on finality and by revision 5:

32. Confirm rules F1 to F5 (§1) as the meaning of the statement of 2026-10-02.
    In particular: is a received payment redeemable at face value with no time
    limit, including after a scheme is closed to loads?
33. Does "regulatory controls" mean exactly R6 (the block list, including
    receive freshness), R7 (limits) and R8 (expiry, including the reboot
    policy)? If a fraud hold, a key revocation or a rules-version floor should
    also count, say which. §3.1 lists each as an exception because this
    document reads them as not regulatory.
34. Key revocation (§3.1). Option 1, the rule in §5.1: every phone under a
    revoked key stops paying and requesting until it syncs. Option 2,
    recommended: certificates stand until their own expiry, capped at one lease
    after the revocation. Option 3: never revoke. Under option 2, for
    certificates that never expire: no limit, or a grace period after which the
    holder must sync once. The root key's succession rule (§5.11) is decided
    with this.
35. Stopped wallets (§5.10, §7.2). A wallet whose marker is gone while its
    key lives cannot be used again. Removing the passcode on an iPhone does
    this; so does restoring older wallet files. A wallet that has lost only
    its journal can be repaired online if it signed nothing since its last
    sync. Accept it as a
    listed exception with a warning before the first load, or let such a wallet
    resume online, knowing that an ordinary user can then reset a balance and
    the operator pays each time.
36. A hold on a phone whose own key signed conflicting records (§5.3, §8.2).
    The hold stops that phone's unloads until a governed reinstatement. It is
    the only event that closes the unload door for a compromised phone, and a
    platform fault can put an honest phone there. Accept the exception? The
    reinstatement path must exist before production value; say what it pays.
    May the issuer also place a hold without on-chain evidence, and for how
    long?
37. Undelivered payments (§5.2). A payer whose Payment, or whose `Refused`
    Outcome, never crossed has the amount in neither wallet until the two
    phones meet again. Is that enough, or is the issuer relay of item 26
    wanted? And are Requests and Outcomes kept for the life of the wallet
    (about 0.3 KB each, an estimate), or for a period after which the loss is
    permanent?
38. Migrate (§7.2). Payments with no stored Outcome are abandoned at a Migrate,
    and a Payment that arrives later for one of the old phone's Requests gets
    no refund. Accept that as the user's informed choice, or add a signed
    object by which the new phone answers for the old one (not designed).
39. A lost account key (§8.2). An unload pays only the account bound to the
    device, and nothing changes that binding. A holder who loses the account
    key can still pay offline but cannot unload or Migrate. Accept, or define a
    governed rebinding, which is a theft path for whoever controls it.
40. A rules-version floor (§5.11). May the operator retire an old rules version
    so that an app that has not been updated can no longer pay or be paid by
    wallets that hold the notice? That is a forced update. The alternative is
    to retire a version only when a certificate is issued or renewed; with
    expiry off, a defective version then lives as long as its holders stay
    offline.
41. What gives the operator's promise force outside the ledger: a guarantee,
    capital, or statute (§8.4). The ledger cannot make the operator fund the
    pool. Also: the amount paid in before loads open; whether loads close while
    the pool is short; and, where the operator also issues the asset, whether
    the backstop may be funded with newly issued units (prohibited, allowed, or
    allowed only as a visible on-chain issue).
42. The brake on unloads (§8.2). The draft has one: a per-row amount per window
    above a row's own loads. A cap shared by all rows would bound the total
    drain, but an honest holder's wait would then depend on how many others
    claim. Confirm no shared cap. Also the value per tier, and whether a
    merchant tier has a higher one.
43. Whether a row is paid above its own loads only while a valid witness
    receipt is recorded on-chain (§8.1, §8.2). It stops a thief of the registry
    authority alone from draining the pool. Its cost: after a witness-key
    revocation such payouts pause until receipts are recorded again.
44. When the account bound to a device is blocked on the ledger, is its unload
    payout held until the block is lifted, or paid? The claim stays recorded
    either way (§8.2).
45. The default tier, given the statement on finality. With expiry on, every
    holder syncs once per lease. With receive freshness on, every receiver
    renews within a period. With limits and expiry both off, nothing bounds or
    reveals what a compromised phone creates (§3). Which controls are on by
    default, and what lease length?
46. Limits after a lost phone (§5.4, §7.2). The issuer cannot know what a lost
    phone spent. The draft counts its whole allowance for the current day and
    month as spent. Where expiry is off, the lost phone's share can never be
    shown dead: either it never returns to the account, or it returns after a
    period during which the account can exceed its limit through the old phone.
47. Four settings of §5.4 and §5.5: how far apart two phones' dates may be
    before a payment under limits or expiry is refused (2 hours proposed, with
    no measurement behind it); whether the per-counterparty cap is a limit in
    the sense of R7 and so absent when limits are off; whether the
    `send_blocked` flag, which R6 does not mention, is wanted; and whether
    expiry may be switched on for certificates already issued.
48. Closure (§5.11). Closing a scheme ends loads and enrollments and never ends
    redemption. The reserve behind unredeemed offline value can therefore never
    be released. Confirm. If the chain itself were retired, who pays unloads?
    Does a legal dormancy rule count as a regulatory control that may set a
    deadline?
49. Android renewal (§4). A renewal shows nothing new about the phone's boot
    state or patch level. Is a freshly generated attested key at each renewal
    wanted? It would show the current state of a phone that holds the payment
    key at that moment. Not designed here.
50. User authentication (§5.9). Recommended: the device key carries no
    authentication requirement and the app shows the platform's PIN or
    biometric prompt before paying, unloading or migrating. A compromised phone
    can skip the prompt; no balance is lost to a settings change. The
    alternative binds the key, and on Android the platform then destroys the
    key, and the balance, when the screen lock is removed. Does the recommended
    option meet "generally it should be related to the secure hardware"?
51. Recovery insurance (§7.3). Confirm off by default. If an operator may
    switch it on, a payout never retires or blocks the old phone, so an
    ordinary user can claim and keep spending: expect the whole budget to be
    drawn each period. Is an optional delay before payout wanted, and may a
    payout be deducted from later unloads by the same device?
52. Renewal and the chain (§6, §7.1). Each renewal waits for one registry write
    to be final, so with expiry on a long chain outage stops phones sending;
    and the chain shows when each device synced. Keep both, or anchor serials
    ahead and anchor heads as one digest per period.
53. Fee settlement (§5.8). Per payment on-chain, as drafted: the chain then
    holds payer, receiver, amount and the payer's totals for every settled fee.
    Or in aggregate by the issuer: the chain sees no payment and the total
    rests on the issuer's word. Also: a new fee policy reaches a wallet only at
    its next renewal.
54. The marker (§5.10). Accept a window of a few milliseconds in which a copy
    of the wallet's files holds a signed, unreleased object, or close it at the
    cost of a second durable write on every payment. And: if the power-off test
    shows that a phone model can lose a marker deletion that had already
    returned, is that model unsupported?
55. Decision B in detail (§2, §2.3). Is one to two seconds a gate for a
    proof-carrying payment too, or may it take longer, and how long? The budget
    and date of each stage. How many times the relation or proving system may
    be reconsidered (once is proposed). Which outcomes at the final gate are
    ruled out now. If only the lean relation passes, is that acceptable? Under
    a proof-carrying design: may the payer's balance be shown to the receiver;
    do proofs keep accepting revoked keys and withdrawn circuit releases; may
    Android keys be generated to sign any 32 bytes from the first enrollment;
    and confirm that received value is spendable only after the receiving phone
    proves it, and that redemption must not need the holder's proof.
56. One design (§11, §11.1). Does accepting this proposal withdraw the existing
    statements that make stock-phone keys online-only or require non-forking
    hardware: yes, no, or only if a proof is carried? May Layer A replace the
    signature-only suite in place, and does any app outside this repository use
    that suite's types? Until removals happen the tree holds four payment
    designs instead of three; is that exception to the
    no-parallel-implementations rule accepted, and until which event? Who owns
    migration of the adapters named in the 2026-10-01 record? What do the other
    SDKs keep? Is a finite-state model of the commit and marker rules wanted?
57. Carriers and confidentiality (§5.6, §5.7). QR only for the first cut, or
    also build and measure a Bluetooth carrier? Should the Payment and Outcome
    be encrypted against bystanders (about 100 bytes, an estimate)? Is it
    acceptable that the chain shows when each device synced and, with fees on,
    a record of every payment whose fee is settled?
58. Evidence of an issuer-side fault (§5.3). A certificate and receipt whose
    device id has no registry row, or a MintFold whose voucher matches no
    on-chain load, shows misuse of an issuer key and not of the phone. What
    does such evidence do to the phone that holds the object, and to its
    balance? Item 36 covers only a phone whose own key signed the conflict.
    Also: who may submit a reinstatement (§6).

## 13. Evidence

[`kagemusha_single_design_evidence.md`](kagemusha_single_design_evidence.md)
opens with section 0, a claim map for this revision: each factual claim, by
section of this proposal, with its source, its class (documented, measured in
the repo, read from code, third-party, extrapolated, inferred) and whether the
source supports it. The map marks every row that rests on a single automated
pass. Sections 1 to 6 of the appendix are the research behind earlier
revisions, kept as history under the section numbers of the revision they were
written for. Section 7 holds the sources the revision-5 drafts added.

## 14. The June 2025 TC3 document

"TC3: Offline Capabilities" is a response to a central-bank consultation. Its
header says it was written with input and technology from the owner's company.
It is not in the repository. The owner supplied it on 2026-10-02 as a source
of ideas and requirements, with the note that it is old and not fully current.
The date is the owner's; the text read here carries none. An automated
comparison produced about ninety findings. This section keeps those that
change the design or need a decision.

**Its design.** A hardware key; an attestation certificate from an offline
certificate authority; a signed transaction shown as a QR code; an optional
countersignature by the receiver; upload of every transfer at the next sync.
That is signatures only, with no proof. A payment that has passed through
several hands is read here as carrying the signed record of each. The document
says only that each hop adds a signed record and that the first sync carries
the full history; its payment message holds the signed transaction and the
certificate. R4 and R9 together rule that reading out (§2).

**Where it disagrees with itself.** On two points the document says two
things. This document follows the first of each pair. On finality, the first
is also what the owner's statement of 2026-10-02, quoted below, asks for. On
forced sync, the statement rules out a sync that no regulatory control
requires. It does not speak of holding limits.

- Finality. It says the payment is complete once the receiver has verified
  and recorded it, and that the receiver can spend the value again offline. It
  also says that at sync the server honours the first valid spend and rejects
  the rest, and that value from a wallet reported stolen could be refused.
- Forced sync. Its summary recommends limits on transactions only, and
  recommends against holding limits and synchronization requirements. Its
  policy section then lists a balance ceiling, a forced resync after a number
  of payments, a value or a time, and an automatic freeze.

**Carried over.** An attested hardware key and a certificate shown in every
payment. A request, then a signed payment. The payer debited at signing. A
payment that is complete once the receiver has verified and recorded it. The
receiver able to spend again offline, for any number of hops; Decision B says
whether a proof must be made first (§2, §9). Synchronization that is optional.
A block list refreshed at sync. Optional limits and expiry, which stop sending
and leave receiving alone; here only the block-list freshness control of §5.5
stops a wallet requesting. Higher limits by tier for named users. Unload only
to the bound account. A lost phone is a lost balance. PIN or biometric before
paying (§5.9).

**Withdrawn.** This document assumes that the current requirements override
the 2025 document where the two disagree. The owner has not said that in
general. On finality the owner has said, on 2026-10-02: "offline payments must
be secure and final. once you transfer from one phone to another, it must be a
final transfer of value there, with no need to ever go online again unless
there are regulatory controls". Reading that statement as withdrawing the
first three items below is this document's reading. §12 item 27 asks the owner
to confirm it.

- Every offline transfer replayed on the ledger between per-wallet offline
  accounts. A transfer that is final on the phone cannot wait for the ledger
  to execute it, and nobody has to go online to report it. Here the ledger
  holds a pooled reserve. It never executes an individual offline payment and
  never reverses one. What it sees is listed in §5.7.
- "Honors the first valid spend, rejecting the rest." That takes value from a
  receiver after the transfer, through someone else's act. Here a received
  payment is never reduced or reversed, and a shortfall caused by counterfeit
  is the operator's (§8.4).
- Refusing value that came out of a wallet later reported stolen, "at the cost
  of the innocent payee". Here a block entry against a payer stops later
  payments by holders of the list (§5.5). It does nothing to value already
  received.
- "The Backend system thus always knows the total amount of Digital Shekels
  that are in circulation offline." No requirement overrides this. It is
  dropped because it is not true once a phone is compromised: the ledger knows
  loads minus paid redemptions, and counterfeit is not observable (§8.3).
- An expired certificate that still pays when there is no connectivity. Where
  the scheme has switched R8 on, sending stops after `expiry_grace`. Where R8
  is off, a certificate does not expire. R8 overrides this item; the owner's
  statement does not bear on it.
- An override code obtained by telephone for one large payment. R1 allows no
  call to authorize a payment.

**Statements that do not hold on stock phones.**

- Balance, limits and one-time-spend counters kept and enforced inside the
  secure hardware. Apple's and Android's secure hardware perform key operations
  for whatever the app asks and run no app code. The owner has confirmed that
  the design does not assume this (§1).
- Attestation proving that the wallet's keys are in secure hardware. On
  Android the attestation chain shows it for the payment key. On iPhone Apple
  attests the App Attest key only, and the payment key is a second key that
  nothing attests (§4).
- Cloned devices blocked by attestation checks. Attestation is made once, at
  enrollment. It does not see a compromised phone copy its wallet files and
  pay twice (§4.1).
- SafetyNet was shut down in January 2025. Play Integrity gives a verdict about
  the app and the device; it does not attest a key. Key attestation and App
  Attest do that (§4).
- Payment by Bluetooth advertising. An iPhone app can advertise 28 bytes. A
  payment needs a connection. The platforms allow one between store apps; the
  repo does not implement one (§5.6).
- Tap between any two phones (§5.6).
- Anonymous wallets that can also be traced to an owner through provisioning
  records. Attestation open to a store app gives no unique device identifier,
  so an un-identified wallet traces only to the account that funded it.

**Not in R1–R9; each needs a decision.** §12 items 15 to 31. The owner has
answered three (timing, fees, user authentication). The list: timing and the
meaning of the size bound; carriers; fees; privacy tiers and bystander
confidentiality; the default on forced sync; holding, count and since-sync
caps; one issuer or several; a lost-or-stolen report; user authentication at
the key; Play Integrity; the recovery default; relay of undelivered payments;
other device classes; the design outage; the limit day. Item 27 asks the owner
to confirm the withdrawals above. Item 31 asks whether prevention must hold
against a compromised phone (§4.1). Two points of the old document have no
decision item here: clearing history from the phone after a sync, and location
stamps on records. A third, version checks that force app updates and push
emergency rules, is §12 item 40. A version check that stops a wallet until it
updates forces the wallet online. As this document reads the owner's
statement, only a regulatory control may do that. This document has one such
check, the rules-version floor of §5.11. §3.1 lists it as an exception, and
it exists only if the owner allows it.

**Set aside by the owner.** Smart cards: a future option to explore, with no
plan to use them now. The owner did not mention wearables, which the 2025
document names beside cards. A passive wearable is a card in another shape and
is treated the same way here.
