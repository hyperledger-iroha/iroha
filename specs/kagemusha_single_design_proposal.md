# KAGEMUSHA single design — proposal, revision 6

Status: **proposal for owner decision, 2026-10-02. Not accepted. It authorizes
no deletion and no production release.** Nothing in §2–§12 is implemented.
Sentences that say "exists", "existing", "today", "current" or "the repo"
describe present code.

Process note. Every repository and web fact in this document was gathered by
automated passes run by the author. None has been confirmed by a human expert,
a build, a test or a device. Revision 6 was drafted section by section by six
automated passes from one brief, after four automated design passes and three
adversarial passes on the acceptance criterion, the marker and the
operating-system evidence. Sources are in
[`kagemusha_single_design_evidence.md`](kagemusha_single_design_evidence.md).
Sizes and times are estimates or arithmetic unless a line says measured.

History:

- Revisions 1 to 4 moved from "signature-only, delete the proof code" to
  "keep the proof code, decide after measurement", and added the marker, fees,
  user authentication and the June 2025 document. Each was checked by
  automated passes that found defects, some serious.
- Revision 5 was written around the owner's statement that offline payments
  "must be secure and final". It read that statement as "the operator
  underwrites" and listed about thirty rules as exceptions.
- A review the owner pasted rejected both. It gave the acceptance criterion
  that §1 now states, said that "documenting exceptions does not satisfy it"
  and that a backstop "cannot establish that value was transferred without
  duplication", and asked for device evidence before any choice between a
  proof-carrying and a signature-only payment.
- The owner then said: "because we cannot allow compromised OS, we should
  include as part of our proof constraint matrix that the OS is real and prove
  it somehow".
- Revision 6 is built on the acceptance criterion. It states the assumptions
  the design needs, states what it claims for each property under them,
  removes the rules that
  broke a property, makes the marker carry the wallet's current state, puts
  evidence about each paying phone's operating system into the proof's
  relation, and puts a device evidence gate first. One automated consistency
  check of the assembled text found 202 defects, 20 of them serious, mostly
  places where sections written in parallel disagreed. They are fixed. The
  result has not been checked, and more disagreements of the same kind should
  be expected.

## 0. Summary for the owner

**The criterion.** For a completed payment from phone A to phone B: B owns the
value durably and can spend it onward offline (P1); A cannot spend it again,
under the stated security assumptions (P2); B's payment does not depend on
later reconciliation, approval or settlement (P3); later discovery of
misconduct by A cannot invalidate B's value (P4); only explicitly enabled
regulatory controls may require connectivity (P5); and everything needed for
these must finish before a wallet reports the transfer complete (PC). §1
states it as binding.

**The assumptions.** The design claims those properties under six stated
assumptions (§4): the phone's secure hardware keeps keys inside it; the
phone's operating system is the vendor's and behaves as documented; the app is
the released wallet; the issuer's keys and the ledger are sound; the
cryptography is sound; and the holder keeps the phone, the app and, on iPhone,
a passcode. §3 states what the design claims for each property under them.
None of it is tested on a phone, and no prover exists. No backstop or
underwriting is part of that argument.

**The mechanism against paying twice.** The wallet keeps a marker in the
phone's key store, outside every backup, and replaces it at every signed
object. The marker carries the wallet's current state. A wallet whose files
are restored, reinstalled or damaged resumes at its current balance and cannot
go back to an older one (§5.10). This is the "unique counter/commitment" of
the owner's mechanism. It rests on key-store behaviour that has not been
tested on any phone.

**The proof and the operating system.** Following the owner's instruction, a
payment carries a proof, and the proof's relation includes a statement about
the operating system of every phone the value passed through (§2, §2.1). The
statement is made from what the phone's secure hardware signs at enrollment
and, where expiry is on, at each renewal. What it shows, as of enrollment or
the last renewal: the phone's secure hardware was told at boot that a
vendor-signed system was verified on a locked bootloader, at an acceptable
patch level, and the system named the
genuine wallet app. What it cannot show: that the running
system has not been taken over since. No hardware-signed statement available
to a store app changes when that happens. On iPhone no such fields exist at
all; the statement there is "a genuine Apple Secure Enclave acting for this
app". Three things therefore stay assumptions: no run-time takeover of the
paying phone, no leaked attestation key, no broken secure hardware.

**Where the criterion is not met today** (§3.1). None of these is an accepted
exception; each says what would be needed.

- P2 against a phone whose operating system is taken over at run time. No
  mechanism reachable by a store app prevents it. Meeting it needs hardware
  that runs wallet logic, which is set aside for now.
- Time. With a proof on every payment, the payer's and the receiver's proving
  both fall inside the payment. Nothing is measured; the repo's own gate is
  10 s for one proof. "1-2 s should be good ux" is not met unless proving
  takes a fraction of a second. No prover exists yet.
- iPhone. Removing or resetting the passcode discards the marker. And Apple's
  published key-store source suggests a write may not survive a forced restart,
  which would let an ordinary user pay twice; only a device test can settle
  it.
- A lost or erased phone loses its value. A payment that never completed can
  leave the payer out of the amount.

**What comes first.** A device evidence gate (§10.4): tests on each target
phone of restore paths, crashes and power cuts, key-store behaviour, what
attestation carries, and timing. Each phone comes out unsupported, supported
with a stated assumption, or supported. The proof qualification follows the
gate (§2.3).

**Decisions that are the owner's** (§3.2, §12). The document takes none of
them.

1. Whether P2 must hold against a compromised operating system. If it must,
   stock phones are expected to fail, and the options are hardware that runs
   wallet logic, or no production release of offline value on stock phones
   (§3.2).
2. What follows if a proof-carrying payment cannot be made in an acceptable
   time.
3. Which phones and platform classes may hold value, given that iPhone and
   HarmonyOS NEXT carry a weaker statement about the operating system.
4. Which regulatory controls are on by default. With expiry on, the lease
   (the period after which a wallet must sync before it can send again) is
   also the age limit of the evidence about each phone's system.
5. If an assumption fails on some phone, value exists with no load behind it,
   and under P4 it stays good in honest wallets. Who supplies the difference
   is not decided here.
6. One design: what becomes of each existing track (§11.1).

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
quotation marks are the owner's. The owner's statements follow in full, each
with the place where it bears. Where this document reads more into a statement
than its words say, the reading is marked as this document's.

**What the owner has said.**

- One design. "there should be only one design. we need to remove other stuff
  and standardize on one design." §2 names the design. §11 gives every existing
  track an end state.
- The security model. "our security model requires us to have a hardware backed
  key that is used to attest that our signing key/state is valid and the tx is
  from a valid app. then our offline-offline payments are allowing unbounded
  hops, except when there are regulatory rules like blacklisted accounts or
  daily/monthly spend limits, etc." R2, R4, R6 and R7 come from it. The end of
  this section says where stock phones fall short of it.
- The product. "kagemusha is an offline protocol so online only is not in our
  design. we want to make usability so any user with a modern phone like pixel
  6, huawei, meizu, iphone, samsung, etc., that is modern and mainstream and
  has a secure element can charge up from online to their offline wallet, then
  do direct device to device transfer of value, then can move it online if they
  want, but online settlement is not required. the offline value transfer
  itself is final and settled instantly. we want to have a blacklist of
  accounts that users that have the blacklist won't send to. we want there to
  be some notion of daily or monthly limits that can be optionally set. also
  there should be some optional expiry for attestation so users will have to
  sync online before they can send offline again, optionally." R1 to R8 restate
  it.
- Size. "to do device to device transfers, we really cannot have payment data
  exceed around 10k or so, which is why we were looking at using cryptographic
  proofs to proof in a concise way correctness". This is R9, and it is why a
  payment carries a proof and not its history (§2).
- Hardware. "right now smart cards are future optionality to explore but there
  are no plans right now to use. we don't assume running in secure hardware as
  we don't have oem access and instead plan to use hardware backed keys and
  some unique counter/commitment to prevent reset and double spend". §4.1 says
  what stock phones offer for a counter or commitment.
- Fees, authentication, timing. "1) optional fees sound good, but can only be
  received to an online account when someone syncs. 2) pin/biometric is a ux
  functionality but generally it should be related to the secure hardware on a
  phone 3) yeah, 1-2 s should be good ux". §5.8 covers fees and §5.9
  authentication. Timing is taken up below and in §3.
- The June 2025 document. "for reference, a lot of the ideas/reqs we have for
  offline can be found here, but this is an old doc and not fully up to date".
  §14 compares it with this proposal.
- Finality, 2026-10-02. "offline payments must be secure and final. once you
  transfer from one phone to another, it must be a final transfer of value
  there, with no need to ever go online again unless there are regulatory
  controls". R4 and R5 are restated from it.
- The acceptance criterion. On 2026-10-02 the owner supplied a review of this
  proposal. The review states the criterion set out below. It says: "The design
  must satisfy it; documenting exceptions does not satisfy it." It says: "A
  backstop can supplement a secure protocol; it cannot establish that value was
  transferred without duplication." And it says: "The next design gate should
  be evidence that the proposed hardware and protocol meet it, before treating
  either B1 or B2 as an acceptable implementation choice." B1 and B2 are the
  review's names for a payment that carries a proof and a payment that carries
  signatures only (§2). The gate is the evidence gate of §10.4.
- The operating system. "because we cannot allow compromised OS, we should
  include as part of our proof constraint matrix that the OS is real and prove
  it somehow". §2.1 puts the constraint into the proof. §4 says exactly what
  the proof then shows.

**The acceptance criterion.** It is binding. The design has to meet it. For a
completed payment from phone A to phone B:

- P1. B owns the transferred value durably and can spend it onward offline.
- P2. A cannot spend that same value again, under the stated security
  assumptions.
- P3. B's payment does not depend on later reconciliation, approval, or
  settlement.
- P4. Later discovery of misconduct by A cannot invalidate B's accepted value.
- P5. Only explicitly enabled regulatory controls may require connectivity.
- PC. Any processing needed to establish those properties must finish before
  the wallet reports the transfer as complete.

The labels P1 to P5 and PC are this document's. The sentences are the review's.
§3 states what the design claims for each. §3.1 states where the design does
not meet them.

**How this document reads the criterion.** Each line is this document's
reading, not the owner's words. The owner confirms or corrects each (§12).

- "A completed payment". The receiving wallet has reported the transfer
  complete under PC. A payment that was signed and never reached that point is
  not a completed payment. The criterion does not speak of it. §3.1 does.
- P1, "durably". The value survives everything that leaves the device key and
  the marker (§5.10) on the phone: a restart, a power cut, a locked phone, an
  app update, an operating-system update, and the loss, damage or restore of
  the wallet's files. This document does not read "durably" as surviving the
  destruction of the key that holds the value. Whether durable may mean as
  durable as the phone is the owner's to say. §3.1 states what the other
  reading would need.
- P1, "spend it onward offline". From the moment the wallet reports complete,
  the next payment needs no network, no proof still to be made for the received
  value, and no other party.
- P2, "under the stated security assumptions". The assumptions are T1 to T6 of
  §4 and no others. The criterion attaches these words to P2 only. This
  document claims P1, P3, P5 and PC under assumptions as well: about the
  holder's own phone and about the issuer side (§3). That is a reading. §3.1
  puts it to the owner.
- P3, "later reconciliation, approval, or settlement". Nothing that happens
  after B's wallet reports complete is needed for B to hold the value or to pay
  it onward. A renewal under R8 is a regulatory control and falls under P5.
  Redeeming value on the ledger is the holder's own choice to go online.
  Whether P1 and P3 also govern how fast the ledger pays a redemption is not
  settled by the words. §3.2 puts it to the owner.
- P4. No condition is attached, and this document adds none. "Invalidate" is
  read to include reduce, refuse, hold and delay.
- P5, "explicitly enabled regulatory controls". R6, R7 and R8, each only where
  the scheme has switched it on, so that it shows in the certificate, in a
  root-signed notice or in the block list. "Require connectivity" is read as:
  stop a working wallet paying or requesting until it reaches the issuer, the
  ledger or an app store. The owner named a blacklist, limits and attestation
  expiry. Six things under those names are this document's additions and count
  only if the owner confirms them: receive freshness (§5.5), the `send_blocked`
  flag (§5.5), the per-counterparty cap (§5.4), the reboot policy
  `require_anchor` (§5.4), a tier-row notice that switches a control on for
  certificates already issued (§5.11), and a block at the holder's own request
  (§7.2). What a renewal under R8 may check is in §4 and §3.2.
- PC, "complete". §5.2 fixes it. The receiving wallet reports complete after
  its checks, after any proof its own onward spending needs, after the durable
  commit of the ReceiveFold and the `Credited` Outcome, and after the marker
  step is confirmed. The paying wallet shows "sent, not confirmed" until it
  stores a `Credited` Outcome, and "returned" after a RefundFold commit.

**One wallet rule beside the criterion.** A wallet never destroys its own key
or balance on a condition that may pass: locked storage, a failed read, an
error code. A destructive step needs an established, unrecoverable loss. The
rule exists because a locked key store and a missing wallet look alike to an
app that only tests whether a read succeeded. §5.10 and §7.2 apply it.

**Readings of the requirements.** Each is this document's.

- R2. The owner named a phone that "is modern and mainstream and has a secure
  element". Hardware-backed key storage, and the exclusion of a custom applet,
  are this document's reading of the statement on hardware above: no wallet
  logic is assumed to run inside secure hardware, and the project has no OEM
  access. Smart cards are a future option. Nothing here is designed for them,
  and nothing should foreclose them.
- R4. The owner first stated it as "the offline value transfer itself is final
  and settled instantly", with "unbounded hops, except when there are
  regulatory rules like blacklisted accounts or daily/monthly spend limits,
  etc." This document reads "final and settled instantly" as P1, P3, P4 and PC.
- "Instant". The owner said "1-2 s should be good ux". This document takes one
  to two seconds as the target. The owner stated no percentile, no end points
  and no gate. Under PC everything a payment needs falls inside the interval,
  so this document measures from the payer's confirmation to the receiving
  wallet reporting complete. The end points and the percentile are for the
  owner to fix before the first measurement (§2.3). §3 says what a proof does
  to this target.
- R8. The owner's words are "optional expiry for attestation so users will have
  to sync online before they can send offline again". In this design the thing
  that expires is the device certificate. A renewal issues a new certificate
  and a new enrollment statement (§4). On Android the renewal carries a fresh
  hardware attestation, so R8 there is attestation expiry in the owner's sense.
  On iPhone an attestation of an existing key cannot be repeated. A renewal
  there carries an assertion by the enrolled App Attest key. It shows that the
  key still signs and shows nothing about the operating system.
- R9 has no unit. §5.6 gives this document's reading.
- The operating system. This document reads "proof constraint matrix" as the
  relation that a payment proof proves (§2.1). It reads "prove it somehow" as:
  put into that relation every statement about a paying phone's operating
  system that vendor-signed evidence can back. That evidence describes how a
  phone booted. No evidence that a store app can obtain shows that a running
  operating system is uncompromised (§4). The owner's words "we cannot allow
  compromised OS" are therefore met as an assumption (T2) and not as something
  a receiver verifies. §3.1 says so among the places where the criterion is not
  met.
- The June 2025 document. This document assumes that R1 to R9 and the criterion
  win where they disagree with it. The owner did not say so; §12 asks.

**Where stock phones fall short of the stated security model.** The owner's
model is a hardware-backed key "that is used to attest that our signing
key/state is valid and the tx is from a valid app". Stock phones give less.

- No platform attests wallet state. Attestation covers a key and an app
  identity at one moment. The app keeps the state, and the marker of §5.10 is
  the app policing itself.
- No platform attests the running operating system. Android attests how the
  phone booted and which patch levels its secure hardware was told at boot.
  iPhone attests nothing about the operating system.
- On iPhone the payment key is not itself attested.
- No payment carries platform evidence that the released app produced it. On
  Android the app identity is stated by the operating system when a key is
  generated. On iPhone the payment signature carries no app identity.
- No target phone is shown to have a counter or a one-use key that its secure
  hardware enforces against its own operating system (§4.1).

The rest of this document uses the statements of §3 and §4. They claim no more
than this allows.

## 2. The proof and what it must show

The owner's instruction:

> because we cannot allow compromised OS, we should include as part of our
> proof constraint matrix that the OS is real and prove it somehow

Two other statements of the owner bear on the same point. On size: "to do
device to device transfers, we really cannot have payment data exceed around
10k or so, which is why we were looking at using cryptographic proofs to proof
in a concise way correctness". On the security model: "our security model
requires us to have a hardware backed key that is used to attest that our
signing key/state is valid and the tx is from a valid app".

**The design.** What follows is this document's reading of those statements.
It is not the owner's wording.

- A payment carries a proof.
- The proof's relation covers every hop behind the paid value, not only the
  last one.
- For every phone that paid in that history, the relation includes an
  enrollment statement about that phone's hardware, operating system and app.
- No phone checks a vendor certificate chain. The validators check it when the
  device is registered, and a server may also prove it once. A phone proves
  one device signature per hop and the rules that link the hops.

§2.1 gives the relation clause by clause, and says exactly what a verified
proof then shows and what it does not show. It does not show that the
operating system of a paying phone was uncompromised when the payment was
signed. On every platform studied, no evidence that a store app can obtain
shows that (§2.1).

Terms used in §2.

- Layer A is the design apart from the proof: device keys, the journal, the
  marker, the exchange of §5.2, the registry and the ledger accounting (§4 to
  §10). The proof is made over Layer A's objects.
- A hop is one transition (§5.1) in the history behind a payment: every
  transition, by every wallet, through which any part of the paid value
  passed since it was loaded.
- A relation is the statement that a proof proves.
- The enrollment statement, E, is the per-device statement about hardware,
  operating system and app. §2.1 lists its fields.
- The app attestation key is a key that the Android wallet creates in the
  phone's secure hardware at enrollment. Its only use is to certify keys
  that the same hardware generates later.
- A policy entry is one entry of the policy table (§5.1), which §2 also calls
  the governed table. The table holds the values that the enrollment relation
  compares against: vendor root keys, the app identity, patch floors, the
  revocation set (§2.1, "Policy inputs").
- The signature-only form is a payment with the same Layer A objects and no
  proof (§2.4).

**Status.** No prover exists that can make any State or payment proof, on a
host or on a phone (§2.2). Whether the relation of §2.1 can be proven on the
target phones, in any time, is not known. Nothing in §2 has been built or
timed. §2.3 is the plan that decides it, and it starts only after the evidence
gate.

**Decision B.** The name stands for the question whether a payment carries a
proof. The owner's instruction answers the question of the target: it does,
and the proof carries the constraint about the operating system. What remains
under that name is the owner's ruling at the end of §2.3: whether the
proof-carrying design is qualified on the supported phones, in a time the owner
accepts. The signature-only form is described in §2.4 so that the reader can
see what a receiver checks without a proof and what is then missing. It is not
offered as an equal option.

**What a receiver learns from one payment.** The receiver is offline. The
table says what it learns from the bytes it receives and its own state, with
the proof and in the signature-only form.

| What the receiver learns | With the proof (relation of §2.1) | Signature-only form (§2.4) |
|---|---|---|
| That the payer's device key signed this SendSplit for this receiver's own Request | Yes, by its own signature check | Yes, the same check |
| The payer's enrollment statement E | Yes. E is opened from the proven state, and it was sealed by the validator quorum at registration | Yes, the same E, from the certificate and the witnesses' receipt. The receiver does not verify the validators' seal, so E then rests on the certificate key and the witness quorum (§5.1, §8.1) |
| The state of the payer's operating system | What E states: how the phone booted and which app the operating system named, as of enrollment or the last renewal. On iPhone, nothing about the operating system | The same |
| Whether the payer's operating system is uncompromised now | Nothing | Nothing |
| Whether the payer signed another successor of the same state | Nothing | Nothing |
| Whether the payer's wallet used its marker (§5.10) | Nothing | Nothing |
| That the payer's balance covers the amount and the fee | Yes, proven | Nothing. No peer-visible object carries a balance (§5.7) |
| How the payer's balance was reached | Every transition since the payer's Bootstrap was signed by the payer's key, in sequence, linked, with the counters and the balance added up correctly | Nothing |
| The earlier holders of the value | Each had a device key with an enrollment statement that met a policy entry of the governed table. Each of their transitions was signed by that key and kept the same rules | Nothing |
| Where the value began | Every unit traces to a load that the validator quorum sealed, if the mint stays quorum-sealed in the relation (§2.3) | Nothing |
| An arithmetic defect in the payer's app or an earlier holder's | It cannot be in a proven history. A defect in the circuit itself is not covered | Nothing. The receiver accepts the value and passes it on |
| Lease, limits, block list (R6 to R8) | The receiver's own checks, from the payer's certificate, its own clock, list and tally. The proof adds nothing | The same checks |
| Size of the Payment | About 7.7 KB: the repo's 6,528 B proof ceiling and about 1.14 KB of Layer A objects, of which E is about 0.14 KB (§5.1). An estimate, and only if verifying a device signature per hop does not enlarge the proof that travels. The repo's payment-message gate is 7,552 B, about 0.1 KB less | About 1.14 KB. An estimate |
| On QR with the repo's existing framing (§5.6) | About 50 frames: 4 to 10 s per pass at 12 to 5 frames per second. Computed, not measured | About 8 or 9 frames: 0.7 to 1.8 s per pass. Computed, not measured |
| Work inside one payment | Two proofs, one on each phone, and one proof verification, besides everything in the next column. Not measured | Four hardware signatures, the marker steps, the durable commits and three transfers. Not measured |
| What exists | No prover. No State or payment proof has been produced (§2.2) | No prover is needed. None of it is written |

**What the proof does not replace.** P2 does not come from the proof. A payer
is kept from paying twice by its own wallet: the marker of §5.10, under
assumptions T1 to T3 on the payer's phone (§4). A proof does not check the
marker and cannot. Both branches of a fork made on a compromised phone satisfy
every clause of the relation (§2.1). While T1 to T6 hold on every phone, P1 to
P5 hold with or without a proof. What the relation changes is how much a
receiver must take on assumption about phones it never sees. With it, every
earlier hop was signed by a key whose phone met the enrollment statement, the
arithmetic is right, and the value began as a sealed load.

**Why a proof and not the history.** A payment could instead carry the signed
record of every earlier hop. One earlier hop costs about 0.85 KB (certificate,
receipt, signed transition; an estimate). Such a payment passes 10 KB after
about eleven earlier hops, or at once when the payer's balance merges about
eleven received payments, because each one brings its own history. R4
(unbounded hops) and R9 (about 10 KB) together exclude it. The proof makes
the same check at constant size.

**What "complete" means, and what it does to time.** PC (§1) says that any
processing needed to establish P1 to P5 finishes before the wallet reports the
transfer as complete. With a proof on every payment that fixes the order of
work on both phones.

- No wallet commits a transition it has not proven. The proof of a transition
  is made after the transition is signed in memory and before the marker for
  its commit is created (§5.2). If the proof cannot be made, the signature is
  discarded, nothing is committed and the wallet is unchanged.
- The payer. Pre-check, hardware signature, proof of its state after the
  SendSplit, marker step and durable commit, then release of the Payment. Its
  wallet shows "sent, not confirmed" until it stores a `Credited` Outcome.
  After a `Refused` Outcome it shows "returned" once its RefundFold is proven
  and committed.
- The receiver. Its checks, which include verifying the payer's proof; its
  hardware signature on the ReceiveFold; the proof of its own state after the
  ReceiveFold, which is what its own next payment needs; the durable commit
  of the ReceiveFold and the `Credited` Outcome; the confirmed marker step.
  Only then does it show the credit and release the Outcome.
- A receiver whose proof fails commits nothing for the credit and shows
  nothing. It tries again while its Request is open. If no credit is
  committed, the Payment is answered `Refused` once the Request has closed
  (§5.2). A refusal is a signed Outcome and needs no proof. The payer refunds
  when it holds that Outcome and has proven its RefundFold.
- The proof of the new state, and what is needed to extend it, are written to
  the key store with the marker (§5.10). A wallet that resumes after its
  files were lost or put back must still be able to pay, and a payment needs
  the latest proof.
- No wallet holds received value that it has not proven, and no unload waits
  for a proof that was put off.

What this does to time, stated without softening.

- Both proofs fall inside the payment: the payer's before the Payment leaves,
  the receiver's before the credit is shown.
- The repository's gates are 10 s for one proof, 1 s for one verification and
  30 s for a complete handoff, all at the 95th percentile
  (`specs/kagemusha_v1_production_readiness.md:119-122`). They are acceptance
  limits. They are not measurements. Under them the receiver may show the
  credit up to 21 s after the payer confirms, before any transfer time is
  counted. With the repo's QR framing the Payment adds 4 to 10 s per pass.
- Nothing is measured. The one recorded figure for a circuit on the
  stock-phone path is far outside every phone gate (§2.2).
- The owner said: "1-2 s should be good ux". A proof-carrying payment does not
  meet that unless each proof takes a fraction of a second on the slowest
  supported phone and the carrier moves about 7.5 KB in under a second. No
  measurement supports either.
- The payer needs the Outcome, and the receiver cannot release it before its
  own proof is done. So the two phones have to stay together for the whole of
  that time. If they part earlier, the receiver still finishes by itself and
  holds the credit, and the payer's wallet stays at "sent, not confirmed"
  until the two meet again (§5.2).

§2.3 lists shapes that could shorten a payment. Each is a choice to evaluate.
None is designed and none has been checked against P1 and PC.

**What this design does not meet today.** Each point is taken up where the
text names it.

- The owner's timing wish. With PC, a proof-carrying payment holds two proofs,
  and no measurement shows either can be made in a fraction of a second.
- "The OS is real" in the sense of an operating system that is uncompromised
  when a payment is signed. No platform gives evidence of that. On iPhone and
  on HarmonyOS NEXT no evidence about the operating system exists at all
  (§2.1).
- Prevention of a fork. A proof does not give it (§2.1).
- A prover. None exists, and the one circuit built for stock-phone keys was
  refused at a width far above the phone gates (§2.2).
- The enrollment proof. The repository lacks three of the gadgets it needs.
  Until it exists the proof rests on the validator quorum for E (§2.1).
- A change of circuit release that reaches offline phones. The proof that
  travels needs a verifying key that never changes; that is not designed.
  Without it a wallet that has not updated cannot check a payment from one
  that has (§2.1, "Policy inputs").
- A wallet that resumes without its files. It can pay only if the latest
  proof is in the key store beside the marker. Key-store entries of that size
  are not tested (§2.3).

**Position.**

- The proof-carrying design with the relation of §2.1 is the target, because
  the owner asked for the proof to carry the constraint about the operating
  system. The repository's record of the owner requirement of 2026-10-01 also
  makes platform and issuer equations and real State proofs in both fields
  mandatory (`specs/kagemusha_v1_production_readiness.md:8-25`). That record
  is the repository's text, not a quotation of the owner. The relation of
  §2.1 keeps one platform equation per hop and moves the issuer's part to
  enrollment; whether that meets the record is for the owner to confirm
  (§12).
- Feasibility on phones is unproven and is stated as such wherever the design
  is described.
- The order is the evidence gate first, then Q0 to Q3 (§2.3). The evidence
  gate tests the marker and the device evidence that every payment rests on
  with or without a proof. No stage of the proof qualification can repair a
  failure there.
- No pool opens for production value before the owner's ruling on the
  evidence gate and the ruling that ends Q3. Both are necessary. Neither is
  sufficient.
- The Pasta implementation is kept. Nothing in §2 recommends removing
  anything (§11).
- The circuit-facing surface of Layer A is provisional until the relation is
  fixed in Q0. §2.3 lists it, and says which parts of Layer A can be built
  before then without rework.

One design holds throughout: one payment format, one value pool per scheme,
one wallet core.

### 2.1 What the proof establishes

Nothing in this section is implemented. Every statement about what a platform
signs comes from vendor documentation or published source code read on
2026-10-02. No device was tested and no vendor chain of a target phone other
than Google's published test vectors was parsed.

**The enrollment statement E.** One per device key. Its digest is part of the
wallet's proven state. Its fields are shown to the receiver of each payment
for the payer only.

| Field of E | Android | iPhone |
|---|---|---|
| Attested key | The device key K (P-256), generated in the secure hardware | The App Attest key, and the payment key K that its attestation names |
| Platform class | Android, with the chain class (remotely provisioned or factory-provisioned) and the root the chain ends at | iPhone App Attest |
| Security level | TEE or StrongBox | Not attested. Apple documents the App Attest key as held in the Secure Enclave |
| Boot | Bootloader locked; verified-boot state Verified; digest of the verified-boot key | No field exists |
| Patch levels | Operating-system, vendor and boot patch levels | No field exists |
| App identity | Package name, signing-certificate digest and version code, as the operating system reported them | The hash of this scheme's App ID; the production environment; from iOS 27 the launch category and the bundle version |
| Key used at renewal. Not a field of E: the certificate and the registry row hold it beside E (§5.1, §8.1) | The public key of the app attestation key, where the phone has one | The enrolled App Attest key, which E names by its identifier |
| Policy entry | The id of the entry the statement was checked against | The same |
| Revocation | A commitment to the serial numbers of the certificates in the vendor chain | The same commitment, over Apple's certificates in the evidence (§5.1). Nothing is tested against it: Apple publishes no revocation list for App Attest keys |
| Enrollment epoch | The ledger height at which the validators sealed E | The same |

The fields of E that a receiver sees are about 50 to 100 bytes (estimate). The
device certificate of §5.1 already carries some of them.

**The enrollment relation, Android.** The evidence is the vendor's certificate
chain down to the certificate of the app attestation key, and one leaf
certificate for the device key signed by the app attestation key. A statement
E is valid for a policy entry when all of these hold.

- EA1. Chain. The signature on every certificate verifies under the public
  key of the certificate above it. The top key is one of the vendor root keys
  in the policy entry: Google's RSA-4096 root key or Google's ECDSA P-384 root
  key.
- EA2. Chain class. The chain is classed as remotely provisioned or
  factory-provisioned from the certificate below the root, as Google's
  reference verifier does (§10.3). The class is one that the policy entry
  admits, and E records it.
- EA3. Where the key description is read. Each key description (the
  attestation extension, OID 1.3.6.1.4.1.11129.2.1.17) is found by walking the
  certificate's DER structure from the top, header by header. It is never
  found by matching bytes: the app chooses up to 128 challenge bytes inside
  the same certificate, so bytes that look like a root-of-trust record can be
  placed there. Counted from the root, the first certificate that has the
  extension must be the app attestation key's. Google states that only the
  first occurrence in a chain can be trusted. The second occurrence, in the
  device key's leaf, is trusted because of EA4.
- EA4. The app attestation key. In the hardware-enforced list: the purpose is
  attest-key and nothing else; the origin is "generated"; both security-level
  fields are equal and are TEE or StrongBox; the root of trust says locked and
  Verified. A key with that purpose alone cannot sign arbitrary data, so
  every certificate it signs was composed by the secure hardware.
- EA5. The device key. In the hardware-enforced list of its leaf: purpose
  sign only; algorithm EC; curve P-256; origin "generated"; the digest that Q0
  fixes (§2.3); no use limit and no user-authentication requirement (§5.9);
  both security-level fields equal to those of EA4; `deviceLocked` true;
  `verifiedBootState` Verified; the same verified-boot key as in EA4.
- EA6. Patch levels. `osPatchLevel`, `vendorPatchLevel` and `bootPatchLevel`
  in the hardware-enforced list are each at or above the floor in the policy
  entry. E records them.
- EA7. App identity. The application id (tag 709) is present in the
  software-enforced list and absent from the hardware-enforced list. The
  all-applications tag (600) is absent. The id names exactly one package and
  one signing-certificate digest, both equal to the policy entry's. If the
  policy entry holds a minimum version code, the attested one is at or above
  it; whether an entry may hold one is the owner's decision on an app-build
  floor (§5.11, §12).
- EA8. Binding. The attestation challenge of both keys is the SHA-256 of the
  enrollment transcript. The transcript holds the scheme id, the account the
  device is bound to, the id of the policy entry and a value the ledger can
  date. The attested device public key is K in E and in the Bootstrap state.
- EA9. Revocation. No serial number of a certificate in the vendor chain is
  in the revocation set whose root the policy entry names. E carries a
  commitment to those serial numbers.
- EA10. Dates. For a remotely provisioned chain, each certificate above the
  two leaves is within its validity period at the enrollment epoch. Google
  says that this must be checked; in Google's test vectors the attestation
  key's certificate lives 9 to 29 days. Only a party that knows the time can
  check it (below).

No list of allowed verified-boot keys is needed to tell a vendor's operating
system from a user-installed one. `Verified` with `deviceLocked` true means
that the boot chain was verified against the root of trust the manufacturer
embedded; a user-set root gives `SelfSigned`. The policy entry may still hold
an allow-list or a deny-list of boot keys by manufacturer. Two of Google's
Pixel 9 Pro test vectors show a zero boot key on a locked, Verified phone, so
whether such a list is usable is a question for the evidence gate.

A phone on which the app attestation key cannot be created, or on which it
fails the evidence gate's test, has no key for EA4. Its device key is then
attested by the vendor chain directly. Its renewal carries a fresh key under
the vendor's chain, whose public key the device key signs. That shows the
boot and patch fields for the hardware that made the fresh key. It does not
show that the device key is in the same hardware (§5.11, §7.1). Whether such
a phone is admitted is part of the owner's decision on pools.

**The enrollment relation, iPhone.** The evidence is an App Attest attestation
object. E is valid for a policy entry when all of these hold.

- EI1. Format and chain. The format is `apple-appattest`. The credential
  certificate verifies under Apple's App Attestation CA 1 key, and that key
  under Apple's App Attestation root key, or CA 1's key is itself in the
  policy entry. Its certificate runs to 2030.
- EI2. App. The RP ID hash in the authenticator data equals the SHA-256 of
  the App ID in the policy entry.
- EI3. The counter is zero and the `aaguid` is the production value.
- EI4. Key. The credential id equals the SHA-256 of the attested public key,
  and the key in the authenticator data is the credential certificate's key.
- EI5. Nonce. The credential certificate's nonce extension
  (1.2.840.113635.100.8.2) equals SHA-256 of the authenticator data followed
  by the SHA-256 of the enrollment transcript.
- EI6. Payment key. The transcript holds what EA8 lists and the payment
  public key K. A P-256 signature by K over the transcript verifies. K is not
  the App Attest key.
- EI7. From iOS 27: the launch category is App Store. If the policy entry
  holds a lowest bundle version, the reported one is at or above it; whether
  an entry may hold one is the owner's decision on an app-build floor, as in
  EA7 (§5.1, §5.11, §12). Where the fields are absent, E records that, and
  the policy entry says whether such a statement is admitted.
- EI8. Dates. The credential certificate is within its validity period at
  the enrollment epoch. In Apple's sample it lives 72 hours.

What EI1 to EI8 state is this: a key that Apple certifies as a Secure Enclave
key, acting for this scheme's App ID in the production environment, named
bytes that commit to the payment key, and the payment key signed the same
bytes. No clause says anything about the operating system. None can: the
attestation carries no operating-system version, patch level, boot state,
device model or jailbreak state. No clause shows that the payment key is
itself a Secure Enclave key; that rests on T2 and T3 at enrollment (§4).

**Evidence that only a vendor's server can give.** A Play Integrity verdict,
Apple's receipt and fraud metric, and the freshness of Google's revocation
list each need a call to the vendor. None can be checked from bytes alone. If
the scheme requires one of them (§12), the issuer applies it at enrollment
or at renewal. It does not enter E (§5.1, §10.3), so no proof and no
receiver sees it, and it rests on the issuer's own check.

**Who establishes E.** Twice over, by two means that fail differently.

1. The validators, natively, at registration. The registration instruction
   (§6) carries the raw evidence: 3.1 to 4.0 KB for an Android vendor chain
   in Google's test vectors, and 0.6 to 0.8 KB more for the device key's leaf
   (an estimate; none was captured); about 1.9 KB for an Apple attestation
   without its receipt.
   Every validator checks EA1 to EA10 or EI1 to EI8 with ordinary library
   code when it executes the instruction. The policy table and the revocation
   set are ledger state, so every validator checks against the same inputs,
   and the time is block time. The registry row stores E. The validator
   quorum seals the registry root with its Pasta keys, by the mechanism that
   seals a mint today
   (`crates/iroha_core_zk/src/kagemusha_v1_recursion/mint_authority.rs`,
   `mint_finality.rs`). This needs no new arithmetic in any circuit. It is the
   only means that anchors time, so it is the only one that checks certificate
   dates and a current revocation set. The party trusted for E is then the
   validator quorum, which the mint already rests on. A stolen issuer key
   certifies nothing. One limit: someone has to post the vendor's revocation
   list to the ledger (§6), and a list posted late admits a chain the vendor
   has already revoked.
2. A server-made enrollment proof. The evidence holds nothing secret, so
   anyone who has the bytes can prove that EA1 to EA9 or EI1 to EI7 hold, and
   the proof is sound whoever makes it. A server proves it once per
   enrollment. The phone's recursion verifies that proof at Bootstrap, in the
   place where the existing Guard verifies a provider-made credential proof
   (`crates/iroha_core_zk/src/kagemusha_v1_recursion/guard_bundle.rs:261-401`).
   No signer is then trusted for the chain check. A proof does not know when
   it was made, so alone it cannot check EA10, EI8 or which revocation set
   was current: an attestation key that leaked after its certificate expired
   would still verify. The repository has no P-384, RSA or SHA-384 gadget, so
   this proof cannot be built today. Building it is a qualification item
   (§2.3). It is made on a server and never on a phone.

Until the enrollment proof exists, the recursion consumes the quorum-sealed
statement: at Bootstrap the relation opens the pair (device id, digest of E)
under a sealed registry root. When both exist, an attacker needs a chain that
verifies in the proof and admission by the quorum.

**The per-hop relation.** A wallet's proven state S holds: the device public
key K; the sequence number; the digest of the last transition; the balance;
the cumulative outflow, refund and fee totals; the redeemed total; the day and
month counters; the digest of E; the last voucher number folded; the flag that
a Migrate was folded; a record of the credits already folded; a record of own
SendSplits that are still unresolved; and the policy head. Q0 fixes the exact
fields and their encoding. For a transition t that takes S to S′:

- H1. Signature. One ECDSA P-256 signature over the signed preimage of t
  (§5: tag, scheme id, rules version, payload) verifies under K from S. This
  is the only non-native signature check on an ordinary hop, on Android and
  on iPhone alike.
- H2. Identity. The device id in t is the hash of K (§5.1). K is the same in
  S and S′.
- H3. Sequence and link. The sequence number of t is that of S plus one. The
  previous digest in t is the digest in S. S′ holds the digest of t.
  A Bootstrap has sequence number 0 and no predecessor.
- H4. Cumulative counters. `cum_out_after` equals the total in S plus the
  outflow of t. `cum_fee_after` and `cum_refund_after` follow the same rule
  with the fee and the refund of t (§5.3, §5.8). The redeemed total grows by
  the amount of a RedeemSplit and by nothing else. The day and month counters
  are added up for the time t names (§5.4).
- H5. Balance. The balance in S′ is the balance in S, less what t debits,
  plus what t credits. It is not negative.
- H6. E is carried. The digest of E in S′ is that in S, unless t adopts a
  renewal. The policy entry that E names is a member of the governed table
  whose head is a public input of the proof.
- H7. Rules by kind.
  - Bootstrap. Balance and counters are zero. E is established as above.
    Bootstrap holds no value, so a server can make this proof too.
  - MintFold. It credits the voucher's amount. The load behind the voucher
    names this device id. It is in ledger state that the validator quorum
    sealed, if the mint stays quorum-sealed in the relation; the other
    choice, a voucher-key signature, is decided in Q0 (§2.3). The voucher
    number is the last voucher number in S plus one (§7.1).
    Folding one voucher twice, or out of order, cannot be proven.
  - SendSplit. It debits the amount and the fee. The fee equals the formula
    of §5.8 applied to the record whose digest is `fee_policy_id`. That this
    record is the one in the payer's certificate is the receiver's check on
    the last hop (L5); for earlier hops no clause checks it. S′ records the
    payment as unresolved.
  - ReceiveFold. The payer's proof verifies for the payer's state after its
    SendSplit. The policy head of that proof is this wallet's policy head or
    precedes it in the governed table. The SendSplit names this wallet's
    device id as counterparty. The credit is the SendSplit's amount. The
    payment id is not among the credits S records, and S′ records it. The
    SendSplit was not signed by K: a wallet cannot fold a payment from its
    own key.
  - RefundFold. It credits the amount and the fee of an own SendSplit that S
    records as unresolved. A `Refused` Outcome for that payment id carries a
    P-256 signature that verifies under the key whose device id the SendSplit
    names as counterparty. S′ no longer records the payment as unresolved.
    This is the one kind with a second non-native signature check.
    A `Refused` Outcome signed by the successor of a Migrate (§5.2, §7.2)
    does not meet this clause as written. Q0 fixes the clause that admits
    it; until then such a refusal cannot be folded under a proof. Where
    the counterparty has migrated, the Outcome is its successor's (§5.2,
    §7.2): the clause then checks the old key's signature on the Migrate
    that lists the Request, and the successor's signature on the Outcome.
    That is one more non-native check; Q0 fixes its form (§2.3).
  - RedeemSplit. It debits the amount.
  - Recertify. No value moves. Where it adopts a renewal, E is replaced
    (below).
  - Migrate and MigrateFold. A Migrate debits the whole balance, names the
    successor's device id and is the last transition of its key. A
    MigrateFold verifies the old key's proof ending in that Migrate, credits
    that balance, adds the old counters as §7.2 says, and sets the
    Migrate-folded flag, which must not have been set. What shows that the
    issuer accepted the Migrate (§7.2) is fixed in Q0.

The rule H4 is what makes a hidden transition useless as padding: a wallet
cannot choose its counters. The self-payment rule in ReceiveFold is what
stops one compromised key from paying itself on a hidden branch and folding
that credit into its visible chain.

A relation that proved only arithmetic and mint authorization, and left the
device signature to the receiver's check of the last hop, would not carry E
for earlier hops. It is not a candidate.

What no clause covers. That a state has only one successor. What time it was
when a transition was signed. Which block list the payer held. That the wallet
used its marker. Whether the terms in a payer's certificate (lease, limits)
were kept by an earlier holder. Expiry, limits and the block list are
therefore enforced as §5.4 and §5.5 say: by the payer's own app, and by the
receiver on the last hop.

**Renewal.** Renewal is the owner's "expiry for attestation" (R8). Where a
scheme has switched R8 on, a renewal refreshes E.

- Android. The renewal request carries one leaf certificate for a key that
  the secure hardware has just generated, signed by the app attestation key
  that the registry row holds beside E (§5.1, §8.1), with the SHA-256 of the
  renewal transcript as its challenge. The
  checks are: the leaf's P-256 signature under that key; EA3 for the leaf;
  both security-level fields as in E; locked and Verified; the three patch
  levels at or above the floor of the current policy entry; the app identity
  of EA7; the serial numbers E commits to, against the revoked set on the
  ledger; and the device key's signature over the renewal transcript (§7.1).
  Whether the verified-boot key must equal the one in E is decided from the
  evidence gate's record of boot keys across updates (§10.4, test h1). The
  new statement E′ is E with what the new leaf states (the patch levels and
  the app's version code), the policy entry and the epoch replaced. It shows
  how the phone booted this time and its patch levels now. The patch floor is
  applied here and at enrollment, and nowhere else.
- iPhone. The request carries an assertion by the enrolled App Attest key
  over the renewal transcript. The checks are one P-256 signature under that
  key, the RP ID hash, a counter above the last one the row recorded, and the
  payment key's signature over the renewal transcript (§7.1). E′ is E with
  the policy entry and the epoch replaced and, from iOS 27, with the launch
  category and bundle version the assertion reports (§5.11). It shows that
  the key Apple certified still signs for this
  App ID. It shows nothing about the operating system. A new attestation is
  not an alternative: Apple refuses to attest a key that is already attested.
- E′ is established as E is. The validators check the request natively when
  the new certificate serial is anchored (§6) and seal the registry. The
  Android leaf needs only P-256 and SHA-256, for which the repository has
  gadgets, so a proof of the renewal relation does not wait for the missing
  ones. It is not built or measured.
- The wallet adopts E′ with its Recertify transition (H7).
- A renewal is refused only for an enabled regulatory control or because the
  device's own key signed two successors. A leaf below the patch floor, and a
  serial number that has entered the revocation set, are refusals under R8
  itself: R8 is the control the scheme switched on, and this is its content.
  The holder then cannot send after the lease ends. Receiving and unloading
  continue (§5.4).
- With R8 off there is no renewal. E stays as it was at enrollment for the
  life of the wallet.

**The receiver's native checks on the last hop.** The receiver makes them
before it signs anything, from the Payment, its own state and its own clock.

- L1. The proof verifies for the payer's state after this SendSplit and for
  one policy head.
- L2. The policy head is one the receiver holds, or the Payment carries the
  entries that lead to it from a head the receiver holds, and their
  signatures verify ("Policy inputs", below).
- L3. The E shown in the Payment hashes to the digest in the proven state.
  The receiver takes the payer's key K from it. The proof has already shown
  that E met the policy entry it names (H6).
- L4. The SendSplit's signature verifies under K from E, and the SendSplit
  answers the receiver's own open Request (§5.2).
- L5. The payer's certificate and receipt verify, and the certificate is for
  K (§5.1). The fee is the one the certificate requires (§5.8).
- L6. The regulatory controls that are switched on: the lease on the
  receiver's own clock, the limits and the receiver's own tally, the block
  list (§5.4, §5.5).

The receiver does not test E against a newer patch floor, a newer list of
admitted classes or a newer revocation set than the ones E was sealed
against. "Policy inputs" below says why, and what acts on such a phone.

The receiver checks nothing time-dependent about earlier hops, and neither
does anyone else. Each earlier holder accepted the value under P1 to P4 when
it received it, and nothing later may undo that.

**What a verified proof establishes.** For every hop h behind the paid value:

1. The arithmetic, sequence, link and counter rules held (H2 to H7).
2. The transition of hop h carries a P-256 signature that verifies under a
   key K_h.
3. Android. For K_h the phone's secure hardware, under a certificate chain
   that ends at a vendor root key of the policy entry, signed a statement
   saying: K_h was generated inside secure hardware at the TEE or StrongBox
   level;
   at the boot during which the statement was made, the bootloader was locked
   and the boot chain was verified against the manufacturer's root of trust;
   the patch levels the secure hardware had been told at that boot were at or
   above the floor of the policy entry named; and the operating system named
   the wallet's package and signing certificate as the caller. The statement
   dates from enrollment, or from the last renewal where R8 is on.
4. iPhone. For K_h there was an Apple-rooted certificate for an App Attest
   key of this scheme's App ID in the production environment, with the launch
   category and bundle version Apple reported on iOS 27, and its attestation
   named a hash that commits to K_h.
5. The validator quorum admitted that evidence at a ledger height it
   recorded, against the revocation set of that height. Where the enrollment
   proof exists, the chain check of item 3 or 4 also holds with no signer
   trusted for it.
6. Every unit of the value entered through a MintFold of a load the validator
   quorum sealed, if the mint stays quorum-sealed in the relation (§2.3).

**The two readings.** The owner's words are "the OS is real". A verified
proof shows that in this reading only: each paying phone's secure hardware was
told at boot that a vendor-signed image was verified on a locked bootloader,
was told patch levels at or above the floor, as of enrollment or the last
renewal, and the operating system named the genuine wallet package as the
caller.

It is false in this reading: the running operating system of each paying phone
was genuine and uncompromised at the moment its payment was signed. No proof
shows that, because no evidence for it exists to check. Every attested field
about the operating system is set once at boot, and the secure hardware
accepts no other value until the next boot. The app identity is supplied by
the operating system; the KeyMint interface states that the hardware cannot
enforce that field. A locked, verified, fully patched phone whose operating
system is taken over after boot by an exploit produces the same chain, the
same root-of-trust fields, the same app identity and the same signatures as an
honest phone, at enrollment and at every hop. Apple's documentation says that
an attacker who modifies the operating system may get around App Attest's
restrictions. Samsung's documentation says that secure boot and
hardware-backed key stores stop being effective once the kernel is
compromised at run time. Exploits of this kind exist: Android's security
bulletins list kernel privilege escalations as being under limited, targeted
exploitation (May 2022, March 2025, September 2025).

On iPhone even the true reading does not apply. No operating-system, patch,
boot or jailbreak field exists. The statement is: a genuine Apple Secure
Enclave, acting for this App ID, certified a key that named the payment key.
HarmonyOS NEXT attestation carries no boot, lock or patch field either.

**Three things stay unproven.** They are assumptions (§4), and the proof
cannot turn any of them into something a receiver can check.

- No run-time compromise of a paying phone's operating system (T2).
- No leaked or extracted vendor attestation key outside the revocation set
  (T1). A published study extracted hardware-protected keys on Samsung Galaxy
  S8 to S21 phones.
- No broken secure hardware (T1).

A fourth limit is not an assumption but a fact about dates. The patch level in
E is the level at enrollment or at the last renewal. A phone that enrolled
above the floor and never renews keeps a valid E while public exploits for its
patch level appear. With R8 on, the lease is the age limit of that evidence.
With R8 off it has none.

**What the relation excludes from every hop of a proven history.** None of
these depends on the issuer's certificate key. The first two are stated by
Android evidence only. An iPhone statement has no boot or lock field, and the
iPhone payment key is named, not attested: that it is a Secure Enclave key
rests on T2 and T3 at enrollment (EI6).

- Emulators and keys held in software: the security level and origin fields
  and the chain to a vendor root.
- Phones with an unlocked bootloader or a user-installed boot key: the lock
  state and boot state.
- A repackaged, re-signed or renamed app on a healthy operating system: the
  attested package and signer, and on iPhone the App ID.
- Keys certified with a stolen issuer key: E is checked by the validators and
  by the enrollment proof, not signed by the issuer.
- Chains signed with a leaked factory attestation key, where the policy entry
  refuses the factory-provisioned class.

So the cost of entering a proven history rises. It was: unlock the
bootloader, or run a modified app on a rooted phone. It becomes: hold a
working privilege-escalation exploit for a locked phone at or above the patch
floor, or an attestation key that has leaked and is not yet revoked, or a way
into the secure hardware. That is a higher cost. It is not exclusion.

**Case by case.** The table uses only what this section, §4.1, §5.3 and §8.1
establish. What happens after an assumption has failed, and what it implies
for value already accepted, is §3.2.

| Case | Signature-only form | With the relation of this section |
|---|---|---|
| An honest wallet with an arithmetic defect | The wallet signs transitions with a wrong balance. No receiver can tell. The issuer's replay at a sync finds it only if the replaying core lacks the defect and the wallet syncs | The wallet cannot prove the wrong transition, so it commits and releases nothing. A soundness defect in the circuit itself is not contained |
| An ordinary user with the unmodified app | Cannot pay twice if the marker of §5.10 holds on that phone; the evidence gate decides that per tuple | The same. A proof does not check the marker |
| A key that is not held by a genuine, locked phone running the genuine app | Refused on the last hop from E, unless the certificate key and the witness quorum are stolen (§8.1). Not looked for on earlier hops | An Android key cannot appear at any hop, unless its chain was signed with a leaked attestation key of a class the policy admits and the revocation set does not yet hold it, or the key was taken out of broken secure hardware after enrollment. An iPhone payment key is named, not attested, so the relation does not exclude one held in software (EI6) |
| A compromised phone with one enrolled key and no accomplice | It signs payments for any amount. It needs no load. Its chain is self-consistent and no evidence need exist | It must load real value once. It cannot fold its own payments. To pay out more than its proven balance it must sign two successors of one state, and the two receivers then hold a pair that is evidence, if both records reach the chain |
| A compromised phone with a second enrolled key, or a receiver who colludes | As the row above | One real load, and a second enrollment that meets every clause of E. One branch is paid to the other key, which folds it. The conflicting signature is then only a private input of that key's next proof. No evidence |
| Theft of the voucher key | Value with no load behind it is minted onto genuine phones | Nothing a proof accepts, if the mint stays quorum-sealed in the relation (§2.3) |
| A leaked vendor attestation key not yet revoked, or broken secure hardware | Passes every check | Passes every clause |
| A run-time compromise of a locked, verified, patched phone | Passes every check | Passes every clause |

**A fork is not prevented by a proof.** The secure hardware signs what the
operating system asks and keeps no memory of what it signed. A compromised
phone signs two successors of one state, and each branch satisfies every
clause above. A renewal on Android adds nothing here: the same phone obtains a
conforming leaf at each renewal.

What a fork leaves behind under this relation, and when.

- A phone with one enrolled key and no accomplice cannot pay honest receivers
  more than its proven balance without leaving evidence in their hands. Take
  any two of its payments that come from different branches. At one sequence
  number they are two digests. At adjacent numbers the link is broken.
  Further apart, the later one fails the cumulative-outflow rule of §5.3,
  because a branch can raise its proven `cum_out_after` only by a proven
  outflow, and that outflow debits the same branch. This argument is this
  document's own. It has not been checked by a second pass or written as a
  test.
- The pair is evidence only when both transitions reach the chain. Each sits
  in one receiver's files, and no holder has to sync. A receiver whose files
  are lost resumes without them (§5.10). So the evidence may never appear.
- Evidence never touches a payment already received (P4). Two conflicting
  signatures by one device key, verified on-chain, place a hold on that
  key's row, or on the row that took over its balance by Migrate (§5.3,
  §8.2). The hold stops that row. It stops no earlier payment.
- None of this holds if one branch is paid to another enrolled key the
  attacker controls, or to a receiver who colludes. A second profile on the
  same phone is enough, if it passes enrollment. Value can then be created
  without evidence for the price of one real load and one more enrollment.

**Policy inputs.** Three kinds of input decide whether an enrollment statement
passes. They change in different ways, and no change sends a phone online.

1. Rules fixed in the circuit. These are the format rules that do not change
   with time: the certificate structure and the signature algorithms accepted
   at each level of a chain; EA3, EA4, EA5 and EA7 apart from the values they
   compare with; EA8; EI1 to EI6 apart from the App ID. Changing one is a new
   circuit release. A phone gets a new release with an app update.
2. The governed table. One entry per policy version, and the enrollment
   relation names the entry it used. An entry holds: the vendor root public
   keys (Google's RSA-4096 key, Google's ECDSA P-384 key, Apple's App
   Attestation root key or CA 1 key); the app identity (Android package name
   and signing-certificate digest, Apple App ID hash); the patch floor per
   platform; the root of the revocation set; the chain classes and platform
   classes admitted; the lowest attestation version; the digests of the
   circuit releases that are accepted. The table is append-only. Each entry
   commits to the one before it, so a proof can show in a few hashes that
   the entry an earlier hop used precedes the entry it uses, and the proof
   that travels names one policy head.
   Governance installs an entry on the ledger, and the scheme root signs it
   as a notice (§5.1), so that it travels peer to peer like every other
   notice (§5.11).
3. What the receiver supplies at payment time: the newest policy head it
   holds, its own block list and its own clock. These are applied to the
   last hop only (L2 to L6). They are never proven for earlier hops.

How a change reaches an offline phone. At a sync; with an app update; or from
the other phone in a payment. A receiver that is behind the payer's policy
head takes the missing entries from the Payment and checks them one by one
from the head it holds. A payer that is behind needs nothing: its proof names
an older head, and the receiver's own fold shows that the older head precedes
the newer one. A wallet that has received a newer entry from nobody goes on
under the older one.

What a change does to proofs already made.

- A root key or an app signer is added. Nothing.
- The patch floor rises. A statement E holds the patch level of the day it
  was made. Only a new attestation refreshes it, and that needs the issuer
  and the ledger. So a floor is applied at enrollment and at each renewal and
  nowhere between, and it bites only where the scheme has switched R8 on.
  A receiver does not apply the newest floor to a payer offline: that would
  send an honest payer online, to update its operating system, for a reason
  that no enabled control names. A phone whose vendor has stopped shipping
  updates falls below a rising floor for good. Google's update commitment
  for the Pixel 6 ends in October 2026; whether a floor applies to such a
  phone is the owner's decision (§12 item 22).
- A vendor attestation key or an app signer is revoked. Enrollment statements
  made before the revocation stay valid inside proofs. Value that passed
  through such a phone and sits in other wallets is not touched (P4). For the
  phone itself the rule is the one §5.1 gives for a certificate under a
  revoked issuer key: its statement stands for as long as its certificate
  does. Where R8 is on, the renewal is refused and the phone stops sending
  when its lease ends. Where R8 is off, the statement stands without limit,
  and what that means after a vendor key has leaked is one of the
  residual-risk decisions of §3.2. The reason for this rule is the
  factory-provisioned class. Such an attestation key is shared by a batch of
  phones; one way the Android compatibility rules describe is one key per
  100,000 units (read from a search summary, not from the rules themselves).
  When one leaks and is revoked, every honest phone of the batch falls under
  the revocation. A receiver that refused them at once would stop honest
  holders for a reason that is not an enabled regulatory control. A scheme
  that wants immediate refusal can have it only as a block entry under R6
  (§5.5). Whether the block authority may enter a device for this reason is
  the owner's decision.
- A class is no longer admitted, for example the factory-provisioned class.
  The same rule: statements already sealed stand for as long as their
  certificates do, and the class is refused at enrollment and at renewal.
- A rule fixed in the circuit changes. Value in circulation was proven under
  the earlier release, and its holders may be offline for years. Every
  release that was ever accepted therefore stays accepted inside proofs, and
  a withdrawn release is not removed from them. The same holds for keys: a
  key that was valid when a proof was made stays accepted inside it. This
  needs a proof that travels whose verifying key does not change between
  releases, with the accepted releases listed in the governed table. It is
  not designed (§2.3). Without it, a phone that has not updated cannot verify
  a payment from one that has. A release found to be unsound cannot be shut
  out offline; that is residual risk (§3.2).

A proof never shows that an earlier hop would pass today's policy. It shows
that each hop passed the policy entry it named.

**Platform classes and pools.** What E says depends on the platform.

| Class | What E says about the operating system | What E says about the key | Known gaps |
|---|---|---|---|
| Android, remotely provisioned chain | Booted locked and Verified, with patch levels at or above the floor, as of enrollment or the last renewal. The app identity is the operating system's own report | Generated in the TEE or StrongBox. The attestation certificate is per device and short-lived | No chain of this class was parsed from a target phone. The dates of EA10 need a party with a clock. On phones that launch with Android 16 this is the only class, and Android's provisioning code runs it only where Google's services app is enabled, which matters for mainland-China builds |
| Android, factory-provisioned chain | The same fields | Generated in the TEE or StrongBox. The attestation key is shared by a batch of phones | Such keys have leaked. Google's revocation list held 1,759 entries on 2026-09-29, 1,733 of them for key compromise. A leaked key signs a chain for a software key with any fields its holder writes. The repository's Pixel 6 StrongBox chain is of this class, under Google's first root (`specs/kagemusha_v1_production_readiness.md:416-421`) |
| iPhone App Attest | Nothing | The App Attest key is a Secure Enclave key by Apple's documentation. The payment key is named, not attested | The attestation names no model and no operating-system version, so an issuer cannot keep an old device out by attested fields. Third-party code shows a jailbroken iPad producing valid attestations for other App IDs; not tested on current hardware |
| HarmonyOS NEXT | Nothing. The documented claims hold no boot, lock or patch field | Generated or imported in the key store, for a named application id | No wallet design exists (§10.2). The chain's algorithms are not known |

The owner named Samsung, Huawei and Meizu among others. Samsung phones on
Google-certified builds are expected to return the Android statement, and so
are Xiaomi, OPPO and vivo phones, which the owner did not name. No chain
from any of them was captured. Nothing was found for Honor or Meizu beyond
their presence in Google's list of certified models. No vendor-specific
attestation (Samsung Knox, Huawei's integrity checks, Xiaomi's trusted-device
token, Tencent SOTER) can be checked without the vendor's server, so none can
enter the relation.

A pool's claim is that of the weakest class it admits. Value moves between
phones, so one admitted phone whose E says nothing about its operating system
puts that gap under every balance its payments reach. A pool also admits
every device that its issuer cannot keep out (§10.4.6).

Whether iPhones, phones with a factory-provisioned chain, and HarmonyOS NEXT
phones are admitted to a pool that claims the constraint about the operating
system is the owner's decision. Refusing the factory-provisioned class
excludes older phones and the Pixel 6's StrongBox key; its TEE chain has not
been classed. Refusing iPhones excludes iPhones. Admitting them means the
pool's claim for every holder is the iPhone statement.

**Optional, and not in the base design.**

- Android: a fresh leaf under the app attestation key at every payment, or
  once per stated number of transitions, checked in the relation. A verifier
  cannot enforce "once per boot": nothing in a leaf names the boot, and its
  creation time comes from the operating system's clock. Per leaf it costs
  one more P-256 verification, 11 to 13 SHA-256 blocks and a structure walk,
  which roughly doubles the non-native work of a hop, and one attested key
  generation on the phone, whose time is not measured. It shows the boot
  state and patch levels of the current boot.
- iPhone: an App Attest assertion on every hop, in place of the payment key's
  signature. The hop is then signed by the key Apple certified. Any lost
  assertion, and any invalidation of that key by Apple after a restore or a
  reinstall, leaves the balance unable to move, with no regulatory control
  involved (§4.1).

Neither shows a run-time compromise: a compromised phone obtains the same leaf
and the same assertion. Both cost time on every payment, and bytes wherever
the receiver checks them itself.

### 2.2 State of the Pasta implementation

As read from source and repo records on 2026-10-02. Line numbers are those of
commit `b2a3cd05bc`. The working tree holds staged, uncommitted edits to
several cited files (`composite.rs`, `guard_bundle.rs`, `pasta_sha256.rs`,
`isi/kagemusha.rs`, the bridge's `Cargo.toml`, `status.md`). There the lines
have moved, and two statements below are marked as true of the commit only.
Not confirmed by build, test or device.

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
  (`.../real_payment_corridor/state_milestone.rs:2879-2974`). It has no
  recorded completed run. RedeemSplit stops after terminal authorization, and
  verification there uses a test-local verifier.
- **Stock-phone keys.** A separate "ordinary" Guard circuit verifies a
  full-width P-256 device signature and a P-256 issuer signature at k = 16 in
  both fields, under a mock prover
  (`.../ordinary_guard_composition.rs:178-181`,
  `.../ordinary_guard_composition_tests.rs:262-323`). The State relation
  contains its consumer, but the production construction refuses it
  (`.../composite.rs:1961-1968`), ordinary ReceiveFold is refused in every
  construction at commit `b2a3cd05bc` (`.../composite.rs:1003-1008`), and the
  only proof test is ignored. A staged, uncommitted edit in the working tree
  replaces that refusal and adds `ordinary_state_receive_consumer.rs`. It was
  not read for this document. App Attest and KeyMint classes are rejected at
  the three monetary folds. The repo records key generation for this path as
  blocked at 8,584 advice columns against a 1,024 ceiling (`status.md:168-171`
  at commit `b2a3cd05bc`; the file is being edited in the working tree and the
  lines have moved).
- **Four P-256 equations per transition.** As built, that Guard runs four
  full-width P-256 equations for each transition: the issuer's signature over
  the credential admission message (`.../ordinary_guard_composition.rs:207`),
  the issuer's signature over an integrity lease or a fixed pad (`:293`), and
  both an Android and an Apple platform equation, one on real data and one on
  a pad (`:304`; `.../ordinary_platform_union.rs:3`, "Both equations
  execute"). The relation of §2.1 needs one per ordinary hop (§2.3).
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
    path the figure belongs to, or which part of it produces the width. By
    the published halo2-lib profile, four P-256 equations would be on the
    order of 40 columns at k = 16, so the P-256 equations do not account for
    8,584; one automated estimate is that the width comes from the carrier or
    SHA-256 layout. No build confirmed either. The record gives no k; k = 16
    is what the ordinary Guard builder uses. It is not known whether the
    width is a layout defect that can be removed, like the 6,738-column
    regression, or the size of the relation. The ordinary path is also bound
    to the online profile's objects, and Q1 re-specifies it (§2.3).
  - What follows. The figure sizes a refused build of unknown composition.
    It shows that one circuit on the stock-phone path, as built, is two
    orders of magnitude wider than the phone gate allows, by the repo's own
    conversion. It is not a measured memory requirement of the relation of
    §2.1, and no such measurement exists.
- **P-256 cost.** Unmeasured. The gadget is a bit-serial ladder with complete
  additions on three 87-bit limbs
  (`crates/iroha_core_zk/src/kagemusha_p256_curve_gadget.rs:42-43`). The
  reduced-window tests run at k = 18 as a test choice; the full-width equation
  is built at k = 16 by widening columns, and its capacity test is ignored
  (`.../kagemusha_p256_curve_gadget.rs:1160-1161`). No cell count, key size,
  memory or time is recorded.
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
- **Phones.** No on-device harness runs this prover. At commit `b2a3cd05bc`
  the mobile bridge enables the prover feature unconditionally
  (`crates/connect_norito_bridge/Cargo.toml:42, 49`). A staged, uncommitted
  edit in the working tree removes `proofs-halo2` and
  `kagemusha-production-prover` from those lines.

**What the repository does today about evidence of the operating system.** No
circuit checks anything about it.

- Where it is checked. Verified-boot state, lock state, patch level and app
  identity are checked only in the issuer's Python.
  `python/iroha_app_attestation/src/iroha_app_attestation/attestation.py:974-1061`
  verifies an Android chain to a pinned root, takes the key description from
  the certificate nearest the root, and requires a hardware key, the pinned
  package and signer, `deviceLocked` true, Verified, a non-zero boot key and a
  32-byte boot hash (`:1049-1058`). The same file verifies an Apple
  attestation (`:574-652`).
- What is not checked anywhere. The verified-boot key is not compared with
  any list of manufacturer keys. Patch levels are parsed and not compared in
  that file; `attested_enrollment.py:178-217` lowers the tier when a
  configured floor is not met, and with the default floor of zero nothing is
  checked.
- What the data model holds. No field for boot state, lock state or patch
  level. The credential the issuer signs carries a platform class, a security
  level and digests of policies and of the raw evidence
  (`crates/iroha_data_model/src/kagemusha/kagemusha_ordinary_app_enrollment_v1.rs:857-914`).
- How it reaches a proof. As a one-byte platform class tag with a fixed
  guarantee mask
  (`crates/iroha_core_zk/src/kagemusha_v1_recursion/guard_bundle.rs:261-315,
  867-897`), as a SHA-256 digest of the raw attestation that no circuit opens,
  and as digests of policies. A proof made with these circuits would show
  that an authority holding the provider secret, or the issuer's P-256 key,
  vouched for a device key under a class. It would show nothing that the
  secure hardware or the vendor signed.
- The staged circuits. The circuit stages for an App Attest assertion and for
  a KeyMint one-use head are built only by unit tests
  (`.../composite.rs:10-28`). The live paths refuse both classes
  (`.../guard_verifier.rs:87-103`, `.../composite.rs:1547-1568`).
- A second proof engine.
  `crates/iroha_core_privacy/src/privacy_engines/zk_x509/` verifies X.509
  chains inside a proof for another product. It is a STARK for P-256 with
  SHA-256 only. Its own record gives a proof of 9,420,938 bytes
  (`.../zk_x509/proof_size_redesign.md:3`), a prover ceiling of 12 GiB and a
  target of 300 s (`.../zk_x509/profile.rs:181, 191`), and says that
  activation is unavailable. It cannot verify a Google or an Apple chain, and
  it is not a Pasta proof this recursion could consume. It is the
  repository's only record of what a strict X.509 relation costs.

**Cost of checking vendor evidence inside a proof.** The first table is
measured from published files. The second is published figures from other
proof systems. The paragraph after it is this document's estimate for this
stack. Nothing was built.

What one chain contains, measured by parsing published samples.

| Evidence | Bytes | Signatures to verify | Hashing | Source of the sample |
|---|---|---|---|---|
| Android factory-provisioned chain, four certificates | 3,613 | One ECDSA P-256; one ECDSA P-384 over a SHA-256 digest; one RSA-4096 | About 25 SHA-256 blocks | Google's verifier test data, Sony Xperia 10 III |
| Android remotely provisioned chain to the P-384 root, five certificates | 3,113 to 3,305 | Two ECDSA P-256; one or two ECDSA P-384 with SHA-384 | About 18 SHA-256 blocks and 6 to 10 SHA-384 blocks | Google's verifier test data, Pixel 9a and one Android 17 vector |
| Android leaf under an app attestation key | 600 to 800 (estimate; none captured) | One ECDSA P-256 | 11 to 13 SHA-256 blocks | Leaves in Google's vectors are 694 to 799 B under a system key |
| Apple App Attest attestation, two certificates | 1,057 and 583 | One ECDSA P-384 over a SHA-256 digest; one ECDSA P-384 with SHA-384, or CA 1's key pinned | About 15 SHA-256 blocks for the certificate, 4 to 5 for the nonce, 2 for the key id | The Apple sample in `python/iroha_app_attestation/tests/fixtures/` |

Published figures. None is for a Halo2-family prover on Pasta with
inner-product commitments except the first.

| Primitive | Figure | System | Source |
|---|---|---|---|
| ECDSA P-256 | 40,000 constraints, message pre-hashed. No time or memory given | Kimchi on Pasta | o1js pull request 1885 |
| ECDSA P-256 | k = 16 with 8 advice and 2 lookup columns, about 0.5 million cells; 4.38 s on a laptop | halo2-lib, KZG on BN254 | webauthn-halo2 benchmark (evidence appendix, section 3) |
| ECDSA P-256 | 1,972,905 constraints; 26 s on a desktop; 1.2 GB proving key | circom, Groth16 | PSE circom-ecdsa-p256 |
| ECDSA P-384 | 4,429,227 constraints; 2.25 times the circom P-256 figure. Marked by its authors as not audited | circom | crema-labs ecdsa-p384-circom |
| P-256, P-384 and RSA-2048 inside an X.509 check | 11.8, 47.8 and 17.4 million cycles; about 5 minutes on a laptop. P-384 is about 4 times P-256 | SP1 zkVM with a Groth16 wrap | arXiv 2603.25190, a preprint |
| RSA-2048 with SHA-256 | 536,212 constraints | circom | circom-rsa-verify |
| SHA-256, one block | About 2,267 rows of one lane; a lane at k = 16 holds 28 blocks | This repository, on Pasta | `crates/iroha_core_zk/src/pasta_sha256.rs:36-39` |
| SHA-384 or SHA-512, one block | No published figure found | | |
| Android key attestation in a proof | A two-certificate chain with P-256 only: about 30 s on a Pixel 9a. P-384 and RSA-4096 are listed as future work | Noir UltraHonk, universal setup | Yamamoto et al., "Anastasia", a talk, 2025-10-21 |
| RSA in a Halo2 prover on phones | Reported to crash on an iPhone 15 Pro and a Pixel 6 Pro because it needs about 5 GB of memory | Halo2 with KZG | zkmopro.org performance page |

Estimates for this stack. They are this document's arithmetic. No measurement
is behind any of them.

- The repository has no P-384, RSA, SHA-384 or SHA-512 gadget in
  `crates/iroha_core_zk`.
- P-384. The integer check that the P-256 gadget uses needs six limbs of 87
  bits for a 384-bit modulus where P-256 needs three. Limb products per
  multiplication go from 9 to 36 and the ladder has 384 steps, not 256.
  Estimate: 4 to 6 times one P-256 verification.
- RSA-4096 with exponent 65537. Seventeen modular multiplications on numbers
  of about 92 limbs. Estimate: on the order of one million cells, two to
  three times an optimized P-256 verification.
- SHA-384. Estimate: about 2.5 times a SHA-256 block, for a block twice as
  long.
- The structure walk for a key description is about 40 to 60 headers. It is
  small next to the signatures. Its engineering and review cost is not small:
  layouts differ by attestation version and by vendor.
- One Android factory chain in all: between 50 and 150 advice columns at
  k = 16 if the gadgets are as compact as halo2-lib's, which is 100 to
  300 MiB for one advice bank and several times that for the process. The
  repository's own P-256 ladder is unmeasured and may be several times wider.
  That is the size of circuit the repository builds on a host. It is one to
  two orders of magnitude over the 128 MiB phone gate. It has never been
  built.

What follows from the figures. Checking a vendor chain on a phone is not
practical. Checking it once on a server is plausible by size and is
unmeasured. The per-hop work is one P-256 verification, for which no phone
measurement exists in this proof system or in any recursive construction.

### 2.3 Qualification plan

The plan has two parts, in this order.

First the evidence gate (§10.4). It tests, on the target
phones, what every payment rests on with or without a proof: that a payer
cannot bring back an earlier state and pay from it, that a receiver's credit
survives and can be paid onward, that a complete payment finishes its
processing before the wallet reports it, and what the attestation of each
phone contains. It needs no prover and no Layer A byte format. It ends in an
owner ruling in writing: which tuples are supported, under which stated
assumption, or that the criterion cannot be met on stock phones.

Then the stages Q0 to Q3. They qualify the proof of §2.1. They start only
after that ruling, and only for the tuples it supports. No stage of Q0 to Q3
can repair a failure of the evidence gate, because a proof does not check the
marker (§2).

The plan has to end in a decision. So it fixes five things before any
measurement: the pass conditions of the evidence gate, the targets, a budget
and a date for each stage, a limit on how often the relation may be
reconsidered, and what each outcome leads to. Each stage has a named person
responsible for it (§12).

**Evidence gate.** The owner first fixes the list of phone models, the time
target and whether it is a gate, and whether P2 must hold against a
compromised operating system. The tests then run. For the proof, three of its
results matter most.

- The attestation fields of an enrollment chain on each tuple: the root, the
  chain class, the algorithms and sizes at each level, the root-of-trust and
  patch fields, the app identity. The enrollment relation of §2.1 is written
  against Google's test vectors until these exist.
- Whether an app attestation key can be created on each tuple, whether a leaf
  it signs carries the current root-of-trust and patch fields, the leaf's
  size, the time to make one, and whether it works with no network after the
  phone's system attestation certificates have expired. Renewal on Android
  depends on it.
- The time of the exchange without a proof, on each carrier. This is the time
  every payment spends. What is left of the owner's target is what the two
  proofs may use.

The gate ends when every tuple on the list has a recorded finding and the
owner has ruled on it. It has no effort estimate.

**Q0 — targets and relation.** Three steps, in this order.

Targets. The owner fixes the targets and the budget table below before any
measurement, so that no result can move them silently. A target changed later
is an owner decision, recorded with the measurement that prompted it.

| Target | Current repo gate | To be fixed by the owner |
|---|---|---|
| Whole payment: from the payer's confirmation to the receiver showing the credit, with both proofs and the transfer inside, p95 | None defined. The gates below for its parts sum to 21 s before any transfer | ____ . The owner said "1-2 s should be good ux". The percentile, the end points, and whether a proof-carrying payment must meet the same figure are for the owner to state |
| Whole payment, to the payer showing complete: the same, plus the Outcome crossing back | None defined | ____ |
| The payer's proof (SendSplit), p95 | 10 s proving | ____ |
| The receiver's proof (ReceiveFold), p95 | 10 s proving | ____ |
| Proof of a RefundFold, a RedeemSplit, a Recertify and a MigrateFold, p95 | 10 s proving | ____ |
| Verification of an incoming payment, p95 | 1 s | ____ |
| Complete handoff: send, payment and the receiver's fold across two phones, p95 | 30 s | ____ |
| Peak process memory | 128 MiB (repo-defined, not a platform limit) | ____ |
| Proving key / verifying key | 64 MiB / 64 KiB | ____ |
| Energy per payment | A measurement is required by the release model; no limit is set | ____ |
| Payment message, binary | 7,552 B payment; 9,211 B whole exchange; 6,528 B proof | ____ |
| Proof store: the proof entry of §5.10, the key-store entry that holds the latest proof and what is needed to extend it | None defined | ____ bytes, as a bound that does not grow with the wallet's history |
| Marker step with the proof store written, on each tuple | None defined | ____ |
| The largest transition of each kind is provable within the memory target on every supported tuple | None defined | Required. A kind that cannot be proven on a tuple makes that tuple unsupported for this design |
| Enrollment proof on a server: wall time and peak memory for one chain | None defined | ____ |
| Host budget for Q1: wall time and peak memory for one proof | None defined | ____ |
| Phones | Pixel 6 is the repo's mandatory profile | The tuples the evidence gate's ruling supports |

The repo gates in the table are from
`specs/kagemusha_v1_production_readiness.md:119-122`. The memory target has to
be settled here: it is raised, the SHA-256 claim chain is removed from the
relation, or the Claim is split as the phone-algorithm spec sketches (slice
arithmetic test-only, the rest unimplemented; memory unmeasured).

There is no target for "how often a proof fails". A wallet whose prover is
stopped by the platform retries, and nothing was committed. A transition that
can never be proven on a phone leaves value on that phone that cannot be paid
onward, which P1 does not allow. The relation must therefore have a bound on
the memory and the stored data that each transition kind needs, whatever the
wallet's history, and Q2 tests the largest case of each kind.

Measurements that need no prover. They run beside the relation work, on the
supported tuples, over at least 100 operations each, recorded as median and
p95.

- Carrier throughput for a Payment of 7.5 KB, in each direction, on each pair
  of phones, on each carrier of §5.6: the repo's QR framing at 5 and 12
  frames per second, a denser QR framing, NFC where the pair allows it, and
  each radio carrier §5.6 keeps. Time from the first frame or tap to a
  complete decode, second passes included.
- The marker step with a key-store entry of 7 KB, 14 KB and the proposed
  proof-store bound: creation, read-back and deletion (§5.10). On iPhone,
  keychain items of tens of kilobytes are not tested.
- Signing a caller-supplied 32-byte digest with the device key, on Android
  for a TEE key and a StrongBox key and on iPhone for a Secure Enclave key,
  unless the evidence gate has recorded it. It settles whether the "Algebraic
  signed digest" choice below is open on each phone.
- Background execution. A stand-in computation sized to the proving and
  memory targets is started after a payment. Record whether it completes
  with the app in the background, with the screen locked, in low-power mode
  and under memory pressure, and after how long the platform suspends or ends
  it. The base design proves in the foreground and does not need this. The
  candidate shapes at the end of this section do.

Relation. The relation of §2.1 is fixed, after deciding the relation choices
listed below, with predicted rows, columns, key size, one-bank memory and
proof-store size. A relation whose prediction misses the memory, key or
proof-store target is changed before Q1.

The budget table. Blanks are the owner's. No value is proposed. The "Basis"
column says what is known.

| Stage | Basis | Budget | Date |
|---|---|---|---|
| Evidence gate | Not estimated. The work is listed in §10.4 | ____ | ____ |
| Q0, targets and relation | Not estimated. It includes the relation choices below and the measurements that need no prover | ____ | ____ |
| Q1, host proofs | An automated estimate gave 8 to 14 engineer-weeks for the proofs of a per-hop relation shaped like Recursive V1. It did not cover the enrollment relation and its three missing gadgets, the renewal relation, the refund path or Migrate. An automated estimate for the optional stack check was 1.5 to 4 engineer-weeks | ____ | ____ |
| Q2, phone proofs | An automated estimate gave 3 to 5 engineer-weeks, once Q1 delivers keys and a witness. It did not cover two-phone timing with a real carrier | ____ | ____ |
| Q3, soundness and ruling | Not estimated. It includes an independent review | ____ | ____ |
| Total, all stages and one reconsideration | The stages that have an estimate sum to about 12 to 23 engineer-weeks, for one engineer who knows this stack. That sum leaves out every stage and item marked not estimated | ____ | ____ |
| Reconsiderations of the relation or proving system allowed | Each one is a new relation and circuit stack, which this document cannot estimate. One is the value to confirm or change | ____ | |

Layer A has no estimate in this document. One is needed before the owner can
see the cost of the whole design.

**Q1 — host proofs.** One complete lineage with real proofs in both fields, on
a host: Bootstrap from a quorum-sealed E; MintFold with the voucher number;
SendSplit; ReceiveFold of another party's payment; a signed refusal and the
payer's RefundFold; RedeemSplit; Recertify that adopts a renewed E; Migrate and
MigrateFold; receiver-side verification; node-side verification with the
authenticated verifier; and one history that crosses a change of circuit
release. Record time, memory, key size, proof size and proof-store size,
separately for the P-256 verification and for the rest. This needs: the
production refusal lifted; an ordinary ReceiveFold consumer; key generation
under the column ceiling; and a Guard specified again against Layer A's
objects, because the existing one is bound to the online profile's approval
objects. Passing the column ceiling is not passing the memory target (§2.2).
A relation that misses the host budget uses the one reconsideration, or goes
to the ruling.

Three items belong to Q1 beside the lineage.

- One P-256 equation per hop. The existing ordinary Guard runs four (§2.2).
  The relation of §2.1 needs one: the device signature under the key in the
  proven predecessor state. Both issuer equations go, because E is
  established at enrollment and carried in the state. The two platform
  equations become one, because an Android device key and an iPhone payment
  key both sign plain ECDSA P-256 over SHA-256. RefundFold is the only kind
  with a second equation.
- The enrollment proof, on a server. It needs gadgets the repository does
  not have: ECDSA P-384, RSA-4096 with PKCS#1 v1.5, SHA-384, a structure walk
  over DER, and the fixed-profile rebuild of Apple's CBOR. §2.2 gives the
  estimates; they are estimates only. Two reductions are available: accept
  only Android chains that end at Google's P-384 root, which need no RSA; and
  pin Apple's CA 1 key, which saves the P-384 check with SHA-384. Deliverable:
  one enrollment proof of a real Android chain and one of a real Apple
  attestation, wrapped into the form the recursion consumes, with server time
  and memory recorded, and the time a phone takes to verify the result once.
  If it cannot be built within the host budget, the recursion keeps consuming
  the quorum-sealed E. The proof then rests on the validator quorum for E,
  and the owner is told so in writing. That outcome does not stop the plan.
- A stack check, optional and time-boxed, before the lineage. Run the
  existing ignored lineage tests for the secure-hardware relation under an
  optimized profile. This shows whether the stack can produce a proof at
  all. It is not a baseline for the relation of §2.1 and not a gate.

**Q2 — phone proofs.** No harness exists; it needs a bench entry point,
host-generated keys and captured witnesses, and iOS and Android test wrappers.
No wallet commits a transition it has not proven (§2). So PC puts the
payer's proof and the receiver's proof inside one payment, and the quantity
to measure is that payment from end to end, carrier time included, not one
proof. On each supported tuple, over at least 100 operations each plus a
sustained run to thermal steady state:

- with two phones and a real carrier, the time from the payer's confirmation
  to the receiver showing the credit, and to the payer showing complete, with
  the share of each part: the payer's proof, the transfer, the receiver's
  verification, the receiver's proof, the marker steps with the proof store,
  the commits, the Outcome;
- each proof alone, with peak memory: SendSplit, ReceiveFold, RefundFold,
  RedeemSplit, Recertify, MigrateFold, and the largest case of each kind;
- a receive followed at once by a payment onward from the same phone;
- energy, from the fuel gauge or a power monitor;
- the app killed at each point of a proof, and the phone locked while a
  proof runs: the wallet must end in the state before the transition with
  nothing committed, or complete the transition once it is ready again
  (§5.10);
- a resume with the files removed (§5.10): the wallet must pay offline from
  the proof store alone;
- message bytes.

Each proof and each payment is run again after an operating-system update on
the tuple.

**Q3 — soundness and ruling.** A relation that met the targets enters soundness
qualification: a mutation test per constraint, the 1,024-handoff run,
independent review. The mutations for the enrollment relation include: a
key description placed inside the challenge bytes; an extension in a
certificate below the first one that has it; an unlocked phone; a `SelfSigned`
boot; a wrong package or signer; a patch level below the floor; a serial in
the revocation set; an app attestation key with a second purpose; and for
Apple a development `aaguid`, a non-zero counter and a nonce that does not
cover the payment key. Each is also run against the validators' native check,
and the two must reject the same inputs. Passing targets is not qualification.
The stage ends in the owner's ruling in writing.

**What each outcome leads to.** A stage ends at its exit result, at its date,
or when its budget is spent, whichever comes first. No stage extends itself.

| At the end of | Outcome | It leads to |
|---|---|---|
| Evidence gate | Every tuple on the list is unsupported | No Q stage starts. The owner chooses among the options §10.4 names |
| Evidence gate | Some tuples are supported with a stated assumption and none is supported without one | The owner rules in writing whether the stated assumption is accepted. If it is, Q0 starts for those tuples and the residual-risk question of §3.2 is put. If it is not, as the row above |
| Evidence gate | The exchange without a proof misses the time target on a carrier the owner requires | The owner changes the target or the carrier before any prover work |
| Q0 | No required carrier moves a 7.5 KB Payment in the time the exchange without a proof leaves | A smaller proof, another carrier, or a longer target for a proof-carrying payment. The owner picks one in writing before Q1, or the ruling |
| Q0 | The relation has no predicted fit to the memory, key or proof-store target | The one reconsideration. If it is already used, the ruling |
| Q1 | The lineage completes within the host budget | The relation goes to Q2 |
| Q1 | The lineage does not complete | The one reconsideration, within the remaining budget and date. If it is already used, the ruling |
| Q1 | The lineage completes and the enrollment proof does not | The relation goes to Q2 with the quorum-sealed E. The owner is told in writing what the proof then rests on |
| Q2 | Every target is met on every supported tuple | The relation goes to Q3 |
| Q2 | Memory and every per-proof target are met, and the whole payment misses the owner's time target | The candidate shapes below are evaluated, within the remaining budget and the one reconsideration. A shape is taken up only if it shows what its entry requires. If none does, the ruling |
| Q2 | Some supported tuples meet every target and others do not | The owner rules: a pool of the tuples that passed, or the ruling below |
| Q3 | The relation passes | The owner rules in writing that the proof-carrying design is qualified for those tuples, with the measured figures beside the tables of §2 |
| Q3 | The relation fails | The defect is fixed within the remaining budget, or the ruling |
| Any stage | Its date passes or its budget is spent without the exit result | Work on the stage stops. The ruling |

**The ruling when no shape meets PC in a time the owner accepts.** The owner
chooses, in writing, among the options that §10.4 names (10.4.6)
for a criterion that cannot be met on the phones tested. This document chooses
none. Read for the proof, those options are:

- Hardware that runs wallet logic. With it, one successor per state rests on
  the hardware, and what the proof must show changes. The owner said of
  smart cards that "there are no plans right now to use", and that "we don't
  assume running in secure hardware as we don't have oem access" (§1; §12
  item 3).
- A narrower device list: only the tuples on which the proof met the targets.
- Accepting a stated assumption. Here that means accepting, for every earlier
  hop, the assumptions of §4 in place of a proof. That is the signature-only
  form, and §2.4 says what it lacks. It gives up what the owner asked the
  proof to carry.
- No production release of offline value.

Two further choices exist for the proof alone, and they are also the owner's.

- A different target: a stated, longer time for a proof-carrying payment.
- One named constraint is relaxed, and one more cycle runs under a new budget
  and date. The candidates are the absence of a trusted setup, the 10 KB
  bound, and the list of phones.

A passed Q3 qualifies the proof. It is necessary for a production release and
not sufficient.

**Candidate shapes that could shorten a payment.** Each is a choice to
evaluate. None is designed. None has been checked against P1 and PC. None is
part of the design until it has shown what its entry requires.

1. A bounded native tail. A wallet keeps a proof of an earlier state and a
   tail: the signed transitions it has made since, up to a fixed number. A
   Payment carries the proof, the tail and the new SendSplit. The receiver
   verifies the proof and applies H1 to H7 to each tail transition and to the
   SendSplit by its own native check. It commits its ReceiveFold on those
   checks, with no proof of its own, and the ReceiveFold joins its own tail.
   Each wallet folds its tail into a new proof later, in the background. Both
   proving times then leave the payment. What remains inside is the transfer
   of a larger message, one verification for each proof carried and one
   signature check for each tail transition. A proof of the payer's state
   made before the payment, with the signed SendSplit checked natively, is
   the same shape with a tail of one. The shape gives up the rule that no
   wallet commits a transition it has not proven. What would have to be
   shown for it to satisfy P1 and PC:
   - Onward spending at once. When the receiver shows the credit it must be
     able to pay that value onward with nothing more computed. Its Payment
     then carries its own proof, its tail and, for a ReceiveFold in the tail,
     the whole Payment that was received, proof included. With the repo's
     6,528 B proof ceiling that is about 15 KB after one receive (computed),
     and it grows by about 7.5 KB for each further phone that passes the
     value on before anyone folds. R9 says about 10 KB. So the proof must be
     much smaller, or R9 relaxed, or the number of unfolded receives bounded.
     If that number is zero, a receiver proves its fold before it shows the
     credit, as in the base design, and only the payer's proof leaves the
     payment.
   - The bound. Once the bound is reached, the next payment waits for a fold.
     In a quick chain of payments the wait falls inside a payment, and that
     payment takes as long as in the base design. The worst case is therefore
     not shortened. PC is met only if the owner reads a wait on the phone
     itself, offline, before a later spend, as leaving P1 intact. This
     document does not take that reading for the owner. If the owner does not
     take it, the shape fails PC.
   - The same acceptance. The native check and the relation must accept
     exactly the same transitions. If the native check accepts one that the
     relation cannot prove, a receiver holds a credit that it can never fold,
     and at the bound it cannot spend it; no remedy exists on the phone. Q3
     would run every mutation against both. A defect common to every wallet
     core passes every native check and shows only at the fold, so the
     containment of arithmetic defects is lost for as many hops as the bound
     allows.
   - Durable before the credit is shown. The tail and the received Payments
     in it must be in the key store (§5.10) before the wallet shows the
     credit. A wallet that resumes without them has its balance and can
     neither pay nor fold. That is up to about 7.5 KB per unfolded receive in
     the marker step; key-store entries of that size are not tested.
   - The fold completes. Each platform must let the wallet finish a fold in
     the background (the Q0 measurement). Where it does not, the fold runs in
     the foreground at the next use and its time returns there.
   - The receiver's verification of the largest allowed Payment fits the
     time target.
2. A faster path for the bytes. NFC where the pair allows it, a Bluetooth
   connection, or a denser QR framing for the 7.5 KB Payment (§5.6); or the
   payer's proof crossing while the payer is still confirming. The second
   shortens the measured interval and not the time the two phones are held
   together. Neither changes the proving time.
3. Less work per proof. The relation choices below that shrink a step:
   circuits per operation, an algebraic signed digest, the quorum seal
   confined to the steps that need it, a smaller record of folded credits.
   Their effect is not measured.
4. A different proof system. This relaxes a named constraint and belongs to
   the ruling above. A circuit-specific setup has published sub-second phone
   timings for single non-recursive proofs. A universal setup has published
   Pixel 6 timings of about 1 to 8 s for single circuits. A message cap of
   tens to hundreds of KB admits hash-based recursion. No published phone
   measurement for merging two parties' proofs was found for any of these.
   With no setup and 10 KB both fixed, Halo-style accumulation is the only
   family for which an implemented cross-party merge was found. That is an
   absence of counter-examples, not an impossibility result. "No trusted
   setup" is not among R1 to R9. No KAGEMUSHA spec states it; it follows
   from Recursive V1's IPA stack and the workspace ZK policy, which rejects
   trusted-setup backends (`specs/zk_envelopes.md:8`,
   `specs/zk_cryptographic_audit.md:214, 1186`). Whether it binds here is an
   owner decision (§12 item 28), and relaxing it changes that policy too.

**What can be built before the relation is fixed.** Every signed preimage
starts with `tag ‖ scheme id` (§5). Work done before Q0 completes uses a test
scheme id. Its encodings, vectors and enrolled keys are discarded when the
relation is fixed. This rule is what keeps early work from fixing the relation
by accident.

Built now without rework, because they do not depend on any signed byte
format:

- the evidence gate's tests and the Q0 measurements that need no prover;
- the carriers of §5.6, which move opaque bytes and must be sized for 7.5 KB
  as well as 1 KB;
- the attestation verifiers of §10.3, written so that the validators and the
  issuer run the same checks, EA1 to EA10 and EI1 to EI8, except the one
  value that says which digest an Android key must be authorized for;
- storage, the durable commit and the marker (§5.2, §5.10), with room for the
  proof store;
- the block index and the derivation of the list (§5.5);
- the pool accounting of §8.

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
- the layout of E and of a policy entry;
- the digest authorization of the device key. On Android it is fixed and
  attested at key generation, so no production enrollment happens before
  this is settled;
- the signature schemes of the certificate, the receipt and the voucher, and
  the form of the validators' seal;
- the shape of the load and unload instructions, the voucher number, and
  what a registration and a renewal carry on-chain;
- the payment envelope, the Outcome and the refusal;
- what the receiver learns about the payer (§5.7);
- the shape of the fee schedule and how a SendSplit binds `fee_policy_id`
  (§5.8);
- the proof store (§5.10);
- every conformance vector.

Relation choices to decide in Q0:

- **Algebraic signed digest.** Android documents signing a caller-supplied
  32-byte digest (`DIGEST_NONE`). The Secure Enclave API documents signing a
  digest the caller provides; that it accepts 32 bytes that are not a SHA-2
  output is an inference and needs a device test. App Attest assertions cannot.
  It removes SHA-256 from the device-signature check. SHA-256, and with it the
  claim chain, leaves the relation only if the state-commitment head is
  algebraic too. It does not remove the non-native P-256 arithmetic, which is
  the larger cost. Caveats: the hardware then signs whatever 32 bytes the app
  supplies; the repo's Poseidon parameters are not independently qualified;
  StrongBox support is test-enforced rather than mandatory and needs a device
  check. Android fixes and attests the digest authorization at key generation,
  and the repo's key policy is SHA-256-only. The key must be generated with
  `DIGEST_NONE` authorized before the first enrollment, or choosing this later
  re-enrolls every Android phone.
- **Mint authorization.** Today a mint is an exact quorum of validator
  Pasta-Schnorr seals over a top-up root plus a recursive authority chain,
  verified as two inner proofs on every step. An issuer signature instead
  moves mint authenticity from validator keys to the voucher key: a stolen
  voucher key then mints value every proof accepts. E is sealed by the same
  quorum, so the seal stays in the relation in any case. The choice is
  whether to confine it to the steps that need it: Bootstrap, MintFold,
  Recertify and MigrateFold.
- **The record of folded credits.** The current depth-256 path costs 3,084
  Poseidon permutations per field per step (counted from source). A
  replacement must keep non-membership-then-insert in one step and the full
  credit id in the leaf, state its capacity, and never prune by time. With
  single-use Requests a per-receiver receive counter can replace the set for
  ReceiveFold. For MintFold the voucher number is the replay guard (H7).
  Whatever is chosen must have a size bound, because the proof store has to
  hold it (§5.10).
- **The refusal and the refund.** A refusal is a signed Outcome, not a proven
  transition, so that a receiver that cannot prove can still refuse. The
  payer's RefundFold proves the check of H7. Q0 fixes how the state records
  unresolved SendSplits within a size bound, and what the wallet does with a
  refusal for a payment beyond that bound. A receiver whose phone is
  compromised can fold a payment and also sign a refusal; the two objects are
  evidence against it under §5.3 if both reach the chain, and the relation
  does not prevent it.
- **Circuit releases.** The proof that travels must verify under a key that
  does not change between releases, with the accepted releases listed in the
  governed table (§2.1, "Policy inputs"). This needs a uniform wrap proof or
  verifying-key selection. Not designed. Q0 states what happens if the wrap
  itself must change: no rule may then stop a wallet that has not updated.
- **Per-operation circuits** instead of one fixed-shape relation verifying
  five inner proofs on every step. This needs the same wrap.
- **Who may make a proof.** A proof is sound whoever makes it. For the
  transitions that are online by nature (MintFold, RedeemSplit, Recertify,
  Migrate), a server could make the proof from the wallet's proof store and
  the signed transition. That would keep an unload open to a phone whose
  prover no longer runs. It shows the server the wallet's state. Not
  designed.

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

### 2.4 The signature-only form

A payment in this form carries the Layer A objects and no proof. It is
described here because it is what a receiver's own checks amount to on the
last hop, and so that what the proof adds can be read off. It is not what the
owner asked for: the owner asked that the proof include that the OS is real,
and this document reads that as covering every hop of the value's history
(§2).

What the receiver checks, from the Payment, its own state and its own clock:

- the payer's certificate and receipt, under issuer and witness keys it
  holds (§5.1);
- the payer's enrollment statement E, as the validators sealed it at
  registration. E travels in full in the payer's certificate, and the
  witnesses' receipt names that certificate (§5.1). The receiver verifies
  the issuer's and the witnesses' signatures. No Payment carries the
  validators' seal (§8.1). E is about 0.14 KB (estimate);
- the device signature on the SendSplit, under the device key in that
  certificate;
- that the SendSplit answers the receiver's own open Request, for the right
  amount and fee (§5.2, §5.8);
- the regulatory controls that are switched on: the lease, the limits and
  its own tally, the block list (§5.4, §5.5).

These are checks L3 to L6 of §2.1, with E taken from the certificate and the
receipt instead of from a proven state. They state the same E about the
paying phone as the proof does. They rest on other keys. In a proof E rests
on the validators' seal. Here the receiver verifies the issuer's and the
witnesses' signatures and not the seal, so whoever holds the certificate key
and the witness quorum can show it a software key with any E (§8.1).

What the form lacks.

- Nothing about earlier hops. The receiver learns nothing about how the
  payer's balance was reached, who held the value before, or whether it
  began as a load. Every earlier holder's phone and app are taken on
  assumption.
- Nothing about the payer's balance. No peer-visible object carries one
  (§5.7). A phone on which an assumption has failed signs a payment for any
  amount with no load behind it. Its chain is self-consistent and no
  evidence need exist (§5.3).
- An arithmetic defect spreads. A wallet whose core adds wrongly signs
  transitions that every receiver accepts. The defect is found only if the
  issuer's replaying core lacks it and the wallet syncs. Until a corrected
  build has replaced the faulty one, value the defect creates moves on like
  any other.
- A stolen voucher key mints value with no load behind it onto genuine
  phones (§8.1).
- A Migrate moves whatever the old journal claims (§7.2).
- No evidence follows from a compromised phone's counters, because it
  chooses them. Two of its payments are sure to be evidence only at the same
  or adjacent sequence numbers (§5.3).

What the form does not change: what keeps a payer from paying twice (the
marker, §5.10); that no payment needs a network; and when a payment is
complete, apart from the proofs. It is smaller and it contains no proving
time. Whether its exchange fits the owner's "1-2 s should be good ux" is not
measured; the evidence gate measures it, because the proof-carrying payment
spends that time as well.

It cannot serve as a first step toward the proof. A pool opened in this form
holds balances that no proof covers. To bring such a pool under the proof
later, every holder would have to sync once so that its balance could be
issued again as a mint that proofs accept. That sends holders online for a
reason that is not a regulatory control, and it certifies whatever the pool
holds at that moment. The other ways are that receivers keep accepting
payments without a proof, so that the proof is not in force, or a second
pool, which is no longer one design.

Describing this form removes nothing. The Pasta implementation stays, and any
removal of recursion code or of its consensus coupling is a separately
approved change (§11).

## 3. What holds, under which assumptions

This section states what the design claims. Three statements give the position.

- Under assumptions T1 to T6 (§4), properties P1 to P5 and PC hold by the
  mechanisms named below. Every mechanism rests on phone behaviour that was
  read from source code or documentation and has not been tested on a device.
  The evidence gate (§10.4) is where that evidence is gathered. Until it is,
  each claim below is a claim about the design and not about any phone. The
  claims also need a prover that completes on every supported phone. None
  exists yet (§2.2). Two mechanisms the claims rest on are not designed: the
  key-store entry from which a wallet proves again after it lost its files,
  and a proof that a wallet which has not updated its app can still verify
  after a new circuit release (§3.1, group B, item 6).
- A receiver cannot check the payer's phone beyond what the payment carries:
  the certificate, the receipt, the enrollment statement and the proof. They
  show how each paying phone was enrolled. They do not show that a paying
  phone's operating system is uncompromised now, and they do not show that a
  payer signed only one successor of its state (§4).
- No backstop, reserve, insurance or promise by any party is part of the
  argument for any property. What follows when an assumption fails is residual
  risk (§3.2). Who carries it is not decided in this document.

Terms. A is the payer, B the receiver, and C whoever B pays next. A tuple is
one phone model, one operating-system major version, one vendor build family
and one key-store security level; the evidence gate gives each tuple one
finding: unsupported, supported with a stated assumption, or supported (§2.3).
The gate's test groups are named by letter: a, restore and rollback paths; b,
crashes and power cuts at every step; c, key-store writes across power loss; d,
what the key store returns when locked or failing, and settings changes; e, the
Android one-use key; f, the iPhone assertion counter; g, time; h, attestation;
i, onward spending with no network; j, later misconduct by the payer. None has
been run.

**P1. B owns the transferred value durably and can spend it onward offline.**

- Claim. Once B's wallet reports a payment complete, the amount is in B's
  spendable balance. B can pay it onward with no network and no further step.
  It is still there after a restart, a power cut, a locked phone, an app
  update, an operating-system update, and the loss, damage or restore of the
  wallet's files. Nothing stops B spending it except a regulatory control
  switched on for B's own certificate (P5).
- Mechanism. B makes the proof that its next payment needs before it commits,
  so nothing remains to be proven (§5.2, §9). The ReceiveFold and the
  `Credited` Outcome are one commit. The marker for that commit carries B's new
  state, and it is confirmed before B's wallet shows the credit (§5.10). A
  wallet whose files are older, missing or damaged resumes at the state in its
  marker, on the phone, with no network (§5.10). A read that fails puts the
  wallet in the waiting state and never counts as a loss (§1, the wallet rule).
  No rule suspends a wallet except those listed under P5; §3.1 lists the rules
  the design does not have.
- Checked by. B's wallet core, from its own marker and journal. The next
  receiver C, from B's certificate, receipt, enrollment statement and device
  signature and from the proof. C checks nothing about how B came by the value
  beyond what the proof states.
- Assumptions. T1, T2, T3 and T6 on B's phone; T4; T5.
- Evidence missing. A forced power-off after each step of a receive, and after
  "complete" is shown (groups b and c). Every restore, rollback and transfer
  path of each vendor (group a). Survival of the key and the marker across
  updates and settings changes (group d); the first Android test is removal of
  the screen lock on Android 12 to 14. Removal, reset and change of the iPhone
  passcode. Onward payment at once with every radio off (group i). For the
  proof: that proving completes on every supported tuple, and that a wallet
  which lost its files can still prove; neither can be tested, because no
  prover exists (§2.2).

**P2. A cannot spend that same value again, under the stated security
assumptions.**

- Claim. Once A's wallet has released a Payment, no state in which that amount
  is still A's can sign again. A regains the amount only by folding B's signed
  refusal of that Payment, once.
- Mechanism. A's wallet signs the SendSplit in memory and makes its proof. It
  creates the marker for the new commit, which carries the state after the
  debit and the signed SendSplit, and confirms it. It commits to the journal.
  It deletes the previous marker and confirms that it is absent. Only then does
  it release the Payment (§5.2, §5.10). At release the key store holds one
  state, the debited one. Only the certified app can use the key (T2 a, b), the
  key cannot leave the phone (T1), and the released app signs one successor per
  state (T3). A restore of older files changes no key-store entry (T2 c), so
  the wallet resumes at the debited state. A SendSplit names B's device id and
  B's Request, so the same Payment is worth nothing to a second receiver. After
  a resume the once-only guard of §5.10 keeps a wallet from folding the same
  inbound credit twice: a Payment, a `Refused` Outcome, a voucher (by voucher
  number) or a countersigned Migrate (by a flag in the marker).
- Checked by. A's own wallet core, from its marker and journal. B checks, from
  the Payment and its own state: A's certificate, receipt and enrollment
  statement under keys B holds; A's device signature; that the SendSplit names
  B's own open Request; and the proof. B cannot check A's marker. B cannot
  check that A signed no other successor of the same state. B cannot check that
  A's app and operating system are genuine now.
- Assumptions. T1, T2 and T3 on A's phone; T4; T5.
- Evidence missing. The rollback script on each tuple and with each vendor tool
  (group a). The crash table of §5.10 on real phones (group b). Key-store
  durability under a forced restart and a true power cut (group c); the first
  iPhone test is the sequence pay, force restart, delete the app, reinstall
  (§3.1). The contents of an enrollment chain, and of a leaf signed by an app
  attestation key, per vendor (group h). Nothing has been run.

P2 is the paying phone policing itself. Under T1 to T3 that is enough. The gate
can show that each route tested, on each operating-system release tested, does
not break T2 c to k. It cannot show that a phone will not be taken over. From
reading, no tuple is expected to reach the finding "supported", which would
mean that P2 holds against a compromised operating system. The best finding
expected is "supported with a stated assumption", and the assumption is T1 to
T3 on that phone (§2.3).

**P3. B's payment does not depend on later reconciliation, approval, or
settlement.**

- Claim. B's balance, and B's ability to pay it onward, depend on nothing that
  happens after B's wallet reports complete: no sync by A or B, no upload, no
  issuer approval, no ledger settlement, no proof made later.
- Mechanism. B makes every check, and its proof, before its commit, from the
  Payment and B's own state. C's checks of B's next payment use B's
  certificate, receipt, enrollment statement, signature and proof. The proof
  names no party that has to be asked. Inside a proof, a key, a policy entry or
  a circuit release that was valid when a hop was proven stays accepted (§2.1).
  A renewal is refused only for an enabled regulatory control, or because B's
  own device key signed two successors (§7.1).
- Checked by. B's wallet and C's wallet, as under P1. At an unload, the ledger,
  from B's registry row and B's signature on the RedeemSplit (§8.2).
- Assumptions. T2 and T3 on B's phone, so that B's wallet follows its own
  rules. T4, so that the issuer service and the ledger follow theirs at a
  renewal and at an unload.
- Evidence missing. A complete exchange with no network on either phone, and no
  traffic afterwards; value moved over many hops while no phone contacts the
  issuer (group i). A test that walks one credit through onward payment,
  renewal and unload while its payer is blocked, held, revoked and never synced
  (group j, and a ledger test).

**P4. Later discovery of misconduct by A cannot invalidate B's accepted
value.**

- Claim. No party reduces, refuses, holds or delays B's value because of
  anything learned later about A.
- Mechanism. It is the absence of a rule. B's wallet never removes a committed
  credit. No object B holds or sends names A in a field that anyone judges
  later. A block entry acts on the device id whose certificate is presented in
  the payment being judged. A hold acts on the row of the device key that
  signed two successors, or on the row that took over that key's balance by
  Migrate, and on no value that row paid to others (§5.3, §8.2). The ledger
  looks only at the redeeming row. A proof that verified keeps verifying,
  whatever is learned later about a key in its history (§2.1).
- Checked by. Nobody during a payment. A reviewer checks the wallet core, the
  issuer service and the ledger instructions for any rule that reads a payer's
  later status. Negative tests show it (group j).
- Assumptions. None about any phone. T4: the issuer service and the ledger must
  run the published rules. No holder can stop an issuer that refuses a renewal
  out of rule.
- Evidence missing. None on a device beyond group j. Negative tests in the core
  and the ledger: evidence against A, a block of A, a hold on A and a
  revocation of the key that certified A each leave B's balance, B's payments
  and B's unload unchanged.

P4 has no condition. §3.2 says what that implies when an assumption has failed
on A's phone.

**P5. Only explicitly enabled regulatory controls may require connectivity.**

- Claim. A working wallet is stopped until it goes online only by the rules
  below, and by each only where the scheme switched the control on.
  - The lease (R8).
  - The reboot policy `require_anchor` (R8).
  - Receive freshness, `receive_not_after` (R6).
  - A block entry on the holder's own device (R6). It is lifted by a renewal
    after the ledger unblocks the account.
  - A tier-row notice that brings in or shortens a lease (R8 with new values,
    §5.11).
  - A clock that R7 or R8 can no longer use, where the wallet's floor is ahead
    of real time (§5.4).
  - After a resume with R6 on: the block list version the marker names, where
    the list is too long to arrive from a peer (§5.10).
  - An object that carries a critical extension the wallet's app does not
    know: no payment is made with that counterparty until the app is
    updated. §5.11 allows a critical extension only for a regulatory control
    the scheme has switched on.

  With R6, R7 and R8 off, no rule requires connectivity at any time.
- Mechanism. The design has no other rule that suspends a wallet until a sync;
  §3.1 lists the rules it does not have. Three stops need no network. A wallet
  in the waiting state waits for the phone's own storage. A tier may refuse to
  pay until a screen lock is set, and the holder ends that on the phone (§5.9).
  A stopped wallet, whose key or marker is gone, is not helped by a network at
  all (§3.1).
- Checked by. The wallet's own core. Every state in which it refuses to sign is
  one of these: waiting; stopped; or a control that the certificate, a
  root-signed tier-row notice or the block list carries.
- Assumptions. T2 and T3 on the phone in question. T4, so that notices and list
  entries are signed only as §5.5 and §5.11 allow.
- Evidence missing. A wallet with a `Never`, `Unlimited` certificate and no
  receive freshness lives through reboots, clock changes, long idle periods,
  updates, a key rotation, a revocation, a new rules version and a closure with
  no network, and still pays and is paid. With one control on, only that
  control stops it, and the wallet names it (group i).

Enroll, load, unload, renew and Migrate need the network by nature. Each is the
holder's choice. None is needed to keep paying while no control is on.

**PC. Any processing needed to establish those properties must finish before
the wallet reports the transfer as complete.**

- Claim. B's wallet reports a transfer complete only when nothing more is
  needed for P1 to P4. A's wallet reports complete only when it has stored B's
  `Credited` Outcome.
- Mechanism. B: its checks, then the proof its own onward spending needs, then
  the durable commit of the ReceiveFold and the `Credited` Outcome, then the
  confirmed marker step, then "complete". A: its signature and the proof of its
  state after the SendSplit, then the marker, the commit and the deletion of
  the previous marker, then release. A's wallet shows "sent, not confirmed"
  until it stores a `Credited` Outcome. It shows "returned" after it commits a
  RefundFold for a `Refused` Outcome. No wallet commits a transition it has not
  proven (§5.2, §9). What P2 needs from A is finished before the Payment is
  released, so before B can report anything.
- Checked by. Each wallet's core. The core returns "complete" from one place
  only, after the marker step is confirmed.
- Assumptions. T2 and T3 on the reporting phone. For B's "complete" to include
  P2, also T1 to T3 on A's phone.
- Evidence missing. Whatever a wallet reported complete is still there after a
  kill or a power cut at every later point (group b). The time of everything
  inside the interval, on each carrier and each pair of phones (group g).

What PC does to time. Both the payer's proof and the receiver's proof fall
inside the payment. The repository's gate for one proof is 10 s (§2.3). No
prover exists that can make a State or payment proof (§2.2), so nothing is
measured. The owner's "1-2 s should be good ux" is not met by a proof-carrying
payment unless each proof takes a fraction of a second on the slowest supported
phone. No measurement supports that, and the one recorded figure for the
stock-phone circuit points the other way (§2.2). §2.3 lists shapes that could
shorten the wait as choices to evaluate. None is designed and none is checked.
The exchange without a proof is not measured either.

**Conservation.** If T1 to T5 hold for every enrolled phone and every key,
every unit in every wallet came from a load, and no unit is in two wallets.
Claims presented to the ledger then never exceed loads, and pool cash covers
every claim without outside funding. This follows from P2, T3 and three rules:
the once-only guard of §5.10; a voucher issued only for a load that is final on
the ledger (T4); and recovery insurance paid from its own account and never
from the pool (§7.3). No loss rule and no backstop is part of the argument.

**Which phone each property needs the assumptions on.**

| Property | Needs |
|---|---|
| P1 | T1, T2, T3 and T6 on B's phone; T4; T5 |
| P2 | T1, T2 and T3 on A's phone; T4; T5 |
| P3 | T2 and T3 on B's phone; T4 |
| P4 | Nothing on any phone; T4 |
| P5 | T2 and T3 on the phone in question; T4 |
| PC | T2 and T3 on the reporting phone. For B's "complete" to include P2: T1 to T3 on A's phone |

B relies on T1 to T3 holding on A's phone and can verify none of them during
the payment. What B verifies is the enrollment statement: what the validators
accepted about A's phone at its enrollment or last renewal (§4). B's own
properties do not depend on any earlier phone in the history of the value: that
is P4. Conservation does.

### 3.1 Rules removed, and where the criterion is still not met

**Rules the design does not have.** Each rule below would break a property
while every assumption holds. The design has none of them. The last column says
what it has in their place.

| A rule the design does not have | What it would break | What the design has instead |
|---|---|---|
| A wallet commits a transition, or shows a credit, before the proof that transition needs exists | PC, P1 | No wallet commits a transition it has not proven. There is no state for received value that waits for a proof, no rule that a wallet cannot pay again until a background proof finishes, and no redemption path for value a phone could not prove. A receiver that cannot make its proof answers `Refused` (§5.2, §9) |
| A refusal must itself be a proven transition | An honest payer stays debited when the receiver cannot prove | A refusal is a signed `Refused` Outcome, committed with the marker step (§5.2) |
| A revoked issuer key stops every phone under it from paying or requesting until it syncs | P5, P1 | A revocation never stops a wallet paying or requesting. A certificate under a revoked key stands until its own expiry, judged after the stricter-of rule of §5.11. Inside a proof, what was valid when a hop was proven stays accepted (§5.1, §5.11, §2.1). §3.2 says what this means for a certificate that never expires |
| A rules-version floor that takes effect offline | P5, P1 | No floor acts offline. Every release accepts every rules version the scheme ever allowed, and two wallets use the highest they share (§5.11). Whether an app-build floor is checked at a renewal is in §3.2 |
| A wallet whose files are older, missing or damaged stops | P1 | The marker carries the wallet's current state. The wallet resumes at it, on the phone, offline (§5.10) |
| A stopped wallet is repaired online | P5 | No repair is needed to pay again. The issuer accepts a resume record at a sync, a renewal and a Migrate, including from a wallet that made no transition since its last sync (§5.10, §7.2) |
| A credit is shown when the commit returns, before the marker step | PC | The wallet reports complete only after the marker step is confirmed. The payer's wallet shows "sent, not confirmed" until it stores a `Credited` Outcome, and "returned" after a RefundFold commit (§5.2) |
| A signed object is written to a file before the key store holds it | P2 | The signed object goes into the marker first. A copy of the files never holds a signed object that the key store does not hold (§5.10) |
| A device key bound to user authentication | P1: a settings change destroys the key and the balance | The device key carries no authentication requirement. The app shows the platform prompt (§5.9) |
| Recovery insurance paid from the pool | Conservation while every assumption holds: an ordinary user who claims and keeps spending leaves the pool short for honest holders | Where an operator enables it, it is paid from a separate account funded in advance and never from the pool. It is off by default and is no part of the argument for any property (§7.3) |
| Retiring a device id by its account key blocks the phone offline | P1 | Retirement has no offline effect, unless the scheme switches on blocking at the holder's request as part of R6 (§7.2, §5.5) |
| A hold placed on the issuer's judgment, on evidence of an issuer-key fault, or accepted as the price of a platform fault | P1, P3, P4 | A hold needs two conflicting signatures by one device key, verified on-chain. It acts on that key's row, or on the row that took over its balance by Migrate. Evidence of an issuer-key fault places no hold on the phone that holds the object. A tuple that fails the forced power-off tests is unsupported (§5.3, §8.2, §2.3) |
| A renewal refused for a reason of the issuer's choosing | P3, P4 | A renewal is refused only for an enabled regulatory control, or because the device's own key, or the key whose balance its row took over by Migrate, signed two successors (§7.1) |
| A Migrate that leaves the old wallet's open Requests unanswered | An honest payer's refund for a payment that did not complete | A Migrate carries only the Requests the old wallet can show are undecided. The successor answers `Refused` for those only (§7.2) |
| A fixed release rate for unloads above a row's own loads | It slows an honest net receiver while every assumption holds. Whether P1 and P3 reach redemption is the owner's reading | `unload_limit` is a scheme parameter. Its value, "no limit" included, is the owner's choice (§3.2, §8.2) |
| A rule that names who pays for value with no load behind it | It is not a property. It puts a promise where the criterion asks for a mechanism | No section names a funder. §8.4 gives the ledger's mechanics for a shortfall. §3.2 lists who could supply one |

**Where the criterion is still not met.** The design does not meet the
criterion in the places below. Listing them does not meet it either. For each,
the text says what happens, why no rule found removes it on a stock phone, and
what would be needed to meet the criterion there. The owner decides what
follows; §3.2 lists the choices and takes none.

Group A. Not met on stock phones by any rule found.

1. **P2 against a compromised phone.** If T1, T2 a, T2 b or T3 fails on A's
   phone, A can pay the same value to B and to someone else, and both payments
   pass every check, the proof included. The secure hardware signs what the
   operating system asks and keeps no record of what it signed. Every attested
   fact about the operating system is fixed at boot or supplied by the running
   system, so a phone taken over after boot produces the same evidence as an
   honest one (§4). What would be needed: secure hardware that allows one
   signature per state even when its own operating system asks for two, with
   each hop showing its use; or hardware that runs wallet logic. No target
   phone is shown to have the first; groups e and f test for it (§4.1). The
   owner has set the second aside for now. Until one exists, P2 holds only
   under T1 to T3, and the proof does not change that.
2. **P1 when the device key is destroyed.** The phone is lost, stolen, broken,
   erased or factory reset. On Android the app is uninstalled without keeping
   its data, or its storage is cleared. On iPhone the phone is erased and
   restored. The value is held by one key in one phone, and what destroys the
   key destroys the value. Only a signature by that key can show that the
   balance was not spent, so any way to give the balance back without the key
   is a way to spend it twice. What would be needed: a second holder of the
   wallet's state that the first cannot roll back. That is another party, which
   is online, or hardware that runs wallet logic. Capped insurance from a
   separate account (§7.3) is not a transfer of that value and does not meet
   P1. The design narrows the cases: the wallet warns before the first load,
   Android asks at uninstall whether to keep the app's data, and a
   clear-storage request opens the wallet's own screen (§5.10; not tested). It
   does not remove them. This document states the case as T6. Whether P1 may
   rest on T6 is the owner's to say (group C, item 2).
3. **P1 on iPhone after the passcode is removed or reset.** Apple's platform
   security guide says that items of the marker's keychain class become useless
   when the passcode is removed or reset, and the API page says that disabling
   the passcode deletes them. That class is the only one Apple documents as
   never backed up. A copy of the marker in a class that survives passcode
   removal comes back when a backup is restored onto the same phone, so it
   would let an ordinary user restore an older state together with a working
   key. With the marker gone the wallet cannot tell whether its files are its
   latest, and it stays stopped. The balance is out of reach. What would be
   needed: an iPhone store that is outside every backup and survives passcode
   removal. None is documented. One candidate was examined: a second anchor,
   the App Attest assertion counter used inside the wallet. It would let a
   wallet resume from intact files once a passcode is set again, if the counter
   steps by exactly one, cannot be set back by any path an ordinary user has,
   and survives passcode removal. None of the three is documented or tested,
   and Apple engineers have said they intend to change what a restore does to
   App Attest keys. It is not switched on. The gate's passcode tests and group
   f record the facts. Until one of the two exists, P1 on iPhone holds only
   while a passcode stays set. Enrollment requires a passcode, and the wallet
   says before the first load that removing or resetting it puts the balance
   out of reach (§5.9).
4. **A payment that never completed.** The payer is debited at its commit. If
   the Payment never reaches the receiver, or a `Refused` Outcome never reaches
   the payer, the amount is in neither wallet until the two phones meet again.
   If they never do, it is lost. The criterion speaks of a completed payment,
   so as worded it is not broken. An honest payer can still lose value. No rule
   removes it: two phones cannot exchange atomically without a third party,
   debiting first is the order in which an interruption cannot create value,
   and a refund on a timeout would let any two holders create value by not
   showing the Outcome. A resume can make the loss permanent for the other
   side. A receiver that lost its files keeps, in its marker, its open Requests
   and its last four decisions; for any other Request it signs no answer
   (§5.10). A payer that lost its files can present again only the newest two
   of its unresolved SendSplits. What would be needed: a third party that both
   phones reach. An issuer relay when both happen to sync is possible; it is
   online, optional, and no property depends on it. Larger caps reduce the
   second case at the price of a larger key-store write for every signed
   object.
5. **PC against the timing wish.** A proof-carrying payment that meets PC has
   both proofs inside the interval the owner called good at "1-2 s" (§3, PC).
   What would be needed: proofs that each take a fraction of a second on every
   supported phone, or a payment shape in which less is proven while the two
   people wait and PC still holds. §2.3 lists candidate shapes. None is
   designed.

Group B. Not shown. A mechanism exists and the evidence does not, except in
item 6, where two mechanisms are not yet designed.

1. **P2 against an ordinary user on iPhone: key-store durability.** The
   sequence needs no tool: pay, force a restart within seconds, delete the app
   before opening it, install it again, pay again. If the keychain lost the two
   writes of the marker step in the restart, the wallet resumes at the state
   before the payment. Apple's published keychain source opens its database in
   write-ahead-log mode and sets no synchronous or full-sync option (`SecDb.c`,
   `SecItemServer.c`; read 2026-10-02). Apple's SQLite build, read on macOS,
   defaults that mode to a setting which SQLite's documentation says may lose a
   committed transaction on power loss. The iOS build was not read and nothing
   was tested, but the sources point toward failure and not merely to an open
   question. This sequence is the first iPhone test of the gate, at several
   delays after release. The candidate barrier is a wait before release, with a
   sync call; it is charged to the time budget and is not tested. Until the
   test passes, P2 against an ordinary user is not shown on any iPhone. A
   failed test makes every iPhone tuple unsupported under the gate's rule
   (§2.3). If no barrier works, the choice is between this exposure and a
   wallet that stops when its files are absent, which gives up P1 for an honest
   holder who reinstalls.
2. **P1 and P2 on every Android vendor build.** No vendor documents what its
   backup, clone or transfer tool does to Keystore entries. A tool that puts
   older entries back breaks P2 for every user of that build. A tool that
   clears the entries before it restores destroys the balance. AOSP's own
   restore engine clears an app that has no backup agent before a full restore;
   the wallet therefore declares an agent that saves and restores nothing
   (§5.10; not tested on any build). Group a decides per tuple.
3. **P1 on Android 12 to 14 after the screen lock is removed.** On those
   releases the key-store service deletes every certificate-only entry when the
   screen lock is removed, and keeps a key that has no authentication
   requirement (AOSP `keystore2`, release branches android12 to android14;
   read, not run). §5.10 therefore stores the marker there in a form that
   survives: an entry with a key blob and no authentication requirement, or the
   alias form. A gate test decides which. Vendor builds were not read.
4. **P1 on iPhone after a whole-phone backup is restored without an erase.**
   Whether the restore leaves the marker in place, and whether the Secure
   Enclave key still signs, is not documented. If the marker is removed while
   the key lives, the wallet stops. If the key dies, the balance is gone.
5. **P1 and P5 when the terms entry is lost with the files.** The marker names
   a second key-store entry that holds the certificate, the receipt and the
   verified issuer keys (§5.10). If that entry is missing or damaged and the
   files hold no matching copy, the wallet has its balance and cannot build a
   payment until it syncs. Under T2 e and i this does not happen.
6. **The proof.** No prover exists that can make a State or payment proof on
   any machine (§2.2), so no property that depends on proving is shown. Four
   points are open beyond feasibility.
   - A phone on which the prover does not finish cannot pay and cannot receive.
     Its balance cannot leave by an unload either: a RedeemSplit is a
     transition, and a wallet commits it only with its proof (§5.2). §8.2 and
     §2.3 name ways out, and none is designed. Qualification must show that
     proving completes on every supported tuple, every time; a count of rare
     failures is not enough (§2.3, Q2).
   - A wallet that lost its files has its balance in the marker. It can pay
     offline only if the latest proof and the private witness needed to extend
     it are in the key store as well. Their size has no bound until the
     relation is fixed (§5.10; §2.3, Q0).
   - A new circuit release has to be verifiable by a wallet that has not
     updated its app. Otherwise that wallet cannot be paid value that passed
     through a newer wallet until it reaches an app store. Not designed (§2.3,
     Q0).
   - The server-made enrollment proof needs P-384, RSA and SHA-384 arithmetic
     that the repository does not have (§4).
7. **Phones with no path to evidence.** HarmonyOS NEXT has no wallet design
   (§10.2), and its key attestation carries no boot, lock or patch field. No
   primary information was found for Meizu, and no chain has been captured from
   any Samsung, Xiaomi, Honor, Huawei or Meizu phone. On iPhone no test
   produces evidence about the operating system, because the attestation does
   not carry it. Two of the phones the owner named, Huawei and Meizu, can enter
   no claim today.
8. **Findings go stale.** A finding holds for one tuple and one wallet build.
   An enrolled phone updates itself, offline, to a release nobody tested. The
   issuer learns of it at a sync, and on iPhone only from what the app reports.
   With no lease it never learns. Google's update commitment for the Pixel 6
   ends in October 2026 (§10.3).

Group C. Met or not, depending on a reading that is the owner's.

1. **Assumptions outside P2.** The criterion puts "under the stated security
   assumptions" on P2 only. This document claims P1, P3, P5 and PC under T1 to
   T5 on the holder's own phone and on the issuer side, and P1 also under T6.
   If those properties must hold with no assumption, the design does not meet
   them: an honest holder whose phone is taken over by someone else's malware
   can lose the balance (§3.2).
2. **"Durably" and T6.** If durable means that the value outlives the phone,
   item 2 of group A stands against P1 and the design does not meet it.
3. **Redemption.** If P1 and P3 govern how fast the ledger pays a redemption,
   any release rate on unloads breaks them for an honest net receiver. If they
   do not, the rate is a scheme parameter (§3.2).
4. **P5 after an issuer key is stolen.** If P5 holds whatever happens to issuer
   keys, a certificate that never expires stays acceptable without limit of
   time after a theft. If P5 holds only under T4, a deadline to sync after a
   theft is allowed (§3.2).
5. **What counts as an explicitly enabled regulatory control.** The six
   additions of §1, and what a renewal under R8 may check: a fresh attestation,
   a patch floor, an app-build floor.
6. **A hold on the successor of a Migrate.** The hold falls on the row that
   took over the balance of the key that signed twice. If the old phone was
   sold or repaired before its key was deleted, and was then taken over, the
   holder's new phone is held although its own key signed nothing wrong. It is
   the same holder, so P4 as worded is not engaged. P1 fails for that holder
   until a reinstatement (§3.2).

### 3.2 Residual risk when an assumption fails

Nothing in this section is an argument for P1 to P5. The bounds and the choices
below apply only after an assumption has failed on some phone or key. They
supplement the protocol. They do not establish that value moved without
duplication.

**What becomes possible, and which properties then fail.**

| Assumption that fails | Who can break it | What becomes possible | Which properties fail, and for whom | What bounds it |
|---|---|---|---|---|
| T1 on a payer's phone: the key is copied out, or a software key is attested with a leaked or extracted attestation key | Someone with an exploit of the TEE or Secure Enclave firmware, or with an attestation key that is not yet revoked. The payer has the motive. An ordinary user cannot | The key signs any number of successors of one state, on any machine. Each passes every check, the proof included | P2 for every payment that key signs. Value exists with no load behind it, so conservation fails. P1, P3 and P4 still hold for each honest receiver | R6, R7 and R8 as described below. A policy that refuses factory-provisioned attestation roots removes leaked factory keys. Revocation of the attestation key, applied at the next renewal where R8 is on |
| T2 a or b on a payer's phone: the operating system is taken over after boot | Someone with a privilege-escalation exploit for the release on the phone. That is the payer on the payer's own phone | The same, with the key still in the hardware: copy the files and the key-store entries, pay, put them back, pay again. Fresh attested leaves and assertions read as honest | As above | As above. With R8 on, the lease is the age limit of the Android evidence about the operating system, and a patch floor at renewal narrows the exploits that work. Neither says anything about an iPhone's operating system. Evidence where two records meet |
| T2 a or b on an honest holder's phone, by someone else's malware | The author of the malware | The malware pays the balance away. Or it makes the holder's key sign two successors and pays the attacker's wallets from both | P1 for that holder. If the key was forked, value is created, and where the two records meet the hold falls on the honest holder's row | As the row above |
| T2 on a phone that was migrated from: the old key signs above its Migrate | Whoever controls the old phone, if it left the holder's hands before its key was deleted and was then taken over | Two successors by the old key | P1 for the honest holder: the hold falls on the new row, whose key signed nothing wrong | The old wallet deletes its key once the retirement is final (§7.2). Without the succession rule a phone that forked would escape a hold by migrating before the evidence arrives |
| T2 c to k on a tuple: a vendor tool copies key-store entries, a confirmed write is lost, a failed update rolls back, a marker is removed | Nobody has to break anything. The tuple behaves this way or it does not | Every ordinary user of that tuple can restore an older balance. An honest holder can lose a committed payment, sign twice after a power cut, or find the wallet stopped | P2 for payments from that tuple, at scale and with no skill. P1 for an honest holder of that tuple. A hold may follow | The gate: the tuple is unsupported and is not enrolled. After launch, when an update changes the behaviour: on Android the issuer refuses the tuple at enrollment, from the attested release. At renewal it is refused only where R8 is on and the owner has made a list of covered releases part of what R8 checks (§7.1); otherwise nothing reaches an enrolled phone. On iPhone the attestation names no model and no release |
| T3: a defect in the released core, or a build under the operator's signing key that is not a released wallet app | The operator's staff, or someone who steals the app-signing key. A defect needs nobody | Every phone on that build may create value, or accept what it should refuse | P2 and conservation, possibly for every phone at once | The proof keeps an arithmetic defect on the phone: a wallet cannot prove a wrong transition. It does not contain a defect in the circuit, or in the marker order. No floor acts offline, so a defective rules version stays acceptable offline; it ends at renewal only if an app-build floor is part of R8, and never under a `Never` certificate |
| T4, certificate key with the witness quorum | Operator staff, or an outsider who reaches the keys | The certificate key alone makes nothing a peer accepts, because no receipt names its certificates (§5.1). With the witness quorum, for a registered device whose holder cooperates: a certificate that evades a refused renewal, restarts the limit counters, outlives a lease, or carries a serial above a block entry. Under the proof-carrying design no new device, because the validators seal the enrollment statement | R6, R7 and R8 weaken for those devices. Under the proof-carrying design these keys create no value | The tier row caps the terms. After the revocation reaches a receiver: one lease, where R8 is on. §5.5 says why a block entry is not widened to every serial under a revoked key, and how far R6 then holds |
| T4, the keys that seal the enrollment statement and sign receipts, with the certificate key (§8.1) | As above, for more keys | Software keys that every receiver and every proof accepts | P2 and conservation, without bound until the revocation spreads. Where R8 is off, without limit of time under the first reading of P5 below | R8. The server-made enrollment proof, once it exists, also demands a vendor chain that verifies (§4) |
| T4, voucher key | As above | Value minted onto genuine phones with no load. Whether a proof accepts it depends on the mint authorization fixed in Q0 (§2.3) | Conservation | A mint that stays sealed by the validator quorum inside the relation |
| T4, list key | As above | Forged block entries against honest devices | P1 for those devices among holders of the forged list, until a root-signed epoch bump reaches those holders from a peer or at a sync. No value is created or lost | The epoch bump travels peer to peer (§5.5) |
| T4, root key | As above | Notices and issuer key certificates that peers accept offline. Nothing revokes the root; peers stop accepting it for later epochs when they hold the succession to the committed next key, which the thief cannot forge (§5.11) | Whatever the keys it certifies allow: the four rows above | The succession. What a root compromise voids is not decided (§12) |
| T4, the issuer service or the ledger does not follow its rules | The operator; a fault in the ledger | A renewal refused out of rule, a voucher for a load that is not final, a registry or a pool that does not follow §8 | P3 and P5 for the holder who is refused. Conservation | Nothing in this design. An unload needs the ledger and not the issuer (§8.2) |
| T5 | Nobody is known to be able to | Everything | All | None |
| T6 | The holder, or a thief | The holder's own balance is destroyed or out of reach | P1 for that holder. Nobody else is affected and no value is created | A Migrate or an unload before a planned erase. Recovery insurance from its own funded account, where an operator offers it; it is not a return of that value |

Scale differs by row. One phone taken over is one payer. An exploit that works
on every phone of a model, a vendor tool, a core defect and a stolen issuer key
are each a failure for many payers at once.

**The bounds, and what each is worth.** Each is applied by a named party from
an input that party can check.

- R7. Each honest receiver accepts at most one limit per window from one payer
  device id, and at most two in one real day or month across a window boundary.
  The receiver applies it from the limit in the payer's certificate and its own
  tally (§5.4). The tally restarts when the receiver's files are lost or put
  back (§5.10). Nothing bounds the number of receivers a compromised phone
  reaches, so the total is not bounded.
- R8, the lease. A certificate that is not renewed ends at its lease. The
  receiver applies it from the certificate and its own clock, so it holds among
  receivers whose clocks are right (§5.4). The lease is also the age limit of
  the evidence about the operating system: an Android enrollment statement is
  no older than one lease.
- R8, the renewal. On Android a renewal presents a fresh attested leaf (§7.1):
  the boot state and the patch levels of that day. The vendor certificate
  serials are tested against the revocation list of that day. A patch floor
  applied then shuts out phones with holes that are public and patched. It does
  not shut out a hole that is not public, or one between its publication and
  the floor. A phone past its vendor's update commitment falls below a rising
  floor for good. On iPhone a renewal shows nothing about the operating system.
- R6. A block entry stops a device at each receiver that holds the entry. A
  receiver gets the list at a sync or from a peer.
- Evidence. Under the relation of §2.1 a phone with one enrolled instance and
  no accomplice that pays out more than its proven balance must sign two
  successors of one state. The two receivers then hold a pair that is evidence,
  if both records reach the issuer or the chain (§5.3). That argument has not
  been checked by a second reading or by a test. No holder has to sync, so the
  pair may never meet. It does not exist at all where one branch is paid to a
  second enrolled instance or to a receiver who colludes. Evidence leads to a
  block entry and a hold. It undoes nothing.
- Two requirements on Android vendors that would bound a compromised phone and
  cannot be tested here. The KeyMint interface requires a key to stop working
  when the bootloader is unlocked or the verified-boot key changes, and when
  the phone is rolled back to an older patch level. If a phone meets them, a
  signature by the enrolled key implies a boot of the kind seen at enrollment.
  On a production phone an unlock erases the phone before the binding can be
  observed. They are not assumptions of this design. They do nothing against an
  exploit of the running system.
- With R6, R7 and R8 off, only `max_payment` remains, applied to each payment.
  Nothing bounds the total and nothing reveals it.

What bounds nothing. A release rate on unloads slows how fast cash leaves the
pool; it bounds no total. The proof does not prevent a second branch. Cash paid
into the pool in advance pays for value that was created; it limits none of it.

**What P4 implies.** P4 carries no condition. When T1, T2 or T3 fails on A's
phone and A pays B and B' from one state, B and B' each hold accepted value. P4
says neither can be invalidated when the fork is found. P1 says each can pay it
onward offline, and no later receiver can tell it from loaded value. P3 says
neither can be made to wait for a settlement that would sort them out.

So the extra value stays good in every honest wallet it reaches. It shows as a
gap only on the ledger: redemptions exceed loads by that amount, less whatever
is never presented. Someone supplies the difference, or some claim goes unpaid.
The protocol cannot place the loss on the phone that created the value. It can
stop that phone later, with a block entry and a hold, and it can name the
account bound to it.

The criterion therefore excludes two answers: paying only the first receiver to
sync, and voiding value traced to a payer later found at fault. It does not say
who supplies the difference. §8.4 describes what the ledger does when the pool
is short. It names no funder.

**Choices for the owner.** None is taken here, and no order is implied.

1. **Whether P2 must hold against a compromised operating system.** If it must,
   only the finding "supported" meets the criterion. No tuple is expected to
   reach it, and the options are then hardware that runs wallet logic, or no
   production release of offline value on stock phones. If it need not,
   "supported with a stated assumption" meets P2 as written, and the choices
   below remain.
2. **Which phones may hold value.** Every phone that passes enrollment. Or only
   tuples the gate supports, with terms per tuple; on Android the issuer can
   enforce a list from attested fields, and on iPhone it cannot (§2.3). Or no
   stock phone above a small amount until hardware that runs wallet logic is
   available. Two open points of §3.1 bear on iPhones in particular: the
   passcode, and key-store durability.
3. **Which platform classes a pool admits when it claims the operating-system
   constraint.** iPhones; phones whose attestation keys were provisioned in the
   factory; HarmonyOS NEXT. A pool's claim is that of the weakest class it
   admits (§4).
4. **Which controls are on by default.** With limits and expiry off, P5 is met
   in its widest form, and after a failure nothing bounds the amount and
   nothing reveals it. With them on, the bound is per receiver and per lease,
   and holders go online once per lease.
5. **Who supplies a difference between redemptions and loads.**
   - The operator, from cash paid into the pool in advance and a duty to add
     more.
   - The asset's issuer, by issuing new units. The cost then falls on every
     holder of the asset through dilution.
   - A reserve funded by a fee on loads or payments.
   - A third party under a guarantee or an insurance contract.
   - Nobody. The pool then pays in the order claims arrive and later claims
     wait, and some may never be paid. If the criterion reaches redemption,
     that fails P1 or P4 for whichever honest holder is last. Whether it
     does is the owner's reading (§1; §3.1, group C, item 3).
6. **The release rate on unloads above a row's own loads** (§8.2). With no
   limit, an honest net receiver is paid at once, and a row controlled by a
   compromised phone draws cash as fast as it claims until it is held. With a
   limit, that row draws one limit per window, and an honest merchant whose
   takings exceed the limit waits longer every window while every assumption
   holds.
7. **Certificates that never expire, after an issuer key is stolen.** Under the
   first reading, P5 holds whatever happens to issuer keys: there is no forced
   sync, and what the thief signed for such a tier stays acceptable without
   limit of time. Under the second, P5 holds under T4: after a theft the root
   may set a period within which holders of such certificates sync once. The
   rule sections describe the first reading (§5.1, §5.11).
8. **An app-build floor at renewal.** If it is part of R8, a renewal is refused
   to an app below the floor, and a build with a defect ends within one lease.
   If it is not, a renewal is never refused for the build, and a defective
   build lives as long as its holders keep it. Under either answer a renewed
   wallet keeps accepting every rules version the scheme ever allowed.
9. **The patch floor at renewal.** Its value, and whether a phone past its
   vendor's update commitment stays under it.
10. **What a held phone's holder gets back, and who may order it,** where the
    two signatures came from a fault of a supported tuple, from someone else's
    malware, or from a phone the holder had migrated from.
11. **Whether to pursue recovery from the account** bound to a device that
    forked, where accounts are tied to an identity. That is outside the
    protocol.
12. **What follows if the proof cannot be made in an acceptable time** on the
    supported phones (§2.3).

## 4. Trust model

The design claims P1 to P5 and PC under the six assumptions below and under no
others. An assumption is something the protocol relies on and that no party
verifies during a payment. Where a party can verify part of one, the text names
the party and the input it uses.

Terms.

- The operator is the party that runs the issuer service (§6) and that signs
  and publishes the wallet app. It holds the app-signing key, which is not
  among the keys of §6.
- The released wallet app is a build of the wallet that the operator signed and
  published.
- A compromised phone is a phone on which T1, T2 a, T2 b or T3 does not hold:
  its key is out, its operating system has been taken over, or its app is not
  the released app.
- The holder is the person who has the phone.

Neither holder is assumed honest. A payer may use everything an uncompromised
phone offers: backup and restore, reinstall, phone-clone tools, developer
options, the settings app, the clock, a forced restart.

**T1. Secure hardware.** The secure hardware of the phone keeps the device key
so that it cannot be copied out, and signs with it only when that phone's
operating system asks. On Android the secure hardware is the KeyMint
implementation in the TEE or in StrongBox. On iPhone it is the Secure Enclave.
On Android it also states, in a key attestation, the boot state, the lock state
and the patch levels as the boot loader and the booted system gave them to it.
The keys that sign the vendor's attestation of that phone have not leaked or
been extracted, or they are in the revocation set the validators used.

- What breaks it. An exploit of the TEE or Secure Enclave firmware. A leaked
  factory attestation key, used to attest a key held in software.
- Who can. Someone who has a firmware exploit or a leaked attestation key. The
  payer is the party with the motive. An ordinary user cannot. Both have
  happened: published work extracted hardware-protected keys from Samsung
  Galaxy S8 to S21 phones (Shakevsky, Ronen and Wool, USENIX Security 2022),
  and Google's revocation list held 1,733 entries for compromised keys on
  2026-09-29 (one fetch).
- Evidence the gate gives (group h). A captured attestation chain from a
  production unit of each tuple, classed as factory or remotely provisioned,
  with every field recorded. The negative cases: an unlocked bootloader, a
  user-installed boot key, an emulator, a re-signed app and an imported key
  must each be rejected from a field of the chain. A leaf signed by an app
  attestation key, per vendor: whether it carries the root of trust and the
  patch levels, its size, the time to make it, and whether it can be made with
  no network. No test shows that secure hardware has no exploit or that no
  attestation key has leaked.
- iPhone. Apple certifies the App Attest key only. The payment key is a second
  Secure Enclave key that the app creates, and Apple offers no way to attest
  it. Its public key is hashed into the value the attestation names. That shows
  that code under the App ID, holding the attested key, named those bytes. It
  does not show that the bytes are a Secure Enclave key, that the key is on the
  same device, or that the caller holds it. On iPhone T1 for the payment key
  therefore rests on T2 and T3 at enrollment: the released app creates the key
  in the Secure Enclave. The existing verifier requires the two keys to differ.
- What a receiver can check offline. For the payer: the certificate, the
  receipt and the enrollment statement, under keys the receiver holds. For
  every earlier payer: the proof. Both show that the validators accepted a
  vendor chain for the key, with the facts the enrollment statement lists. The
  receiver sees no vendor chain and checks no fresh fact about the hardware.

**T2. Operating system.** A phone that booted locked and verified runs the
vendor's operating system, and nobody has taken that system over since. The
design relies on the behaviours below and on no others.

- a. App separation. No other app and no tool can use the device key, or read,
  change or selectively delete one of the wallet's key-store entries. The
  wallet's files are not covered: restore tools may read and replace them, and
  §5.10 is built for that.
- b. App identity. Android: an installed app can be replaced only by a build
  signed with the same signing key, and the identity in a key attestation is
  that of the installed app. iPhone: only an app signed for the App ID can use
  its keychain items and its App Attest key.
- c. No copy of a key-store entry. Android Keystore entries, and iPhone
  keychain items of the class
  `kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly`, are in no backup, in no
  transfer to another phone and in no output of a vendor backup or clone tool.
  No tool sets one back to an earlier value.
- d. Removal with the app. Whatever removes the wallet's key-store entries
  removes all of them together with the device key: on Android an uninstall
  that does not keep data, clear storage and a factory reset; on either
  platform an erase. None leaves an older entry behind.
- e. Durable key-store writes. A creation or a deletion that the key store
  reported done, and that a read confirmed, survives a power cut and a forced
  restart. The key store never keeps a later write while losing an earlier one.
- f. Durable file writes. A journal transaction that returned as durable (§5.2)
  survives a power cut.
- g. Distinct answers. The wallet can tell "absent" from "cannot be read now"
  by the calls §5.10 names.
- h. Survival. The device key and the marker survive a reboot, an
  operating-system update and an app update. On Android they also survive a
  change or removal of the screen lock: the device key because it is generated
  without an authentication requirement (§5.9), the marker because §5.10
  chooses, for each Android release, a form of entry that survives it. On
  iPhone the marker needs a passcode (T6).
- i. No selective removal. No platform or vendor path removes a marker, or
  another key-store entry of the wallet, while the device key
  stays usable. The one known path, removing or resetting the iPhone
  passcode, is excluded by T6 and not by this item.
- j. No rollback by a failed update. A failed operating-system update does not
  put the key store or the wallet's files back to an earlier state after the
  app has run.
- k. The key store answers again. A key-store call that fails succeeds after an
  unlock or a restart.

Items a and b are about an attacker. Items c to k are about how a phone model
behaves.

- What breaks it. A root exploit or a jailbreak breaks a and b. A vendor backup
  or clone tool that copies key-store entries breaks c. A key store that loses
  a confirmed write breaks e. Storage that ignores a sync request breaks f.
- Who can. For a and b: someone with a privilege-escalation exploit for the
  release on the phone. That is the payer on the payer's own phone, or the
  author of malware on someone else's. Such exploits exist for locked, patched
  phones: Android security bulletins of May 2022 and September 2025 list kernel
  flaws that may be under limited, targeted exploitation (CVE-2022-0847,
  CVE-2025-38352), and a public tool minted App Attest objects on a jailbroken
  iPad mini 4 (third-party; one old model). For c to k nobody has to break
  anything: a tuple behaves this way or it does not. If it does not, every
  ordinary user of that tuple can reset a wallet (c, e), or an honest holder
  can lose a payment, sign twice or lose the wallet (e, f, h, i, j, k).
- Evidence the gate gives. For c, d and i: every restore, rollback, clone and
  transfer path, per vendor tool (group a). For e and f: forced restarts and
  true power cuts after each step (groups b and c). For g, h and k: the read
  mapping in every lock state, settings changes and updates, and an endurance
  run (groups c and d). For j: an update between payments, with the point at
  which the system commits it (group a). A result holds for the routes tested,
  on the release tested. For a and b no test can show that a phone will not be
  exploited. The evidence is what the enrollment statement records, and the
  vendor's update record.
- What the sources say today. For e, the AOSP key-store service syncs each
  transaction before it returns; vendor builds were not read. Apple's published
  keychain source sets no sync option, so e is open on iPhone and the reading
  is unfavourable (§3.1).
- What a receiver can check offline. Android: the enrollment statement of each
  paying phone, as sealed or proven. It says how that phone booted, at which
  patch levels, and which app the system named, at enrollment or at the last
  renewal. It says nothing about now. iPhone: nothing about the operating
  system. For c to k: nothing. A receiver cannot tell which tuple a payer's
  phone is; the issuer keeps unsupported tuples out at enrollment, as far as
  the attestation names the model and the release.

**T3. Released wallet app.** The app installed under the certified identity is
a released wallet app, and the released wallet app follows §5. It signs one
successor per state. It computes balances and counters correctly. It makes its
proof, its commit and its marker step in the order §5.2 and §5.10 give.

- What breaks it. A defect in the released core. A build signed with the
  operator's app-signing key that is not a released wallet app: an insider's
  build, a build made with a stolen signing key, or a test build enrolled
  against the production scheme.
- Who can. The operator's staff, or someone who steals the app-signing key. A
  defect needs nobody: every phone on that build has it.
- Evidence. The gate gives none directly: its app is a test wallet, and its
  crash and restore groups run again on the wallet core when that exists
  (§2.3). Other evidence: the crash-table tests of the core in test mode (§9).
  Conformance vectors. A finite-state model of the commit and marker rules. An
  independent review. Reproducible builds and a custody record for the
  app-signing key. The app identity checked at enrollment: on Android the
  package name and signing-certificate digest as the system reported them; on
  iPhone the App ID hash and, from iOS 27, the launch category and bundle
  version. None of this exists yet.
- What a receiver can check offline. The proof shows that the arithmetic of
  every transition in the history is valid, so an arithmetic defect cannot pass
  a receiver. Nothing shows which code asked for a signature, or that the
  payer's app kept the marker order. The rules version in a SendSplit is the
  payer's own statement.

**T4. Issuer-side keys, and the ledger's finality and rules.** The scheme root
key, the certificate key, the keys that seal enrollment statements and sign
receipts, the voucher key and the list key are used only by their owners and
only as §6 says: a certificate only for a key whose attestation passed, a seal
only for a vendor chain that verified, a voucher only for a load that is final
on the ledger, a list entry only as §5.5 allows. The issuer service follows §6
and §7: it numbers vouchers per device id, and it refuses a renewal only for
the reasons §7.1 allows. The ledger is final and runs the rules of §6 and §8 as
written.

- What breaks it. Theft of a key, misuse by an insider, a faulty issuer
  service, a fault in the ledger.
- Who can. Operator staff, or an outsider who reaches a key.
- Evidence. The gate gives none. The evidence is operator-side: hardware
  custody, the quorum arrangement, audit records. The registry lets anyone
  compare registered device ids with certificates they have seen.
- What a receiver can check offline. The signature chain from the root key it
  holds down to the certificate, the receipt and the enrollment statement, and
  the revocations it holds. It cannot tell a signature made with a stolen key
  from a genuine one before a revocation reaches it. It cannot read the ledger.

**T5. Cryptography.** ECDSA on P-256 and the hash functions are unbroken, and
the random numbers of the secure hardware cannot be predicted. The proof system
is sound, and its circuits constrain what §2.1 says they constrain.

- What breaks it. A new attack on the curve or a hash. Secure hardware whose
  random numbers repeat. A soundness defect in the proof system or in a
  circuit.
- Who can. Nobody is known to be able to break the standard algorithms. A
  circuit defect needs nobody.
- Evidence. The gate gives none, and none is specific to a phone. For the
  circuits it is the soundness stage Q3 (§2.3). None exists yet.
- What a receiver can check offline. Nothing.

**T6. The holder keeps the wallet.** The holder keeps the phone, the app with
its data, and on iPhone a passcode that is neither removed nor reset. In
detail: the holder does not lose, break, erase or factory reset the phone. On
Android the holder does not uninstall the app without keeping its data and
does not clear its storage. A restore from a backup that needs no erase is not
part of this condition: whether it destroys the device key or the marker on a
tuple is a finding of the gate (group a).

T6 is a condition on the holder, not a statement about an attacker. Only P1
needs it. It is listed because P1 says "durably", and the value is held by one
key in one phone.

- What breaks it. Loss, theft, damage, a factory reset, an erase and restore.
  On Android an uninstall that does not keep data, or clear storage. On iPhone
  removing or resetting the passcode.
- Who can. The holder, a device administrator, or a thief. A thief who can
  unlock the phone can also pay the balance away (§5.9).
- Evidence the gate gives. What each such action does to the device key, the
  marker and the files on each tuple: the "destroyed" results of group a,
  the settings changes of group d, and the passcode cases. Developers report
  that a Secure Enclave key no longer signs after an erase and restore. Whether
  it signs after a restore without an erase has no source. Not tested.
- What a receiver can check offline. Nothing, and nothing is needed. T6
  concerns the receiver's own phone after the payment.

§3.1 says which parts of T6 the design narrows and which it cannot remove.
Whether P1 may rest on a condition on the holder is the owner's to say.

**Not assumed.**

- That either holder is honest.
- That any clock is right. Only limits, expiry and receive freshness use time,
  and each is a regulatory control. Each is therefore only as good as the
  clocks of the two phones (§5.4).
- That a network is reachable during a payment.
- That a receiver can check the payer's marker, or that the payer's app and
  operating system are genuine now.
- A counter or a one-use key that the secure hardware enforces against its own
  operating system. No target phone is shown to have one (§4.1).
- That an Android key stops working after an unlock or a rollback. The KeyMint
  interface requires it; it cannot be observed on a production phone. §3.2
  counts it as an untested bound.
- That every enrolled phone is uncompromised. §3 makes its claims for the
  phones it names. §3.2 says what happens otherwise.
- That any party will supply a shortfall.

**What each party can check.**

- A receiver, offline, from the Payment and its own state: the payer's
  certificate, receipt and enrollment statement under keys it holds; the
  certificate against the tier row; the device signature; that the Payment
  answers its own Request; expiry and limits on its own time; its own list; its
  own tally; the proof. It cannot check the payer's marker, whether the payer
  signed another successor of the same state, or whether the payer's app and
  operating system are genuine now.
- The validators, at a registration, from the raw vendor chain: its signatures
  up to a pinned root, the revocation list of that day, and the facts of the
  enrollment statement against the policy entry (§8.1). §7.1 says who verifies
  the fresh leaf at a renewal. Nobody checks anything about the phone between
  those two moments.
- The issuer, at a sync, from the uploaded journal: that it extends the
  acknowledged head and replays; or that it is a resume record whose last
  transition is the acknowledged head itself, or carries the device key's
  signature and a higher sequence number, with cumulative totals not below
  those at that head (§5.10). It cannot check that no other branch exists, and
  it cannot replay inside a gap.
- The ledger, at an unload, from the RedeemSplit and the registry row: the
  device signature and the increase in the cumulative total, with whatever the
  relation fixed in Q0 adds (§8.2). It verifies no balance.

**What the proof adds to the receiver's knowledge of the payer's phone.** The
owner asked that the proof include that the operating system is real (§1). The
design does it as follows.

- The enrollment statement, E. One exists for each device key. It holds: the
  attested key; the platform class; the security level; for Android the
  root-of-trust fields (locked, verified, boot key digest), the
  operating-system, vendor and boot patch levels, and the app identity as the
  operating system reported it; for iPhone the App Attest facts (a genuine
  Apple Secure Enclave key, this scheme's App ID, the production environment,
  and from iOS 27 the launch category and bundle version); the policy entry
  used; a commitment to the vendor certificate serials, for revocation checks;
  and the enrollment epoch.
- E is established twice over. The validators verify the raw vendor chain
  natively at registration and seal the registry (§8.1). That anchors the time
  and the revocation check, and it needs no new arithmetic. And a server-made
  enrollment proof of the vendor chain, and of the constraints on its fields,
  is the starting point of the phone's recursive proof (§2.1). The repository
  has no P-384, RSA or SHA-384 gadget, so the enrollment proof is a
  qualification item. It is made on a server and never on a phone. Until it
  exists the recursion consumes the statement the quorum sealed.
- Per hop, the relation verifies one P-256 device signature under the key in
  the proven predecessor state, the sequence, link and counter rules, and that
  the state carries a valid E under a policy entry (§2.1). No vendor chain is
  checked on a phone, and no platform evidence is made on the payment path.
- A renewal refreshes E, where R8 is on. On Android it carries a fresh
  hardware-attested leaf under the app attestation key that the wallet created
  at enrollment. The leaf shows the boot state and the patch levels of that
  day, and the policy's patch floor is applied then. That the leaf comes from
  the hardware that holds the device key is an inference from how key blobs are
  bound, not a documented guarantee. On iPhone a renewal carries an assertion
  by the enrolled App Attest key, which shows nothing about the operating
  system.
- What the receiver checks itself. For the payer, natively: the certificate,
  the receipt and E as the validators sealed it. For every earlier paying
  phone: the proof. A payment with signatures only would carry the first and
  say nothing about earlier hops (§2). A receiver applies no patch floor of its
  own to a payer: that would send a payer online for a reason that is not an
  enabled regulatory control.

The statement "the proof shows that the operating system is real" has a true
reading and a false one.

- **The true reading.** For every paying phone in the history of the value, the
  proof shows this and no more: the phone's secure hardware was told at boot
  that a vendor-signed image was verified on a locked bootloader; it was told
  patch levels at or above the floor; and the operating system named the
  genuine wallet package as the app that asked for the key. All three are as of
  that phone's enrollment or its last renewal.
- **The false reading.** The proof does not show that the running operating
  system of a paying phone was genuine and uncompromised when a payment was
  signed. Every attested field is fixed at boot or supplied by the running
  system. A locked, verified, fully patched phone that is taken over after boot
  produces the same evidence as an honest one, at enrollment, at renewal and at
  every hop. Both branches of a double spend made on such a phone satisfy every
  clause of the relation.
- On iPhone no operating-system, patch, boot or jailbreak field exists. The
  statement there is "a genuine Apple Secure Enclave acting for this App ID".
  Apple's own documentation says that an attacker who modifies the operating
  system may get past the restrictions App Attest relies on.
- A HarmonyOS NEXT key attestation carries no boot, lock or patch field.
- Three things stay unproven and are assumptions: no run-time compromise of the
  paying phone's operating system (T2 a, b); no leaked or extracted attestation
  key outside the revocation set (T1); no broken secure hardware (T1).

What the constraint excludes from the history behind a payment, and what no
receiver could exclude without it: emulators and software keys; unlocked or
re-keyed phones; repackaged apps on a healthy operating system; keys certified
by a stolen issuer key; and leaked factory attestation keys, where policy
refuses factory roots. With it, a payer who wants to pay twice needs one of
three things: a working exploit against a locked phone at or above the patch
floor, an attestation key that is not yet revoked, or broken secure hardware.
The constraint excludes none of the three.

A pool's claim is that of the weakest platform class it admits, because value
moves between phones. Whether iPhones, phones with factory-provisioned
attestation keys, and HarmonyOS NEXT are admitted to a pool that claims the
operating-system constraint is the owner's decision (§3.2). Refusing factory
roots would exclude the one chain recorded in the repository, a Pixel 6
StrongBox chain under Google's first root
(`specs/kagemusha_v1_production_readiness.md:416-421`).

Optional, and not in the base design: on Android a fresh attested leaf per boot
or per payment; on iPhone an App Attest assertion on every hop. Neither shows a
run-time compromise. Both cost bytes and time, and on iPhone a lost assertion
or a key that Apple invalidates strands a balance.

State of the code. No circuit in the repository checks anything about the
operating system today. Boot state, lock state and app identity are checked
only by the issuer's verifier
(`python/iroha_app_attestation/src/iroha_app_attestation/attestation.py:974-1061`),
and with the default patch floor of zero the patch levels are not checked
(`.../attested_enrollment.py:178-217`). They reach a proof as a platform class
tag, a fixed guarantee mask and a digest that no circuit opens
(`crates/iroha_core_zk/src/kagemusha_v1_recursion/guard_bundle.rs:261-315,
867-897`). The circuits that would accept an App Attest or KeyMint key are
built for tests only (`.../composite.rs:10-28`), and the verifier refuses both
classes (`.../guard_verifier.rs:87-103`). §2.2 has the rest.

### 4.1 The counter or commitment

The owner's mechanism is hardware-backed keys and "some unique
counter/commitment to prevent reset and double spend" (§1). A study on
2026-10-02 looked for every such primitive that an ordinary store app can reach
without OEM access, and had each one attacked. Nothing was run on a device.

Two attackers have to be kept apart.

- **U, an ordinary user with the unmodified app** on an unrooted phone. U uses
  what the platform offers: backup and restore, reinstall, phone-clone tools,
  developer options, the settings app, the clock, a forced restart. "Reset"
  means bringing back an earlier balance while the key still signs.
- **M, a modified app or a compromised operating system** on a phone that
  passed attestation at enrollment. M can ask the hardware key to sign
  anything, any number of times.

**Against U: the marker.** The commitment is the marker of §5.10. It is an
entry in the phone's key store that no backup carries, and it holds the
wallet's current state. An object is released only after the marker for the
state that includes it is confirmed and every earlier marker is deleted. Files
that are older, missing or damaged then change nothing: the wallet resumes at
the state in the marker. Under T2 a, c, e and i, U has no path back to an
earlier balance. The rules, the crash table and the once-only guard are in
§5.10.

| Platform | Marker | Why an older one cannot come back | What is open |
|---|---|---|---|
| Android 15 and later | As on Android 12 to 14 unless the gate shows otherwise: the key-with-certificate entry of §5.10. A certificate-only entry, written in one database transaction with no key generation, may replace it on a tuple where the gate shows that it survives screen-lock removal (§5.10) | Keystore entries are in no backup. An uninstall that does not keep data, clear storage and a factory reset delete them together with the device key (AOSP source) | No device tested. No vendor build read. A read can fail for reasons other than absence, so absence is taken only from a complete listing (§5.10) |
| Android 12 to 14 | An entry that survives removal of the screen lock: an entry with a KeyMint blob and no authentication requirement, or the state carried in a key's alias. A gate test decides | The same | On these releases removing the screen lock deletes every certificate-only entry (AOSP `keystore2` `database.rs`, release branches android12 to android14; read, not run) |
| Android, vendor builds | As above | Not documented by any vendor | Xiaomi and Honor document restore of third-party app data onto the same phone, and Meizu a local backup of app data. Huawei documents it too and excludes what it calls financial application data; how an app is classed is not documented. No vendor says what its tool does to Keystore entries |
| iPhone | A keychain item of class `kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly`, which needs a passcode. Its data is the state | Apple documents that items of this class are not backed up and never move to another device | Removing or resetting the passcode discards the item, and the balance is out of reach (§3.1). A confirmed keychain write may be lost in a forced restart; the pay, force restart, delete the app, reinstall sequence is the first iPhone test of the gate, and a wait before release is the candidate barrier (§3.1). The class is readable only while the phone is unlocked, so a read that fails is not a missing marker. What a same-phone restore does to an existing item is not documented |
| HarmonyOS NEXT | Not designed (§10.2) | | Backup and clone are opt-in per app. Its key store documents no use limit and no counter |

The results that bear on trust are these.

- Without a marker, a long-lived key can be reset by an ordinary user. On stock
  Android, developer options allow a package rollback that restores an app's
  data and leaves Keystore alone (source reading; not run). The vendor tools in
  the table restore app data. On iPhone, Apple documents that device-only
  keychain items other than the passcode class return when a backup is restored
  to the same phone. The Secure Enclave key is kept as such an item; developers
  report that such a key is gone or no longer signs after an erase and restore,
  and this is untested. One desktop tool's guide says it restores one app's
  files without touching the keychain.
- The marker holds only if the key store keeps a write it confirmed (T2 e). If
  it does not, one path remains: pay, force the power off at once, and delete
  or replace the files before the wallet starts. On iPhone deleting the app is
  enough. Reading the sources, the Android reference does not have that window
  and the iPhone may (§3.1).
- The marker is the paying app policing itself. A receiver cannot check it. It
  does nothing against M.

**Against M, nothing was found that prevents a double spend.**

- The secure hardware is a signer with no memory of what it signed. M copies
  the wallet's files and key-store entries, pays one receiver, puts the copy
  back and pays another. The second receiver sees exactly what an honest
  payment looks like, so any rule that accepts honest payments accepts this
  one. No cryptographic construction changes that. Signatures that reveal the
  key when used twice need the key outside the hardware, where M already holds
  it. A bond deters only where the gain is bounded, and here it is not (§3.2).
- Attestation does not help. Every attested fact is fixed at boot or supplied
  by the running system, and M obtains a fresh, conforming attested leaf or
  assertion for each branch (§4).
- Every published design found that prevents double spending of value the
  receiver can spend again offline runs wallet code in a secure element or a
  trusted application. Every design found that keeps such value transferable
  without that hardware detects or scores risk afterwards. The claim is limited
  to transferable value because one cited scheme falls outside it. PulpoPay
  (ePrint 2026/2199) claims to prevent double spending for an offline receiver
  and names no secure hardware. As one automated pass read the full text, the
  payer, while online, has each coin issued to a named receiver's key, and that
  receiver cannot spend the coin again offline. Only the abstract was read a
  second time. R1 and R4 exclude that scheme, because the payer must be online
  and the value does not move on.
- **iPhone assertion counter.** Apple documents it as the number of assertions
  a key has signed and asks servers to check that it grows. Apple does not say
  where it is kept or that it steps by one. One third-party source, for one old
  iPad model, shows the assertion data, counter included, being put together by
  an operating-system service, with the enclave signing what it is given. If
  that holds on current iPhones, M sets any counter it likes. Not tested on a
  current device. The repository's one record shows the values 0, 1 and 2 on
  one iPhone, and an assertion the app discarded still used a count
  (`specs/kagemusha_v1_production_readiness.md:327-333`). The existing code
  accepts any counter above a retained floor
  (`crates/iroha_data_model/src/kagemusha/kagemusha_v1/hardware_selection.rs:489`,
  `IrohaSwift/Sources/IrohaSwift/KagemushaAppAttestEvidenceV1.swift:342-347`);
  with gaps allowed, a counter shows neither a fork nor a rollback.
- **Android one-use keys.** The interface, the reference code and the one
  measurement do not settle whether any phone enforces a limit of one in
  hardware.
  - The KeyMint interface says a use limit above one is enforced in software. A
    limit of one is enforced by the secure hardware only if that implementation
    can do so with its secure storage; the attestation then lists the limit at
    the hardware's own security level.
  - AOSP publishes two reference implementations, and they differ. The Rust
    reference (`platform/system/keymint`, `common/src/tag.rs` at commit
    `fda4e68d`) enforces a limit of one itself when secure storage is available
    and leaves it to software otherwise. That branch has no StrongBox
    exclusion. The JavaCard applet that AOSP publishes for StrongBox
    (`platform/external/libese`, `ready_se/google/keymint/KM300`,
    `KMKeyParameters.java`) lists the use limit among the tags left to
    software. AOSP's Trusty reference for the TEE enforces a limit of one where
    it has secure deletion storage. All three are source readings. None says
    what a given phone does.
  - The one measurement is the repository's. On a Pixel 6, a StrongBox key with
    a limit of one was attested with the limit in the software list
    (`specs/kagemusha_v1_production_readiness.md:342-379`). That is one phone
    and one security level. No TEE key limited to one use has been generated on
    any phone: the repository's probe returns on the feature flag before it
    generates the key
    (`kotlin/client-android/src/main/java/org/hyperledger/iroha/sdk/offline/probe/AndroidKeyMintSingleUseProbeV1.kt:79-81`).
    No StrongBox other than the Pixel 6's has been tried.
  - Even where the hardware enforces the limit, reading the reference code
    shows two ways a compromised operating system gets more than one signature
    from such a key: it can begin two operations before finishing either, and
    after a security-patch update it can obtain a second usable copy of the
    key. Whether a vendor's firmware allows either is not tested. Unless a test
    shows otherwise, the most a chain of one-use keys gives is two conflicting
    hardware signatures. That is evidence, not prevention.
- **Tencent SOTER**, on phones sold in mainland China, signs with a counter
  kept in the TEE. The counter is shared by every app on the phone, so it has
  gaps and cannot show that a state had one successor.

**Items for the evidence gate.** Two candidates could overturn the finding, and
the gate tests both on spare units (§2.3).

- Group e, the Android one-use key, on the TEE and on StrongBox, per vendor:
  where the attestation lists the limit, and whether two begun operations, a
  key upgrade or a key blob put back yield a second signature.
- Group f, the iPhone assertion counter: whether it steps by exactly one,
  whether it persists, and whether code with operating-system privilege can
  produce two assertions with one counter value. The test on current hardware
  needs access the project may not get.

From reading, neither is expected to pass. If one does, a receiver could check
that a state had one successor only in a design this document does not contain:
every hop would have to show its one-use key or its counter value, in the
payment or inside the proof. On iPhone that is an assertion on every hop, which
is not in the base design (§4). The counter is also the candidate second anchor
for the passcode case of §3.1; that use is inside the wallet, gives nothing
against M, and is not switched on.

**What "prevent reset and double spend" can mean on stock phones** is therefore
this. An ordinary user cannot reset a wallet or spend twice with the unmodified
app and platform tools, under T2, and only on a tuple for which the gate has
shown T2 c to k. The gate has run on no tuple, and on iPhone the reading of the
sources is unfavourable. A compromised phone can do both. Prevention against it
needs a counter or a one-use key that the secure hardware enforces against its
own operating system, with each hop showing its use, or hardware that runs
wallet logic. No target phone is shown to have the first, and the owner has set
the second aside for now.

## 5. Protocol

Canonical Norito. Every signed preimage is `tag ‖ scheme id ‖ bare payload`,
fixed-width scalars, explicit presence bytes for optional fields. All wire
objects are new. Sizes are estimates from field lists; none is measured.


### 5.1 Objects

Sizes in this section are estimates. None is measured, and no encoder exists
for any object here. They are computed from the field lists below under the
encoding rules of the existing attested suite. The table at the end says who
signs each object and where it travels. The marker step of §5.10 applies to
every object in that table that a wallet signs and commits.

- **Device id** = `H(tag ‖ scheme id ‖ device public key)`. Every verifier and
  the registry recompute it from the certified key. The issuer never enrolls
  one device key twice: renewal raises the serial under the same device id,
  and a retired id is never re-enrolled.
- **Scheme descriptor**: the root signature over the on-chain scheme cell's
  (epoch, digest). Chain, asset, scale, pool id, role-separated keys, the tier
  table, scheme-wide ceilings, admission policy. It also holds:
  - the root public key and `next_root_digest`, the hash of the root key that
    will follow it (§5.11);
  - `rules_max`, the highest rules version a wallet may sign under (§5.11).
    There is no lowest version: every version from the first to `rules_max`
    stays allowed;
  - the policy table (below);
  - `list_epoch` (§5.5), and whether holder-requested blocking is switched on
    (§5.5);
  - the fee policy records and their beneficiary accounts (§5.8);
  - how long a Request is open for a new payment (§5.2);
  - the recovery parameters (§7.3);
  - the scheme status: open, or closed to loads (§5.11).

  A wallet receives the whole descriptor from the issuer at enrollment and at
  each sync. The descriptor is not sent peer to peer.
- **Tier row**: one row of the tier table. It holds the highest terms a
  certificate of that tier may carry, one value for each term of the device
  certificate below; the fee policy (§5.8); the user-authentication mode
  (§5.9); and the ledger parameters of §8.2 and §5.8.
- **Policy entry**: one entry of the policy table. The table says what
  evidence about a phone's hardware, operating system and app the scheme
  accepts. It is append-only: an entry is never changed or removed, and each
  entry holds the digest of the one before it. The newest entry number a
  wallet holds is its policy head. An entry holds:
  - its number, the digest of the previous entry, and `effective_from_ms`;
  - the platform classes it admits. The classes are: Android under a remotely
    provisioned attestation chain, Android under a factory-provisioned chain,
    iPhone, and a class whose attestation carries no statement about the
    operating system. Which classes a scheme admits is the owner's decision
    (§2.1, §12);
  - for the Android classes: the vendor root keys accepted; the lowest
    security level; that the bootloader is locked and the boot state is
    verified; the lowest OS, vendor and boot patch levels (the patch floor);
    the wallet's package name and signing-certificate digest; the lowest
    attestation version;
  - for iPhone: the Apple App Attest root; this scheme's App ID; the
    production environment; and, from iOS 27, the launch category and the
    lowest bundle version;
  - the root of the set of vendor attestation certificates that were revoked
    when the entry was made;
  - optionally a lowest app build for each platform. Whether a scheme uses
    this field is an owner decision (§5.11);
  - under the proof, the digests of the circuit releases that are accepted
    (§2.1, "Policy inputs"). How a release is named and listed is not
    designed; Q0 fixes it (§2.3).

  The root signs each entry. Ledger governance installs it (§6). It reaches a
  wallet in the descriptor and, peer to peer, as a notice. About 0.3 to
  0.5 KB per entry; rough, because no layout exists.
- **Enrollment statement (E)**: what the evidence about one device key showed
  when it was verified. It has one form per platform.
  - Both platforms: the platform class; the number of the policy entry the
    evidence was judged under; a commitment to the serial numbers of the
    vendor certificates in the evidence; the enrollment epoch, which is the
    ledger height at which the validators verified the evidence, at
    registration or at the latest renewal that refreshed it.
  - Android: the security level (TEE or StrongBox); the root-of-trust fields
    as the secure hardware reported them (bootloader locked, boot state
    verified, the digest of the verified-boot key); the OS, vendor and boot
    patch levels; the app identity as the operating system reported it (a
    digest over the package name and the signing-certificate digest, and the
    version code).
  - iPhone: the identifier of the App Attest key Apple certified; this
    scheme's App ID; the production environment; from iOS 27 the launch
    category and the bundle version. Apple's attestation carries no OS
    version, patch level, boot state or jailbreak state, so E has none.

  The attested key itself is the device key in the certificate that carries E.
  E is a fixed function of the evidence, the policy entry and the height, so
  the issuer, the validators and a server-made enrollment proof arrive at the
  same bytes. `e_digest = H(tag ‖ scheme id ‖ device public key ‖ E)`. On
  Android the digest also covers the public key of the app attestation key.
  The certificate carries that key beside E, and §2.1, §7.1 and §8.1 count
  it as a field of E: a renewal's leaf is checked under it, so a proof of
  the renewal relation needs it bound in the proven state. About 0.14 KB on
  Android and about 0.13 KB on iPhone.

  E states what the evidence showed on the day it was verified. It says
  nothing about the phone since then (§4). §2.1 says how a proof uses E. §8.1
  says how the validators verify the evidence. §5.11 says what a renewal
  refreshes.

  The serial commitment ties E to the vendor certificates behind it. The
  issuer keeps the serial numbers and tests them against the vendor's
  revocation list at each renewal (§5.11). A proof of the enrollment relation
  shows them absent from the revoked set its policy entry names (§2.1). No
  receiver makes that test offline. The vendor's list was about 180 KB when
  read on 2026-10-02, and a refusal by receivers would stop honest phones
  that share a revoked factory key, for a reason that is not an enabled
  control.
- **Root-signed notices**: parts of the descriptor that are signed one by one
  so that they can be handed peer to peer when one side is behind. Each
  carries the scheme id, the descriptor epoch at which it was made, its kind,
  its body and the root signature. About 0.11 KB plus the body. Kinds:
  - issuer key certificate: a certificate, voucher, list or witness key, its
    role and its key id; for a list key also the `list_epoch` it signs for;
  - key revocation: the key id, its role, `revoked_at_ms` and `never_stand`
    (below);
  - tier row: one row, with `effective_from_ms` (§5.11);
  - rules version: `rules_max` (§5.11). It only rises;
  - policy entry: one entry of the policy table;
  - root succession: the next root key, signed by the old root key and by the
    new one (§5.11);
  - scheme status (§5.11).

  A tier-row, rules-version or scheme-status notice with a newer epoch
  replaces an older notice of the same kind and subject. Policy entries and
  revocations are kept, all of them. A wallet that holds an older set is
  looser than the scheme until it receives the newer one; §5.11 says how
  notices travel.
- **Key revocation.** The root revokes a key only after a compromise. A
  planned key change uses no revocation (§5.11). `revoked_at_ms` is the time
  from which the root no longer vouches for what the key signs. Four rules
  hold.
  - A revocation never stops a wallet. A wallet whose own certificate or
    receipt was signed under a revoked key keeps paying and requesting under
    what it holds. Adopting the notice changes nothing in the wallet's own
    state. The wallet receives a certificate and a receipt under the new keys
    at its next sync, whenever that is. Where R8 is on, that is the renewal
    the lease already requires.
  - A revocation never touches value. No balance, no payment already received
    and no claim on the ledger changes because a key was revoked.
  - What a revoked key signed stands to its own expiry. This covers a
    certificate signed under a revoked certificate key, and a certificate
    whose receipt has fewer than k signatures under unrevoked witness keys.
    A holder of the notice judges such a certificate by its effective terms
    (below) with one change: the lease is counted from the earlier of
    `not_before` and `revoked_at_ms`. The change is there because whoever
    stole the key can write any `not_before`. An honest certificate was
    issued before the revocation, so the change never shortens it. The same
    counting applies to `receive_not_after`. A receipt under revoked witness
    keys stands for as long as the certificate it names. Checked by the
    receiver, from the root-signed notice, the tier row and its own time; a
    payer applies the same to a receiver's certificate.
  - A block entry is compared by serial under a revoked key as under any
    other (§5.5).

  By stolen key, what the thief could have made that a holder of the notice
  still accepts:
  - The certificate key alone: nothing a peer accepts. A certificate is
    accepted only with a receipt that names it, and a witness signs a receipt
    only for a certificate the ledger has recorded (below).
  - The certificate key and the witness quorum: certificates and receipts for
    keys that no phone holds. A holder of the notice accepts them for at most
    one lease after `revoked_at_ms`. §8.1 lists the other combinations.
  - The voucher key. A wallet folds a voucher only under a voucher key it
    holds unrevoked. A voucher reaches a wallet online, in an exchange in
    which the wallet also receives the notices, so no offline wallet is
    affected. A voucher issued before the revocation and not yet folded is
    issued again under the new key with the same voucher number (§7.1).
  - The list key: §5.5.

  A `Never` certificate has no expiry to stand to. After a theft of the
  certificate key and the witness quorum, a receiver cannot tell an honest
  `Never` certificate from one the thief signed. Either such certificates
  stand without limit of time, or they stand for a root-signed period after
  `revoked_at_ms` and their holders must then sync once. The first has no
  bound on what the thief's objects create. The second is a sync that no
  regulatory control asked for. Which it is depends on how the owner reads
  P5, and §3.2 puts that decision. The notice carries the outcome as
  `never_stand`: no limit, or a duration. Nothing else in §5 depends on the
  value.

  Like every expiry, these bounds are only as good as the receiver's clock
  (§5.4). §2.1 states which keys a proof's history accepts.
- **Device certificate** (about 0.53 KB on Android, about 0.48 KB on iPhone;
  about 70 B more with a fee policy): device key, serial, `descriptor_epoch`,
  `not_before`, send expiry (`Lease` ending at `not_after`, or `Never`),
  optional `receive_not_after`, `max_payment`, day and month limits (`Limit`
  or `Unlimited`), per-counterparty cap (`Cap` or `None`), `opening_day` and
  `opening_month`, `expiry_grace`, `window_future_tolerance`,
  `clock_regress_tolerance`, reboot policy, optional fee policy
  (`fee_policy_id`, rate, fixed part and ceiling; §5.8), the enrollment
  statement E, on Android the public key of the app attestation key where
  the phone has one (§5.11), issuer key id. No account field.
  - `descriptor_epoch` is the epoch of the descriptor the issuer held when it
    signed. It tells a verifier which tier row was in force at issuance.
  - `opening_day` and `opening_month` are amounts already counted against the
    day and the month that contain `not_before`. They exist so that a new
    certificate never gives the same account a second allowance in a window
    (§5.4).
  - §5.4 says what each tolerance is for.
  - E is carried in full, so every Request and Payment shows it. The
    receiver of a Payment checks three things about the payer's E. The
    receipt names this certificate, so E is the statement the validators
    sealed. The policy entry E names is an entry of the table the receiver
    holds; if the receiver lacks it, the payer hands over the notice. E's
    fields satisfy that entry. The receiver applies no later entry to E: a
    later entry acts on a device at its next renewal (§5.11). For the
    receiver the age of E is bounded by the certificate's lease, where the
    tier has one. A payer makes none of these checks on a receiver's E beyond
    the receipt; nothing about its payment depends on them.
  - The app attestation key is an Android key created at enrollment. It has
    one purpose, to certify other keys the app generates in the same key
    store, and the vendor chain certifies it (§7.1). Its public key is in
    the certificate so that the same signatures bind it to the device as bind
    the device key. The issuer and the validators verify a renewal's fresh
    leaf under it (§5.11). No peer uses it in a payment. It costs about 34 B.
- **Effective terms.** A verifier judges a certificate by its effective
  terms: each term is the stricter of the certificate's value and the value
  in the newest root-signed row the verifier holds for the receipt's tier
  (§5.11), with the counting rule above where a revocation applies. Every
  check in §5.2, §5.4 and §5.5 uses effective terms. A verifier refuses a
  certificate outright only when that row has not changed since the
  certificate's `descriptor_epoch` and the certificate exceeds it, because
  then terms were signed that the scheme did not allow.
- **Registration receipt** (about 150 B with one witness, about 67 B per extra
  witness): witness signatures over (scheme id, registered tier, registration
  height, certificate digest). It is complete with k signatures under witness
  keys the verifier knows. Every certificate has its own receipt: a renewal
  returns the new certificate together with the receipt that names it. A
  witness signs only after the ledger write that records that certificate's
  serial and `e_digest` for a live registry row is final on its own node,
  and after it has checked the certificate's device key, serial and E
  against the row. The ledger does not record the certificate's digest: the
  issuer signs the certificate after that write (§7.1). A witness signs one
  receipt per recorded serial (§8.1). The
  validators verify the evidence behind E when that write executes (§8.1). So
  a certificate with its receipt shows a peer, offline, that the registry
  holds this device key, this serial and this E. That rests on the witness
  keys (T4). Peers take the tier from the receipt. Every Request and Payment
  carries certificate and receipt.
- **Registration authorization** (issuer-signed, never sent to peers): scheme
  id, device id, account id, tier, first certificate serial. It names neither
  a certificate digest nor `e_digest`: E holds the height of the block that
  executes the registration, so neither exists before that block (§7.1).
  The registry requires the registering transaction's authority to equal that
  account. The transaction also carries the vendor evidence the validators
  verify (§8.1).
- **Voucher** (voucher-key signed, issuer to wallet only; about 0.31 KB):
  scheme id, voucher id, device id, voucher number, load id, amount, ledger
  transaction hash, `issued_at_ms`, voucher key id. Apart from the voucher
  number the field list follows the existing suite's voucher
  (`IrohaSwift/Sources/IrohaSwift/KagemushaAttested/KagemushaAttestedModels.swift:685-693`).
  The voucher number is the per-device load counter: the first voucher for a
  device id has number 1 and each later one the next number. A wallet folds a
  voucher only if its number is one above the last number folded, which its
  marker holds (§5.10). The rule is there so that a voucher is never folded
  twice, also by a wallet that has lost its files. §7.1 says how a voucher is
  issued and numbered.
- **Transition** (about 315 B; a SendSplit under a fee policy about 380 B):
  rules version, device id, seq, previous digest, kind, amount, `fee`,
  `fee_policy_id`, subject, counterparty, `device_time_ms`, anchor flag,
  clock-reset flag, cumulative `cum_out_after`, cumulative `cum_refund_after`,
  cumulative `cum_fee_after`, gross-sent and refunded counters for the day and
  the month of `device_time_ms`, certificate digest. Kinds: Bootstrap (seq 0
  only), MintFold, SendSplit, ReceiveFold, RedeemSplit, RefundFold, Recertify,
  Migrate, MigrateFold. For RedeemSplit, `amount` is the increment and
  `redeemed_total_after` is a separate field.
  - The rules version is the first field, and its place and width never change
    (§5.11).
  - `fee` is zero where the payer's certificate holds no fee policy.
    `fee_policy_id` is the digest of the fee policy record the fee was
    computed under, which is the one in the payer's certificate. Settlement
    is judged under that record however late the SendSplit arrives (§5.8).
    `cum_fee_after` is gross, like `cum_out_after`.
  - For a SendSplit, the subject is the digest of the Request and the
    counterparty is the receiver's device id.
  - For a MintFold, the subject is the voucher id, and the transition also
    carries the voucher number (9 B). The commit of amount zero that steps
    the voucher number past a void load (§7.1) is a MintFold of amount zero
    whose subject is the void statement, which the voucher key signs (§6).
    The relation of §2.1 has no clause for it. Without one, a wallet under
    the proof-carrying design could fold no later voucher after a void
    load. Q0 fixes it with the mint authorization (§2.3).
  - For a Migrate, the counterparty is the new device id. The transition also
    carries the old journal's day and month counters and the digests of the
    Requests the old wallet can show are undecided (§7.2).
  - `device_time_ms` is the signer's own time, or for a SendSplit under a
    time-dependent control the effective time (§5.4). Day and month are the
    UTC day and UTC calendar month of that value, so the counters need no
    index field.
  - The certificate digest covers E. A transition therefore names the
    enrollment statement its signer held.
  - The preimage and its digest function are part of what the relation fixes
    (§2.3, Q0).
- **Request** (receiver to payer; about 0.90 KB with one witness): `rules_lo`
  and `rules_hi`, the lowest and highest rules version the receiver can judge
  (§5.11); receiver certificate and receipt; nonce; amount (or zero plus a
  maximum); `created_at_ms`, the receiver's own time (§5.4); anchor flag;
  clock-reset mark (§5.4); what the receiver holds (the highest notice epoch,
  the policy head, `list_epoch` and list counter, root succession number);
  optional notices and an optional list segment. The receiver's device key
  signs it, so that nobody can ask for payment in another device's name. It
  is single-use.
- **Payment** (payer to receiver; about 1.14 KB with one witness, and about
  7.7 KB with a proof at the repo's 6,528 B ceiling): payer certificate and
  receipt, the signed SendSplit, the proof (§2.1), what the payer holds (as in
  the Request), optional notices and an optional list segment. Only the
  SendSplit is signed.
- **Outcome** (receiver to payer; about 0.2 KB): payment id, verdict
  (`Credited` or `Refused`), a reason code, the receiver's device id,
  optional notices and an optional list segment outside the signed part. The
  receiver's device key signs it. The reason code tells the payer's app what
  to show: time, limit, expiry, block, version, stale Request, fee, policy,
  proof. "Policy" is a certificate whose E does not satisfy the entry it
  names. "Proof" covers a proof that does not verify and a receiver that
  could not make its own (§5.2).
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
  which is what an anchor needs (§5.4). A wallet folds a countersigned
  Migrate once; its marker holds a flag for that (§5.10).
- **Resume record** (wallet to issuer; about 0.55 KB with nothing pending, up
  to about 3 KB): what a wallet presents at its first sync after a resume
  (§5.10). It is the checkpoint the wallet resumed at, with the commit
  counters of the two ends of the gap. It holds the wallet's last transition,
  complete with its device signature, the balance, the redeemed total, the
  last voucher number folded, the Migrate flag, and the lists of open
  Requests and unresolved payments. It is not a new signed object. The device
  signature on the last transition is what the issuer verifies; the balance
  in it is the wallet's own statement, as a balance always is (§5.7). The
  issuer accepts it on three checks, each on an input it holds: the device
  key's signature on the last transition; the same sequence number and digest
  as the acknowledged head, or a higher sequence number; and cumulative totals
  not below those at the acknowledged head. It cannot replay the commits
  inside the gap. §7.1 and §7.2 say what the issuer does with it at a sync, a
  renewal and a Migrate.

| Object | Signed by | Size (estimate) | Travels |
|---|---|---|---|
| Scheme descriptor | root key | not estimated | issuer to wallet |
| Policy entry | root key | 0.3 to 0.5 KB, rough | in the descriptor; as a notice |
| Root-signed notice | root key | 0.11 KB plus body | issuer to wallet; peer to peer |
| Enrollment statement E | nobody by itself: the issuer signs the certificate that carries it, and the witnesses' receipt names that certificate | 0.14 KB Android, 0.13 KB iPhone | inside the certificate |
| Device certificate | issuer certificate key | 0.53 KB Android, 0.48 KB iPhone; 70 B more with a fee policy | in every Request and Payment |
| Registration receipt | k of n witness keys | 150 B, plus 67 B per extra witness | in every Request and Payment |
| Registration authorization | issuer | not estimated | issuer to account to ledger |
| Voucher | voucher key | 0.31 KB | issuer to wallet |
| Transition | the wallet's device key | 315 B; a SendSplit with a fee 380 B | SendSplit in the Payment; every kind to the issuer at sync; RedeemSplit, fee settlement and evidence to the ledger |
| Request | receiver's device key | 0.90 KB | receiver to payer |
| Payment | the SendSplit inside it, by the payer's device key | 1.14 KB; 7.7 KB with a proof | payer to receiver |
| Outcome | receiver's device key | 0.2 KB | receiver to payer |
| Block-list segment | list key | 0.1 KB plus 40 B per entry | issuer to wallet; peer to peer |
| Resume record | the last transition in it, by the device key | 0.55 to 3 KB | wallet to issuer |

**Totals against the 10 KB bound.** The owner said: "we really cannot have
payment data exceed around 10k or so". The figures are for an Android payer
and an Android receiver, one witness, nothing optional attached, in canonical
binary form before framing. An iPhone's certificate is about 46 B smaller.

| | No fee policy | With a fee policy | Limit of the repo's existing profile |
|---|---|---|---|
| Request | 0.90 KB | 0.97 KB | 1,024 B |
| Payment without a proof | 1.14 KB | 1.27 KB | |
| Payment with a proof of 6,528 B | 7.66 KB | 7.80 KB | 7,552 B, of which 6,528 B proof |
| Outcome | 0.2 KB | 0.2 KB | 256 B |
| Whole exchange without a proof | 2.24 KB | 2.44 KB | |
| Whole exchange with a proof | 8.77 KB | 8.97 KB | 9,211 B |

- With a proof the Payment is under 10 KB by about 2.3 KB without a fee
  policy and about 2.2 KB with one. A second witness adds 67 B to the Request
  and to the Payment. A notice or a list segment carried in a message counts
  against that message (§5.6).
- The existing limits are those of `specs/peer_transport_v1.md:65-71`. The
  Payment with a proof is over that profile's limit by about 0.1 KB without
  a fee policy and about 0.25 KB with one. The whole exchange is inside its
  gate by about 0.45 KB and about 0.25 KB. The limits of the new profile are
  among the targets Q0 fixes (§2.3).
- 6,528 B is the repo's ceiling for the proof that travels under the existing
  relation. No proof has been produced, and the size of a proof of the
  relation of §2.1 is not known (§2.2).
- Without a proof the Payment is about 8 or 9 frames and the Request about 7
  under the repo's QR framing. Computed, not measured; §5.6 has the framing.
- What E costs: about 0.14 KB in every Request and every Payment.

### 5.2 Payment exchange

One rule covers every object a wallet signs and commits: a Request, a
SendSplit, a ReceiveFold with its Outcome, a `Refused` Outcome, a RefundFold,
and every other transition of §5.1. The steps run in this order. None runs
beside another.

1. Pre-check. The wallet is ready (§5.10), and every check that could refuse
   this object passes when evaluated now.
2. Sign in memory.
3. Prove, in memory. Only where the object is a transition: the wallet makes
   the proof of its state after the transition ("Under the proof", below).
4. Create the marker for the new commit. It carries the checkpoint of the new
   state, with the signed object where §5.10 lists it. For a transition the
   proof entry of §5.10 is written first, in the same step. Done when the
   creation is confirmed by a read.
5. Commit: one durable journal transaction holds the object, its signature,
   its proof and its state change.
6. Delete the previous marker and confirm that it is absent.
7. Release the object.

What each step is for, and what a failure does.

- Steps 1 to 3 change nothing. Before step 4 a signature and a proof exist
  only in memory: they are never logged, stored or passed to another process.
  If a check, the signing or the proving fails, the signature is discarded,
  nothing is debited and the wallet is as it was. The hardware has still made
  that signature. If the wallet later signs a different object at the same
  sequence number, two signatures at one sequence number have existed and
  only one has ever left memory. That is safe because the app is the
  released app (T3); the proof needs the signature, so signing cannot come
  after proving.
- Step 4 fixes the wallet's state. From then on the phone's key store holds
  the new state and the signed object, whatever happens to the files. After
  step 4 there is no discard. The payer is debited at step 4.
- Step 5 is there so that a key store that loses a write in a power cut can
  be repaired from the files (§5.10). If the journal write fails, for example
  because storage is full, the wallet waits at the new state until the write
  succeeds. It does not go back.
- Step 6 is there so that no earlier state remains in the key store when the
  object leaves the phone. If step 6 cannot be confirmed, step 7 does not
  happen. The object stays unreleased until recovery finishes the deletion
  (§5.10).
- The marker step of a commit is steps 4 and 6. It is confirmed when step 6
  is done.

Durable means SQLite `synchronous=FULL`, and on Apple platforms `fullfsync`
and `checkpoint_fullfsync`. The existing Swift suite sets these options
(`IrohaSwift/Sources/IrohaSwift/KagemushaAttested/KagemushaAttestedDatabase.swift:53-54`).
No forced power-off test of them is recorded; the evidence gate runs one
(§2.3). The wallet cannot force a key-store write to storage. §5.10 says what
it relies on there and what is open.

A signature that changes no wallet state takes none of these steps, for
example the device key's signature over the issuer's nonce in a sync. It is
made only when the wallet is ready. A wallet that is waiting or stopped
(§5.10) signs nothing.

**When a transfer is complete (PC).** A wallet reports a transfer as complete
only when nothing more is needed for P1 to P4 (§1).

- Receiver. In order: its checks of the Payment; the proof of its own state
  after the ReceiveFold, which its own onward spending needs; the durable
  commit of the ReceiveFold and the `Credited` Outcome; the confirmed marker
  step. Only then does it release the Outcome and show the credit. From that
  moment the amount is in its balance and it can pay it onward with no
  further step.
- Payer. Its debit and its marker step are finished before the Payment is
  released, so before the receiver can report anything. The payer's wallet
  shows "sent, not confirmed" until it has stored a `Credited` Outcome. It
  shows "returned" only after a RefundFold has committed and its marker step
  is confirmed.
- No wallet commits a transition it has not proven. There is no state in
  which received value waits for a proof.

What each wallet shows at each point:

| Point in the exchange | Payer's wallet shows | Receiver's wallet shows |
|---|---|---|
| The Request is committed and released | | The Request, and that it waits for a payment |
| The payer's pre-check fails | The reason; on a time failure, both dates. Nothing is signed or debited | The Request, unchanged |
| The payer confirmed; steps 2 and 3 run | "Preparing". The balance is unchanged | |
| The payer's step 4 is done; step 5 or 6 is not yet confirmed | "Not sent yet". The amount is out of the balance. The Payment is held until the wallet is ready | |
| The Payment is released (step 7) | "Sent, not confirmed": the amount, the receiver's device id, and the Payment to show again | |
| The receiver has the Payment; its checks, proof and steps 4 to 6 run | "Sent, not confirmed" | "Checking". No credit |
| The receiver's step 4 is done; step 5 or 6 is not yet confirmed | "Sent, not confirmed" | "Not received yet". No credit and no Outcome |
| The receiver's marker step for the credit is confirmed | "Sent, not confirmed" until it stores the Outcome | "Received": the amount is in the balance, and the Outcome to show |
| The receiver's marker step for a refusal is confirmed | "Sent, not confirmed" until it stores the Outcome | "Refused", the reason, and the Outcome to show |
| The payer has stored a `Credited` Outcome | "Complete" | "Received" |
| The payer has stored a `Refused` Outcome; the RefundFold is not yet committed | "Refused, not yet returned", and the reason | "Refused" |
| The payer's marker step for the RefundFold is confirmed | "Returned": the amount and the fee are back in the balance | "Refused" |

The payer's wallet counts the amount as spent from step 4, whatever it shows.
Storing a received Outcome is not a commit and takes no marker step. If the
payer's files are lost after it stored a `Credited` Outcome, its wallet shows
"sent, not confirmed" again, and showing the Payment again returns the same
Outcome. Nothing depends on that display except what the user is told.

**Authentic.** A Payment is authentic for a receiver when three things hold:
its certificate and receipt verify under issuer and witness keys the receiver
knows, revoked or not; the device id recomputes from the certified key; and
the device signature verifies. Every other check in item 4 below is a
judgment.

**The exchange.**

1. **Request** (receiver). Receiver certificate and receipt, nonce, amount (or
   zero plus a maximum), `created_at_ms`, `rules_lo` and `rules_hi`, anchor
   flag, clock-reset mark, what the receiver holds, optional notices and an
   optional list segment (§5.1). `created_at_ms` is the receiver's own time
   (§5.4). `rules_lo` and `rules_hi` are the lowest and highest rules version
   the receiver's app can judge (§5.11). The Request is signed and committed
   under the rule above before it is displayed, and it is single-use. It is
   not a transition and takes no proof.
   - A wallet creates no Request while its requesting is suspended (§7.1
     lists the cases). The clock-reset state (§5.4) is not one of them: a
     wallet in that state still creates a Request and sets the clock-reset
     mark.
   - The Request is open for a new payment for a few minutes of monotonic
     time, in the boot that created it. After that, or after a reboot, it is
     closed.
   - The checkpoint lists the Requests that can still be decided, at most
     four (§5.10). Only that list makes a Request payable. A Request leaves
     the list when it is decided, or when a fifth Request needs its place;
     the oldest then goes, in the same commit, and is closed if it was still
     open.
   - The receiver also keeps every Request on record in its files, open or
     closed, with any Outcome it stored, for as long as the files hold them.
     This is for the payer: a payment or a refusal that did not cross can
     still be settled when the two phones meet again. The cost is storage,
     about 0.3 KB per Request (estimate, not measured).
2. **Pre-check** (payer). The payer applies the rules the receiver will apply,
   on the payer's own clock, list and counters. If any check fails, nothing is
   signed and the payer's app shows the reason; on a time failure it shows
   both dates.
   - The Request's certificate and receipt verify under issuer and witness
     keys the payer knows. Where a key is revoked, the certificate stands or
     not under the revocation rule (§5.1, §5.11). The device id recomputes,
     the certificate is within its tier row, and the Request's signature
     verifies under the receiver's device key.
   - The rules version the payer would sign under lies between the Request's
     `rules_lo` and `rules_hi` (§5.11). This check is there so that a payer
     is never debited by a transition the receiver cannot judge.
   - If the Request carries the clock-reset mark and the payer's certificate
     carries a lease or a limit, the payer refuses. A payer whose certificate
     carries neither pays.
   - The payer's own sending is not suspended (§7.1 lists the cases).
   - The payer's records show no unresolved SendSplit for this Request. If
     they show one, the payer presents that Payment again and signs nothing
     new.
   - A payer whose certificate carries no time-dependent control does not use
     the Request's time (§5.4). It signs its own time, and of the time checks
     below it applies only the `receive_not_after` check. Where items 3 and 4
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
   - The receiver is not `receive_blocked` in the payer's list. The
     receiver's `receive_not_after`, if set, is not earlier than `t_eff`.
   - The amount matches the Request and is within `max_payment`. The fee is
     the one §5.8 requires, under the certificate's `fee_policy_id`. The
     balance covers the amount plus the fee.

   A receiver with a newer list or a different clock can still refuse.
   Outcome and RefundFold handle that.
3. **Sign, prove and commit** (payer). The pre-check is evaluated again as
   step 1, immediately before signing. The SendSplit carries `device_time_ms
   = t_eff` if the payer's certificate carries a time-dependent control, and
   the payer's own time if it carries none (§5.4). It also carries the
   Request as subject, the receiver's device id as counterparty, and the fee
   with its `fee_policy_id` (§5.8). The wallet signs it, proves its state
   after it, and runs steps 4 to 7. A failed signing or a failed proof leaves
   the wallet unchanged. The payment id is the digest of the SendSplit; the
   Outcome, the ReceiveFold and the RefundFold each name it. The Payment is
   released after the previous marker is confirmed absent.
4. **Receive.** The receiver first looks up the payment id. If it holds a
   stored Outcome for it, it returns that Outcome and does nothing else.
   Otherwise it checks that the Payment is authentic and that its subject is
   one of its own Requests for which the table below gives a signed answer.
   If either fails, it signs nothing. Where the table says `Refused`, it
   refuses. Where the Request is in the checkpoint's list and open, it judges
   the Payment:
   - the certificate and receipt stand under the revocation rule; the
     certificate is within its tier row; the receipt names the certificate
     that carries the enrollment statement E, so E is the statement the
     validators sealed; E satisfies the policy entry it names, which the
     receiver holds (§4, §5.1). The receiver does not verify the seal
     itself, and no Payment carries it (§8.1);
   - a rules version that the Request listed; itself as counterparty; the
     Request open; the amount;
   - the time checks of §5.4 on the SendSplit's `device_time_ms`, which the
     receiver makes only where the payer's certificate carries a
     time-dependent control; expiry; limits and its own tally;
   - the block list; the fee (§5.8);
   - no conflict with the last transition seen from this payer (§5.3);
   - the proof in the Payment verifies.

   If everything holds, the receiver signs the ReceiveFold, proves its state
   after it, then signs the `Credited` Outcome, and commits both objects and
   the verdict together in one commit. The Outcome is signed only after the
   proof exists, so that a failed proof does not leave a signed `Credited`
   Outcome behind. If a judgment fails, or the receiver cannot make its
   proof, it signs a `Refused` Outcome and commits it with the verdict. That
   commit decides the Request. A refusal is not a transition and takes no
   proof.

   Either commit runs steps 4 to 7, and the Outcome is released only after
   the marker step is confirmed. A `Refused` Outcome takes the marker step
   too: without it a receiver could refuse, let the payer refund, put its
   earlier files back and credit the same payment.
5. **Outcome** (receiver): `Credited` or `Refused`. Presenting the Payment
   again returns the stored Outcome. That lookup comes first, before any
   other check.
6. **RefundFold** (payer): only for a `Refused` Outcome that verifies under
   the counterparty key named in the payer's own SendSplit, or for the
   `Refused` Outcome of that counterparty's Migrate successor (§7.2). A
   successor's Outcome carries the old key's signed Migrate and the
   successor's certificate. The payer verifies the Migrate under the
   counterparty key named in its SendSplit, finds the digest of its Request
   among those the Migrate lists, and verifies the Outcome under the device
   key of the device id the Migrate names. Either way it folds once per
   payment id, and only for a payment id that the payer's checkpoint lists, or
   covers by its digest, as unresolved (§5.10). Never on a timeout. One other
   signer is accepted: the successor of a Migrate (§7.2). Its `Refused`
   Outcome carries, outside the signed part, the countersigned Migrate and
   the successor's device public key. The payer checks that the Migrate
   verifies under the counterparty key named in its own SendSplit, that it
   lists the Request which that SendSplit names as subject, that the key
   shown hashes to the device id the Migrate names, and that the Outcome
   verifies under that key. A payment that the checkpoint holds only in
   short form cannot be checked this way, because that form does not hold
   the Request's digest (§5.10). The relation of §2.1 has no clause for a
   RefundFold on such an Outcome; Q0 fixes one (§2.3). The
   RefundFold is a transition: the payer proves its state after it before
   step 4. The payer acts on the first valid Outcome it stores for a payment
   id. A second, different Outcome for the same id changes nothing in the
   payer's wallet; it is evidence against the receiver (§5.3).

**Which Payments get a signed answer.** A receiver signs an Outcome only where
it can show what it did with the Request. The receiver's own app applies this
rule from its checkpoint and its journal.

| The Payment names | The receiver's answer |
|---|---|
| A payment id for which the receiver holds an Outcome, in its journal, in an older file that §5.10 lets it use, or in the checkpoint's list of last decisions | That Outcome, over the same bytes |
| A Request in the checkpoint's list that is open | Judged: `Credited` or `Refused` |
| A Request in the checkpoint's list whose time has run out, or whose boot has ended | `Refused`, reason stale Request |
| A Request that is no longer in the list, where the journal holds every commit made since the Request was created and shows it closed or decided for another payment id | `Refused` |
| Any other Request: not this wallet's, unknown to it, or created before a gap in its journal (§5.10) | No signed object |

The last row is the rule after a resume. A wallet whose files were lost
cannot rule out that it credited a payment in the part of its history it no
longer holds. A refusal would then let the payer refund a payment that was
received. So it signs nothing, and never `Refused`. The cost falls on the
payer: if its Payment had been refused and the Outcome never reached it, or
if the Payment arrives late for a Request that had closed, it cannot refund,
and the amount stays in neither wallet. The receiver gains nothing by it.

After a Migrate, the successor answers `Refused` for the Requests that the
Migrate lists as undecided, and for those only (§7.2). The old wallet lists a
Request there only if it can show it undecided under the table above. Such an
Outcome is signed by the successor's device key, which is not the counterparty
key that the payer's SendSplit names. It therefore carries the countersigned
Migrate and the successor's device public key. The payer folds a refund on it
only if all of these hold: the Migrate's signature verifies under the
counterparty key named in the payer's own SendSplit; the issuer's
countersignature on the Migrate verifies under an issuer key the payer holds;
the Migrate names, as successor, the device id of the key that signed the
Outcome; and the Migrate lists the digest of the Request that the SendSplit
names. Item 6 above and the RefundFold rule of §2.1 (H7) accept this form
beside an Outcome under the counterparty key itself. The added size is not
estimated, and Q0 fixes how the relation checks it (§2.3). The
successor signs such an Outcome with its own device key, which is not the
counterparty key the payer's SendSplit names. The payer therefore folds a
refund on it only if the Outcome comes with the countersigned Migrate and
these hold: the Migrate is signed by the counterparty key its SendSplit
names and countersigned under an issuer key the payer holds; the Migrate
names the successor's device id, which the payer recomputes from the key
that signed the Outcome; and the Migrate lists the digest of the Request
its SendSplit names.

A receiver signs nothing for a Payment it cannot parse. An unmodified payer
never sends one, because of the version check in item 2.

**Presenting again.** A payer shows the same Payment as often as needed. It
never signs a second SendSplit for a Request while its records show an
unresolved SendSplit for it. A receiver shows the same Request and the same
stored Outcome as often as needed. Nothing in this section makes an
unmodified wallet sign two different objects for one sequence number or two
different Outcomes for one payment id. After a resume the payer can present
again the two newest unresolved Payments, which its checkpoint holds in full;
for older ones it can still fold a refund and cannot present the Payment
again (§5.10). A payer that lost its files and pays the same Request a second
time signs a second SendSplit at a new sequence number. The receiver credits
at most one of the two. It answers the other `Refused`, and the payer refunds
it, unless the receiver has lost its own record of that Request too; then
the table above gives no answer.

**Under the proof.** The design carries a proof on every payment. Its relation
is in §2.1: for every transition in the value's history, one P-256 device
signature under the key in the proven predecessor state, the sequence, link
and counter rules, and that the state carries a valid enrollment statement E
under a policy entry. No vendor certificate chain is checked on a phone.

- What the Payment carries: the payer's certificate, receipt and E, the
  signed SendSplit, the proof of the payer's state after the SendSplit, what
  the payer holds, optional notices and an optional list segment (§5.1).
- When each proof is made. The payer proves after it signs the SendSplit and
  before step 4. The receiver verifies that proof in its judgment. The
  receiver proves its own state after the ReceiveFold before its step 4. A
  payer that folds a refund proves before step 4 of the RefundFold. The same
  holds for MintFold, RedeemSplit, Recertify, Migrate and MigrateFold.
- No transition is committed unproven. A SendSplit that cannot be proven is
  never signed into the wallet's state and never released; the balance is
  unchanged. A receiver that cannot prove its fold refuses, and the payer
  refunds when it holds the Outcome. The receiver may try the proof again
  while the Request is open and undecided. A payer that cannot prove its
  RefundFold does not commit it; its wallet shows "refused, not yet
  returned", and the amount is back only when the proof succeeds.
- What the receiver checks without the proof, on the last hop only: the
  payer's certificate, receipt and E as sealed by the validators, the device
  signature, that the SendSplit answers its own Request, and the enabled
  controls on its own time, tally and list. Those checks say nothing about
  earlier hops. The proof is what covers them.
- What the proof does not check: that the payer's state has only one
  successor, what time it was, which block list the payer held, or that the
  payer used its marker (§2.1, §4).

**Time.** Both proofs fall inside the payment. Between the payer's
confirmation and the receiver's "received" lie: on the payer, one hardware
signature, one proof, one marker step with its proof entry and one durable
commit; the transfer of the Payment, about 7.7 KB with the proof (§5.1,
estimate); on the receiver, the verification of the payer's proof, two
hardware signatures, one proof, one marker step with its proof entry and one
durable commit. The repository's gates are 10 s for
one proof and 1 s for one verification, at the 95th percentile. They are
gates, not measurements. No prover exists that can make such a proof (§2.2),
and nothing in this list has been timed on a phone. The owner said "1-2 s
should be good ux". A proof-carrying payment does not meet that unless each
proof takes a fraction of a second and the carrier moves the Payment in well
under a second. The evidence gate times the exchange without a proof, and Q2
times the proofs (§2.3). §2.3 lists shapes that could shorten the wait, as
choices to evaluate. None is designed or checked, and this section does not
rely on any of them.

**An undelivered payment is not a completed payment.** The exchange is not
atomic. The payer is debited at its step 4. If the Payment never reaches the
receiver, or a `Refused` Outcome never reaches the payer, the payer's wallet
stays at "sent, not confirmed" and the receiver's wallet shows no credit.
The amount is in neither wallet until the two phones meet again. If they
never do, or if the receiver has lost the record of the Request by then, the
payer has lost the amount. P1 to P5 speak of a completed payment and are not
engaged; PC is met because neither wallet reports complete. It is still a
way for an honest payer to lose value. No rule here forces either side online
to settle it, and no offline rule can refund on a timeout without letting a
payer refund a payment that was credited. A relay through the issuer when
both sides sync is an owner decision (§12).

### 5.3 Evidence

Evidence is a set of objects signed by device keys or issuer-side keys that
shows, to anyone, that some key signed what the rules never let it sign. It
is publicly verifiable and accepted on-chain from anyone
(`SubmitKagemushaEvidence`, §6). The chain checks every device signature in
it against the device key in the registry row, and every issuer-side
signature against the scheme cell.

Let Δ(t) be the outflow a transition must add: its amount for SendSplit and
RedeemSplit, zero otherwise. `cum_out_after` is gross and never decreases.

**Honest paths.** On a phone where T1 to T3 hold (§4), the wallet produces
none of the evidence below. §5.10 gives the reason: the wallet's state is in
the key store, every earlier checkpoint is deleted before an object is
released, and no restore of files changes the key store. An honest holder's
key can still sign such evidence in three ways. Each is a failure of T2 on
that phone.

- Lost key-store writes. A power cut loses the key-store writes of the last
  marker step after its object was released, and the journal commit of that
  step is also lost, or the files are removed or replaced before the next
  start. The wallet then continues one state back, and its next object
  conflicts with the released one. §5.10 calls this the one exposure. On
  Android the reference source syncs each key-store write. On iPhone the
  source reading is unfavourable and nothing is tested.
- A restored key-store entry. A platform or vendor tool puts an older marker
  back while the device key still works. No such tool is known and none has
  been tested.
- Someone else's code with the wallet's privileges. Malware that controls the
  operating system can make the holder's key sign two successors of one
  state.

The evidence gate decides the first two per tuple (§2.3). A tuple on which
either occurs is unsupported. Nothing decides the third in advance.

| Evidence | Signatures it rests on | Path on an unmodified phone |
|---|---|---|
| Two different digests at one `(device id, seq)` | Two by one device key | None while T2 holds. The three paths above. A second signature over the same bytes has the same digest and is not evidence |
| Adjacent transitions whose previous-digest link is broken, or where `cum_out_b ≠ cum_out_a + Δ(b)`; the same for `cum_fee_after` (§5.8) | Two by one device key | None while T2 holds. The three paths |
| `seq_a < seq_b` where `cum_out_b < cum_out_a + Δ(b)` | Two by one device key | None while T2 holds. The three paths |
| `seq_a < seq_b` with a lower certificate serial at b | Two by one device key | None while T2 holds. The three paths. A wallet moves to a higher serial by Recertify and never goes back |
| `seq_a < seq_b` under one certificate with a lower `device_time_ms` at b | Two by one device key | None while T2 holds. The three paths. §5.4 keeps every signed time at or above the last signed one, and the checkpoint holds the time records |
| Within one certificate and one day or month index, a gross-sent or refunded counter that regresses | Two by one device key | None while T2 holds. The three paths |
| A transition naming certificate serial N above a device-signed `Recertify` to a higher serial | Two by one device key | None while T2 holds. The three paths. A head declared in a sync request is not evidence, and neither is an issuer-signed statement alone, so a lost renewal response cannot accuse an honest wallet |
| Any transition above a `Migrate` of the same device id | Two by one device key | None while T2 holds. The three paths. A wallet signs nothing after it commits a Migrate, and after a resume its checkpoint still holds the Migrate as its last transition |
| A Bootstrap at a seq other than 0, shown with any transition of the same device id at a lower seq | Two by one device key | None |
| Two folds of one inbound credit by one device id: two ReceiveFolds or two RefundFolds that name one payment id, two MintFolds that name one voucher, two MigrateFolds | Two by one device key | None while T2 holds. The three paths. The checkpoint holds what makes each credit foldable once (§5.10) |
| `Credited` and `Refused` Outcome for one payment id; a ReceiveFold and a `Refused` Outcome for one payment id | Two by one device key | None while T2 holds. The three paths. The stored-Outcome lookup of §5.2 comes before every other check, each Outcome commit takes the marker step, and after a resume a wallet signs no answer for a Request whose record it lost |
| A certificate and receipt whose device id has no registry row | The certificate key and the witness quorum | None by the phone. It shows misuse of issuer-side keys (§8.1) |
| A MintFold whose voucher matches no on-chain load | The voucher key, and one by the device key | None by the phone. It shows misuse of the voucher key (§8.1). The phone that folded the voucher cannot check the chain offline |

A ReceiveFold after an outflow of 100 carries `cum_out = 100` and Δ = 0, so it
is not evidence. Conformance vectors cover every kind in both positions.

**Fork evidence under the per-hop relation.** The relation proves, for every
transition in a history, the sequence number, the previous-digest link and
the cumulative counters, and it rejects a ReceiveFold whose credit was signed
by the folding device's own key (§2.1). What follows, and its limits:

- A phone with one enrolled instance and no accomplice cannot pay honest
  receivers more than its proven balance without leaving a pair of its own
  signed transitions in their hands. Take two of its payments from different
  branches. At one sequence number they are two digests. At adjacent numbers
  the link is broken. Further apart, the later one fails the
  cumulative-outflow line above, because a branch raises its proven
  `cum_out_after` only by a proven outflow that debits the same branch. This
  argument is this document's own. It has not been checked by a second pass
  or written as a test.
- The pair is evidence only when both transitions reach the chain. Each sits
  in one receiver's journal, and no holder has to sync. With R8 off the pair
  may never meet.
- None of it holds if one branch is paid to a second enrolled instance the
  attacker controls, or to a receiver who colludes. A second profile on the
  same phone is enough. The conflicting signature then exists only as a
  private witness inside the next proof, and no one else ever holds it.
- A receiver's checks without the proof see one hop. Where a payment is
  judged on those checks alone, a phone that controls its own counters can
  pad one branch with a transition nobody sees, and two receivers'
  transitions are then sure to be evidence only at the same or adjacent
  sequence numbers.
- Evidence identifies a phone on which an assumption failed, in some cases.
  It does not prevent the second branch and it does not bound what that
  phone creates. It is not part of the argument for P2.

**What evidence does.**

- It never touches value that others received. No wallet, issuer or ledger
  rule reads evidence against a payer when it judges a receiver's balance,
  the receiver's payments, the receiver's renewal or the receiver's unload
  (P3, P4). A payment that a receiver's wallet reported complete stays as it
  was, whatever is later shown about its payer.
- It places a hold, and only in one case: two conflicting signatures by one
  device key, both verified on-chain against that key's registry row. Every
  line of the table that rests on "two by one device key" is such a case.
  The chain places the hold when it accepts the evidence. No party can place
  one by its own judgment: not the issuer, not the registry authority, not a
  witness.
- The hold acts on that key's row. If that key's device id was retired by a
  Migrate, it acts on the row that took over its balance, the live row at
  the end of the succession chain (§7.2, §8.2). It acts on no other row.
- While the hold stands the row takes no Load and records no new unload
  claim, and its recorded claims are not paid (§8.2). The issuer refuses the
  device a renewal; a device's own key having signed two successors is the
  one reason for refusing a renewal that is not an enabled regulatory
  control (§7.1). Where the scheme runs the block list (R6), consensus emits
  an entry for the device id, and holders of the entry refuse the device
  (§5.5).
- Evidence of an issuer-side fault, the last two lines of the table, places
  no hold on the phone that holds the object, changes nothing in its wallet
  and refuses it nothing. It is recorded against the issuer-side key. What
  the root then does with that key is §5.11. The value concerned stays where
  it is; §3.2 says what that means for the ledger.

A hold reaches only a phone on which an assumption has failed. It is
containment after the failure; it is not what makes a payment secure. It can
still fall on an honest holder: through the first two paths above on a tuple
that the gate did not catch, through malware, or on a Migrate successor whose
old phone was passed on before its key was deleted (§7.2). In each
case the hold freezes that holder's whole balance on the ledger side,
including value received from others. What such a holder gets back, and who
may order it, is an owner decision (§3.2, §12). That decision is needed
before production value: until it is taken, a hold has no end.

### 5.4 Time, limits, expiry

Three controls depend on time: limits (R7), expiry with its reboot policy
(R8), and receive freshness (R6, §5.5). No phone gives an app an attested
clock (§4). Each rule below therefore names who checks it and with what
input. Each rule that can require connectivity belongs to one of these
controls, applies only where the scheme has switched that control on, and is
listed at the end of the section.

A certificate carries a time-dependent control if its effective terms (§5.1)
have a `Lease`, a day or month `Limit`, or a per-counterparty `Cap`. Counting
the per-counterparty cap as a limit in the sense of R7 is this document's
reading. A scheme that switches none of these on issues certificates with
none. For a wallet under such a certificate no rule in this section stops a
payment.

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
- **Where the time records live.** The marker holds the time records as of
  the wallet's last commit, and the last signed time is in the transition the
  marker carries (§5.10). A wallet whose files are older, missing or damaged
  takes its floor and its anchor from the marker. Putting older files back
  therefore lowers no floor below that of the last commit and opens no
  window earlier than the last signed time. What only the files held is
  lost with them: an anchor, a last issuer time or an anchored time
  recorded after the last commit (§5.10, N1).
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
  rule is checked by the wallet's own app from its marker and journal, by the
  issuer when it replays the journal, and by anyone who holds two transitions
  of one device under one certificate with the later one carrying the lower
  time (§5.3).
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
checks in its pre-check, and again as step 1 of §5.2, immediately before it
signs. Where the payment carries a proof, the proving time lies between that
check and the marker for the new commit, and nothing is checked again in
between. If any fails, nothing is committed, nothing is
debited, and both apps show both dates.

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

`t_eff` is fixed when the SendSplit is signed. Where the payment carries a
proof, the payer proves after it signs and before it commits (§5.2), so the
time the proof takes lies between the signed time and the release. The
Request must still be open when the receiver judges the Payment. The period a
Request stays open (below) therefore has to cover both proving times and the
transfer. The repo's gate for one proof is 10 s; nothing is measured (§2.3).

**What the receiver checks.** Input: the signed SendSplit, the payer's
issuer-signed certificate with its receipt, the tier row the receiver holds,
and the receiver's own committed Request. Let `T` be the SendSplit's
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
| Request validity | a few minutes; descriptor value | monotonic time since the Request was made | A Request is paid only while it is fresh, so its `created_at_ms` is close to the time of payment. It must be shorter than `W`, and long enough for a proof-carrying payment to finish |

**Limits (R7)**

- Day and month are the UTC day and UTC calendar month of the signed time. The
  limit check is `gross_sent − refunded ≤ limit`. A RefundFold adds to the
  refunded counter of a window only when its SendSplit carries the same
  window.
- A wallet's own counters are in its last transition, and the marker holds
  that transition (§5.10). A wallet that resumes after losing its files
  continues with the same counters. An ordinary user cannot get a second
  allowance by putting older files back.
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
- **What a resume does to the tally.** The tally is kept in the wallet's
  files and is not in the marker. After a resume (§5.10) the wallet rebuilds
  it from the credits its files still hold for the current day and month.
  Credits inside the gap are not counted. One payer can then pass its limit
  at this receiver once more in that window.
  - Nobody is affected while the payer's app is unmodified: that app keeps
    its own counters, and they are in its own marker.
  - Against a compromised payer the bound per receiver becomes one limit per
    window, plus one more for each resume of that receiver in the window. An
    ordinary receiver can cause a resume by putting older files back.
  - The stricter form keeps two amounts in the marker: the largest sum
    credited from any one payer in the current day, and the same for the
    month (about 34 B). After a resume the wallet counts every payer as
    having already reached those sums. No payer's limit can then be passed.
    The cost falls on an honest receiver that had taken a large sum from one
    payer before it lost its files: it refuses payers with smaller limits
    until the window turns.
  - The text above describes the first form. Which form applies is an owner
    decision (§12).
- The limit subject is the account. The issuer splits the account's limit
  across the certificates of its devices, so that the limits in the live
  certificates of one account never add up to more than the account's limit.
  Input: the registry's binding of device ids to accounts, and the issuer's
  own record of certificates.
- **One rule for a device that leaves.** A device's share returns to its
  account only when its certificate can no longer send. §7.2 states the same
  rule for each flow. In each case:
  - Migrate. The Migrate is the old wallet's last transition, and the
    unmodified app signs nothing after it. The share and the consumed amounts
    move to the successor together (below).
  - A device declared lost. Its share stays counted against the account until
    the end of the UTC day, and for the month limit the end of the UTC month,
    in which its `not_after + expiry_grace` falls. Until then a new device of
    that account gets only a share that was not allocated.
  - A device declared lost under a `Never` certificate. Its certificate can
    always send, so its share does not return. The account's usable limit
    stays reduced by that share unless the device comes back and syncs. A
    tier that combines `Never` with limits has this cost; whether a tier may
    combine them is an owner decision (§12).

  Declaring a device lost has no effect offline unless the scheme has
  switched on holder-requested blocking (§5.5), and then only among holders
  of the list. So the rule does not rely on it.
- **Opening counters.** A certificate's `opening_day` and `opening_month` are
  where the wallet's counters start for the day and the month that contain
  `not_before`. Other windows start at what the journal already holds for
  them: zero, unless the wallet signed ahead into that window (§7.2). The
  issuer sets the opening counters:
  - at a renewal or a Recertify, from the journal the wallet uploads: the net
    amount sent under every signed time in or after the current day, and the
    same for the month. Amounts the wallet signed under a time ahead of issuer
    time are counted now, so they cannot be spent a second time when that date
    arrives;
  - at a renewal that carries a resume record (§5.1), from the last
    transition in it, because the issuer cannot replay the gap. Signed times
    never go down under one certificate. So a last transition dated before
    the current day shows that nothing was sent today, and one dated today
    carries today's counter; the same holds for the month. If the last
    transition is dated ahead of issuer time, in a later day or month, the
    gap may hide amounts signed in the current one. The issuer then sets the
    opening counter of that window to the limit. That delays sending to the
    next window and forces no sync;
  - for a Migrate successor, in the certificate it receives at the renewal
    after its MigrateFold (§7.2): the same figures from its own journal, to
    which the MigrateFold has added the old device's counters;
  - for a device enrolled after another device of the account was declared
    lost, zero, with only a share that was not allocated (the rule above).

  This is for an ordinary user who would otherwise renew, rotate the key or
  re-enroll to get a second allowance in the same window. The cost falls on an
  honest user who loses a phone: the new phone cannot send the lost phone's
  share until that phone's lease and grace have ended. It delays sending and
  forces no sync. It is not a bound against a compromised phone, whose
  counters are its own statement.

**Expiry (R8) and receive freshness**

- Send expiry gates sending only. A wallet whose lease and grace have ended
  by its effective time does not pay until it renews. It still receives and
  still unloads. §5.11 says what a renewal carries.
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
| 16 | P's files are put back to a copy from 30 September, before it spent L | P resumes at its marker (§5.10). Its counters and its floor are those of 1 October | L, as if nothing had been restored |

Month windows behave the same way with the month of the signed time.

**Who can verify what**

| Rule | Checked by | Input | Holds against |
|---|---|---|---|
| The signed time is at or after the receiver's time, and at most `W` after it | the receiver | the SendSplit and the receiver's own Request | any payer |
| The lease, with its grace, has not ended at the signed time | the receiver | the SendSplit and the issuer-signed certificate | any payer, where the receiver's clock is right |
| The payer's counters are within the limits | the payer's unmodified app; the receiver reads the declared counters | the payer's marker and journal; the SendSplit | an ordinary user. Not a compromised payer, whose counters are its own statement |
| One payer does not exceed its limit at one receiver | the receiver | the receiver's own journal | any payer, per receiver, and between two resumes of that receiver |
| The receiver's time is not far ahead of the payer's | the payer's unmodified app | the Request and the payer's own clock | protects the payer; it is not a check on the payer |
| A wallet does not sign below its floor | the wallet's unmodified app; the issuer at sync; anyone holding two transitions (§5.3) | the marker and the journal | an ordinary user; a compromised phone only where both transitions are seen |
| A new certificate does not reset an allowance | the issuer | uploaded journals or the resume record, the registry binding, its own records | an ordinary user |
| One account's devices together stay within the account's limit | the issuer | the registry binding and its own records | an ordinary user. Not limits across accounts |

**What in this section can require connectivity.** Each row is a regulatory
control, and each applies only where the scheme has switched it on in the
tier row and the certificate. None touches value already received. With all
of them off, nothing in this section requires connectivity at any time.

| Rule | Control | What stops | What ends it |
|---|---|---|---|
| Lease expiry | R8 | Sending, once the lease and its grace have ended | A renewal (§5.11) |
| `require_anchor` | R8, reboot policy | Sending, after a reboot | Any direct issuer exchange |
| `receive_not_after` | R6, receive freshness (§5.5) | Requesting, once it has passed | A renewal |
| Clock-reset state with the floor ahead of real time | R7 or R8, whichever the certificate carries | Sending under that certificate, and being paid by payers whose certificates carry a time-dependent control | Real time coming within the tolerance of the floor, with no network; or a sync |
| A tier-row notice that brings in or shortens a lease (§5.11) | R8 with new values | Sending, once the new lease has ended | A renewal |

The owner named a blacklist, daily or monthly limits and an expiry for
attestation. `require_anchor`, receive freshness and the per-counterparty cap
are this document's additions under those names. Each counts as an explicitly
enabled regulatory control only if the owner confirms it (§12).

Limits (R7) delay sending to the next window and never require a sync. A
failed tolerance check requires none; it is cleared by setting the date. A
wallet under a certificate with no time-dependent control is never sent
online by anything in this section. Outside this section, a key revocation
(§5.1) and a new rules version (§5.11) stop no wallet and require no sync.

**Stated limit.** A phone's clock is expected to keep running while it is
powered off, so that an ordinary reboot leaves it as accurate as its setting;
no primary source was found, and the long power-off test must confirm it per
device. The clock is lost when the battery is exhausted or removed, and the
restored value is platform-specific (Android: never earlier than the system
build date; iOS: undocumented). Two unmodified wallets whose restored clocks
both read behind real time, above their own floors and within `W` of each
other, accept each other's expired certificates with no one having changed a
clock. The error is real time minus the receiver's own time, and it has no
upper limit. Certificate expiry therefore bounds sending only among receivers
whose clocks are right. The same holds for the lease as the age limit of the
evidence in E (§5.11).

Not enforceable offline, and against whom:

- R7 and R8 against a payer and a receiver whose clocks are both wrong in the
  same direction, by accident or because two users set them. Neither wallet
  signs below its own floor, so the pair needs a receiving wallet that has not
  signed or synced since the date they choose. Value that then reaches a
  receiver whose clock is right is judged at that receiver's time and under
  the sender's own limits. Under `require_anchor` the payer's side of this
  needs a modified app.
- Any limit against a compromised payer beyond each receiver's own tally.
  Nothing limits how many receivers it reaches (§3.2).
- Expiry against a compromised payer that keeps renewing (§3.2).
- Receive freshness against a payer whose own time is behind the receiver's
  `receive_not_after`, and against a modified payer.
- Limits across accounts not bound to one verified identity.
- The anchor flag and the clock-reset mark against a modified wallet. Both
  are self-declared.

Required tests, per platform. They stay in §10.4 beside the evidence gate;
none has been run. Long power-off; repeated restarts; battery exhaustion,
recording the restored clock; a reset that lands above the floor; reboot on
each side of a payment and with an open Request; both sides unanchored; app
killed and relaunched within one boot; long device sleep while anchored;
post-reboot uptime above the stored reading; Android boot count rewritten
over adb; iOS clock set within one boot; time-zone change; backward clock
steps of a minute, 14 hours and 30 days; a fold signed with the date ahead,
then corrected; re-anchor below the last signed time; replayed and
peer-relayed issuer time; a `Never` and `Unlimited` certificate with no cap
sends and requests throughout, with its Requests marked only in the
clock-reset state. For the effective time and the resume:

- each row of the worked-cases table, with the signed time, the window and the
  floor recorded after each step;
- a Request dated ahead by just under and just over `W`, with the payer's
  floor checked after each;
- twelve payments in a row to receivers each dated `W` ahead of the payer's
  time, to show that rule 2 keeps the floor within `W` of the clock reading;
- a payment just before and just after UTC midnight and a month end, with the
  payer's counters and the receiver's tally recorded;
- opening counters after a renewal, after a Recertify with the floor ahead of
  issuer time, after a renewal that carried a resume record, after a Migrate,
  and after re-enrollment following a declared loss;
- a payer's files put back to an earlier copy inside one window: the counters
  and the floor after the resume equal those before the restore;
- a receiver's files put back inside one window: record what the tally holds
  after the resume, under each of the two forms above;
- the date check: a wallet idle for 31 days and unanchored prompts once and
  then pays;
- under `require_anchor` on iPhone, how often the shell reports a reboot when
  none happened, over a week of normal use.

### 5.5 Block list (R6)

The owner asked for "a blacklist of accounts that users that have the
blacklist won't send to". That is the flag `receive_blocked`. Three things in
this section go beyond those words and are this document's additions: the
second flag `send_blocked`, under which holders of the list refuse payments
from a device; receive freshness; and holder-requested blocking. Each counts
as part of R6 only if the owner confirms it (§12). R6 as a whole applies only
where the scheme has switched it on. With R6 off no list is distributed and
nothing in this section acts on any wallet.

- **Source of truth.** A new consensus index per asset, account →
  `{send_blocked, receive_blocked}`, with a change counter. The offline list
  is derived from it in both directions. A phone holding a stale list is
  looser than the ledger until it refreshes.
- **Entries.** Consensus derives every entry. The issuer transcribes that set
  and adds nothing of its own. There are three sources.
  - A blocked account. Consensus expands it into one entry per device id
    through the registry: `(device id, flags, dead_through_serial)`.
  - A hold (§5.3, §8.2). A hold needs two conflicting signatures by one
    device key, verified on-chain. It acts on that key's row, or, where that
    row was retired by Migrate, on the row that took over its balance.
    Consensus emits an entry for the held device id with both flags and the
    highest serial. No party places a hold on its own judgment, and evidence
    of a fault in an issuer-side key places none on the phone that holds the
    object. Under T1 to T3 no unmodified wallet signs two successors of one
    state, so this entry reaches only a device on which an assumption failed,
    or the successor of one. It is containment after a failure (§3.2), not
    one of the owner's controls.
  - A holder's request, where the scheme has switched that on (below).

  An entry covers every certificate of its device id with a serial at or
  below `dead_through_serial`. Entries are merge-only: the higher serial and
  the union of flags win.
- **Serial rule.** `dead_through_serial` is the last certificate serial
  recorded in the device's registry row when the block is made. The issuer
  has each new serial recorded in the registry row before it releases the
  certificate, and the registry refuses to record a serial while the account
  carries any flag. Checked by consensus, from the block index and the
  registry. This is so that an entry covers every certificate that exists
  when the block is made, and no certificate can be issued above the entry
  while the block lasts. A renewal therefore waits for one ledger write to be
  final. If the issuer stops between that write and the release, the wallet
  asks again and the issuer releases the same serial or has the next one
  recorded; a gap in serials is harmless.
- **What a serial above the entry shows.** A certificate is accepted only
  with a receipt that names it, and a witness signs a receipt only for a
  certificate the ledger has recorded (§5.1). So a certificate with a serial
  above `dead_through_serial` and a complete receipt shows a holder of the
  entry that the ledger recorded that serial after the flags were cleared. A
  holder of the entry needs no further object to see that the block was
  lifted.
- **Entries and revoked keys.** One case needs stating: a device was blocked,
  the ledger cleared the flags, the device renewed above the entry, and a key
  that signed the new certificate or its receipt is revoked later. A receiver
  that holds the old entry and the revocation compares serials as always and
  accepts the device, for as long as §5.1 lets the certificate stand. No sync
  is needed.
  - A thief of the certificate key alone cannot lift an entry. It can sign a
    higher serial, but no receipt names that certificate, so no receiver
    accepts it.
  - A thief who holds the certificate key and the witness quorum can sign a
    higher serial with a receipt, and so step over an entry, for as long as
    §5.1 lets its objects stand. Treating an entry as covering every serial
    under revoked keys would not stop that thief: it can certify a new device
    key for the same person, and an entry binds a device id. It would stop
    the honest device described above until it synced, for no regulatory
    reason. So the design does not do it. Against a thief of both keys R6
    holds only as far as §5.1 bounds what the thief signed; §3.2 lists this
    with the other consequences of a stolen key.
- **What holders of an entry do.**
  - A payer refuses a receiver whose certificate is covered by a
    `receive_blocked` entry. Nothing is signed.
  - A receiver answers `Refused` to a payer whose certificate is covered by a
    `send_blocked` entry, and attaches the list segment that holds the entry.
    The payer refunds when it has the Outcome, and its wallet shows the
    payment as returned.
  - A wallet that holds an entry covering its own certificate creates no
    Request if the flag is `receive_blocked` and does not pay if it is
    `send_blocked`.

  Who can enforce this: the unmodified app of whoever holds the entry, with
  the signed list segment and the counterparty's certificate as input. A
  modified payer pays whom it likes.
- **Value already received.** An entry acts only on payments not yet made. A
  payment that the receiver's wallet has reported complete is the
  receiver's. No entry, from any of the three sources, causes the receiver's
  wallet, the issuer or the ledger to reduce, refuse or delay that value,
  whatever happened to the payer afterwards. The receiver pays it onward, and
  the next receiver's checks see nothing of the earlier payer. A renewal of
  the receiver is not refused because of who paid it (§5.11). An entry also
  changes no balance on the blocked device. What the ledger does with a held
  or retired row is in §8.2.
- **Issuer and ledger.** The issuer refuses enrollment, renewal, load ids and
  vouchers for a blocked account. The registry refuses the registration of a
  new device for one, and the recording of a serial as above. Whether a
  blocked account may unload is the ledger's rule for that account; the
  offline list does not decide it.
- **Lifting.** An entry from a blocked account is lifted only by a renewal
  above its serial, after the ledger has cleared every flag on the account.
  The flags of an entry lift together: clearing one of two flags on the
  ledger has no effect offline until both are cleared. An entry from a hold
  uses the highest serial and is final for that device id; what the holder of
  a held row gets back, and on which device id, is in §8.2 and §3.2.
- **Freshness.** A payer is never sent online to refresh its list. Freshness
  comes from the receiver's side: with `receive_not_after` set in the tier, a
  blocked account cannot renew, its certificate's `receive_not_after` passes,
  and payers refuse it by their own time (§5.4). The longest a block can go
  unenforced among unmodified payers whose clocks are right is then the
  length of the receive lease. The price is that every receiver must renew
  within that period to keep requesting; this is the sync that receive
  freshness brings, and it exists only where the tier sets
  `receive_not_after`. With it unset, R6 holds only as of each payer's last
  list, and nothing sends anyone online.
- **Holder-requested blocking.** Off unless the scheme switches it on in the
  descriptor.
  - Where it is off, the retirement of a device id by its bound account
    (§7.2), for a phone declared lost, has no effect offline. The registry
    row takes no further load, and the phone, if it still works, pays and is
    paid as before.
  - Where it is on, the bound account can also block its own device id with
    `RequestKagemushaDeviceBlock` (§6, §7.2). Retirement itself still emits
    no entry. Consensus emits an entry for that device id with both flags
    and the last serial recorded, and the registry records no further serial
    for it while the request stands. The entry is lifted as any other is:
    the account clears its request, and the device renews above the entry's
    serial. If the phone turns up, it can still unload (§8.2).
  - What it gives: a thief who holds the phone can no longer pay the balance
    to holders of the list.
  - What it costs: whoever holds the account key can stop a working phone
    paying and being paid among holders of the list. It takes no value. The
    phone can still unload, and an unload pays the bound account (§8.2).
  - It is not in the owner's words. Whether a scheme may switch it on, and
    whether it then counts as an explicitly enabled regulatory control, is
    the owner's decision (§12).
- **After a resume.** The block list is kept in the wallet's files. The
  marker names the list version the wallet held at its last commit (§5.10). A
  wallet that resumes with older files or none does not hold that version. It
  neither pays nor creates a Request until it holds that version or a later
  one again, from a peer or from the issuer. The rule is there because the
  alternative lets an ordinary user shed block entries by deleting the
  wallet's files. It is R6 that stops the wallet, and only where R6 is on. A
  peer message has room for at most about 200 entries, so a wallet catches up
  on a long list only at a sync. Keeping the list in the key store beside the
  marker removes this stop, at about 0.1 KB plus 40 B per entry of key-store
  data per segment. Owner decision (§12).
- **Distribution.** A separate list-signing key signs the list in segments.
  Each segment carries the scheme id, `list_epoch`, a counter, the issuer
  time at which it was made, and its entries. Lists are ordered by
  `(list_epoch, counter)`. A wallet receives the whole list from the issuer at
  a sync and may merge any authentic segment a peer hands it; merging can
  only tighten. The issuer time in a segment raises "last issuer time"
  (§5.4). The list is permanent and grows by about 40 B per entry (estimate):
  10,000 entries are about 0.4 MB on each phone.
- **List key compromise.** A stolen list key creates no value. It can block
  honest devices among wallets that receive its segments, until those wallets
  hold the correction. That happens only after an issuer-side key has been
  stolen (T4), and §3.2 lists it. The correction travels peer to peer: the
  root raises `list_epoch` and certifies a new list key in one notice, and
  the first segment of the new epoch carries every entry still in force. A
  wallet keeps applying the old epoch's entries until it holds that first
  segment and then drops them. Merge-only entries stop a forged segment
  lifting a block.

**What in this section can require connectivity.** Each is R6, and none
exists where R6 is off.

- An entry that covers the wallet's own certificate. The wallet stops paying
  or requesting, by the flag, until the ledger clears the account and the
  wallet renews above the entry. An entry from a holder's request stands
  until the account clears the request and the wallet renews above it.
- Receive freshness, where the tier sets `receive_not_after`: requesting
  stops when it passes, until a renewal.
- After a resume, the missing block list: paying and requesting stop until
  the wallet holds the list version its marker names, from a peer or at a
  sync.

An entry from a hold also stops its device among holders of the list. It
arises only after an assumption has failed on that device or on the device
it took its balance from (§3.2).

Outside what any list can stop: a new account on the same phone or another;
after a hold, a new device key under the same account; and receiving through
an accomplice. A block entry binds a device id, not a phone and not a person.

### 5.6 Transport

The three messages are transport-neutral. Nothing in §5.2 depends on the
carrier. A carrier moves bytes between two phones. It adds no check and removes
none: the wallet that receives a message applies §5.2 to the bytes it received,
whatever carried them. Nothing in this section has been measured on a phone.
Every size is computed from the estimates of §5.1, and no encoder exists.

- **Three transfers.** The Request goes from the receiver to the payer, the
  Payment from the payer to the receiver, the Outcome from the receiver to the
  payer. The receiving wallet reports a payment complete after its checks, its
  own proof, its durable commit and its confirmed marker step (PC, §5.2).
  That does not depend on the Outcome reaching the payer. The paying wallet
  shows "sent, not confirmed" until it has stored a `Credited` Outcome. A
  `Refused` payment is refunded only if the Outcome reaches the payer. A
  Request is single-use, so it cannot be printed.
- **Interrupted transfers.** The payer's wallet commits the SendSplit and
  confirms its marker step before it releases the Payment (§5.2). If the
  carrier then fails, the payer is debited and the receiver holds nothing.
  That payment is not complete, and neither wallet shows it as complete. The
  payer's wallet presents the same Payment again: from its files while they
  are current, and after a resume for the newest payments, which its marker
  holds in full (§5.10 states the cap and what is lost beyond it). Presenting
  a Payment twice is safe: the receiver stores one Outcome per payment id and
  returns it unchanged (§5.2). No carrier makes the exchange atomic. §5.2
  says what an honest payer can lose and when.
- **What crosses.** Every Request and every Payment carries its sender's
  certificate, receipt and enrollment statement E (§5.1). The Payment also
  carries the signed SendSplit and the proof (§2.1). This section takes E as
  about 0.14 KB, the figure §5.1 gives for an Android phone.

  | Message | Without E and proof | As sent (§5.1) | Repo limit for the V1 profile |
  |---|---|---|---|
  | Request | 0.76 KB | about 0.90 KB | 1,024 B |
  | Payment | 1.0 KB | about 7.7 KB: 1.14 KB plus a proof at the repo's 6,528 B ceiling | 7,552 B, of which 6,528 B proof |
  | Outcome | 0.2 KB | about 0.2 KB | 256 B |
  | Whole exchange | 1.96 KB | about 8.8 KB | 9,211 B |

  The limits are those of `specs/peer_transport_v1.md:65-71`. The estimate for
  the Payment is at or just above that profile's limit. It also assumes that
  per-hop verification does not enlarge the proof that travels, which is not
  established (§2.2). The limits of the new profile are among the targets Q0
  fixes (§2.3). All figures are for one witness and nothing optional attached.
- **QR is the baseline.** Of the carriers the repo implements, QR is the only
  one that works between every pair of target phones. It needs a screen and a
  camera on each phone and no entitlement from the platform. An exchange is
  three scans: the payer scans the Request, the receiver scans the Payment,
  the payer scans the Outcome. Aiming the camera selects the peer.
- **Size on QR.** One QR symbol holds at most 2,953 bytes (version 40, lowest
  error correction). The repo's existing framing shows a still code only up to
  about 0.34 KB of payload. Above that it animates 256-byte frames with one
  parity frame per two, and a header frame at the start and after every twelve
  others
  (`IrohaSwift/Sources/IrohaSwift/IrohaPeerQRV1.swift:198-199, 267-293`). Under
  that framing a 0.85 KB Request is about 7 frames, a 7.6 KB Payment about 49
  to 51, and a 0.2 KB Outcome is one still code. The repo's widget defaults to
  5 frames per second
  (`IrohaSwift/Sources/IrohaSwiftTransferUI/KagemushaWidgets.swift:281`). These
  figures are computed. No scan was timed. How many passes a scan needs, how
  long the receiver takes to aim at the payer's screen, and how dense a still
  code a phone reads from another phone's screen are not measured.
- **Time on QR.** The owner said: "1-2 s should be good ux". At 12 to 5 frames
  per second one pass of the Request takes 0.6 to 1.4 s, and one pass of the
  Payment takes 4 to 10 s. So on the repo's QR framing the transfer of a
  proof-carrying Payment alone is outside one to two seconds, before any
  signature, marker step or proof. A proof-carrying payment can come near that
  figure only on a faster carrier (NFC where the pair allows it, or a radio
  connection), with a denser QR framing that is untested, or with a smaller
  proof. Group g of §10.4 measures the carriers. §2.3 has the proving times,
  which also fall inside the payment.
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
  - It is not measured. Connection setup time and transfer time for 0.85 KB
    and 7.6 KB are unknown on every pair. That an iPhone and an Android phone
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
  `specs/peer_transport_v1.md:83-96` that matches neither byte layout. This
  design reuses one envelope and one QR framing under a new profile code; the
  normative spec says which. The envelope registers one profile today, so a
  message of this design is refused until its profile is added. No carrier has
  a recorded device measurement.
- **Bystanders.** QR and NFC are in the clear, and so is a Bluetooth LE
  connection unless the wallet encrypts. Of the existing carriers only Nearby
  encrypts. Anyone who films the codes reads both device ids, both enrollment
  statements, the amount and the payer's totals (§5.7). Encrypting the Payment
  and the Outcome to a key carried in the Request costs about 100 bytes
  (estimate). The Request itself cannot be hidden from someone who can see it.
  Owner decision (§12).
- **R9 has no unit.** The owner said: "to do device to device transfers, we
  really cannot have payment data exceed around 10k or so". This document
  reads that as: the Payment is at most 10,000 bytes in canonical binary form,
  before framing. The owner confirms the unit (§12). Two consequences of the
  proof's size follow.
  - A notice or list segment carried in a peer message counts against that
    message's bound. A Payment of about 7.6 KB leaves about 2.4 KB: room for
    about 55 block-list entries at 40 B each, where a Payment with no proof
    would have room for about 200. A Request or an Outcome has more room.
  - If the owner means the text form that a code shows, the Payment is already
    at the bound. The repo's text form of a 7,552 B payment is 10,075 bytes
    (`specs/peer_transport_v1.md:65-71`); 7.6 KB becomes about 10.2 KB.

  No carrier above has a hard limit near these sizes. The bound is a budget
  for the time two phones are held together.

Which carriers a wallet must support is an owner decision (§12). The
measurements that decide it are group g of §10.4.

### 5.7 What the exchange reveals

Privacy is not among R1–R9. The design is pseudonymous, not anonymous. This
section lists what each party can learn. Nothing limits what a party keeps: a
wallet cannot make another phone, the issuer or the chain forget.

- **The pseudonym.** A device id is a stable pseudonym. It changes only when
  the wallet enrolls a new key: on Migrate, or after a declared loss (§7.2).
  The registry records the succession on Migrate only. After a declared loss
  the old and new ids are linked only through the account each is bound to.
- **Each side learns, from the certificate and receipt** that every Request
  and Payment carries: the other's device key, tier, limits, certificate
  dates, reboot policy, issuer key and registration height.
- **Each side learns, from E,** facts about the other's phone as of its
  enrollment or its last renewal (§5.1, §7.1):
  - the platform class: Android under a remotely provisioned attestation
    chain, Android under a factory-provisioned chain, iPhone, or a class with
    no operating-system statement;
  - on Android: the security level (TEE or StrongBox); that the bootloader was
    locked and the boot verified; the digest of the verified-boot key, which
    is the same for every phone that boots that manufacturer's images; the OS,
    vendor and boot patch levels; and the app's package, version and signing
    digest as the operating system reported them;
  - on iPhone: the App ID, the production environment, and from iOS 27 the
    launch category and the bundle version. No OS version, patch level or
    model is in it, because Apple's attestation carries none;
  - the policy entry the enrollment was judged under, a commitment to the
    attestation certificates' serial numbers, and the enrollment epoch.

  So a receiver that keeps what it sees can tell which payers run phones whose
  patch levels were old at their last renewal, and roughly when each phone
  last went online to renew. With R8 off a phone never renews, and E then
  shows the day it enrolled.
- **The payer also learns**, from the Request, the receiver's clock reading
  (`created_at_ms`), its anchor flag, whether it is in the clock-reset state
  (§5.4), and which descriptor and list epochs it holds.
- **The receiver also learns**, from the SendSplit, the payer's sequence
  number, its lifetime gross outflow, refunds and fees, its gross sent today
  and this month, the fee and the `fee_policy_id` it was computed under
  (§5.8), and the time the payer signed. That time is `t_eff` (§5.4), which
  may be the receiver's own `created_at_ms`. The receiver does not learn the
  balance. No peer-visible object carries one, and the proof covers the
  payer's state after this SendSplit, so no balance has to be shown.
- **What the proof shows about earlier holders.** That every earlier hop
  satisfied the relation of §2.1, and one policy head. The keys, enrollment
  statements and transitions of earlier holders are private inputs of earlier
  proofs. As the relation is specified, the receiver learns who paid it and
  nothing that identifies anyone before that. That the proofs in fact hide
  their private inputs is a property for Q3 to review; it has not been
  checked.
- **Two payments from one payer**, seen by one receiver or by two who compare,
  show how much the payer sent and how many operations it made in between.
- **The issuer** receives, at enrollment, the raw vendor attestation for the
  device key: on Android the certificate chain with every field of the key
  description, among them the digest of the booted images, which identifies
  the exact build, and the brand and model where the phone attests them; on
  iPhone Apple's attestation object and receipt, which Apple describes as
  carrying no hardware identifiers. At each renewal on Android it receives a
  fresh leaf with the patch levels of that day. It sees every transition of a
  wallet that syncs, and so both sides of each of its payments. After a resume
  with a gap it sees only the resume record: the last transition and the
  totals, not the commits in between (§5.10). A wallet that never syncs shows
  the issuer nothing after enrollment. §7.1 lists what can make a wallet sync.
- **The vendors.** Apple's server takes part in every iPhone enrollment,
  because the attestation call reaches it. It does not take part in a payment
  or a renewal. No Google server takes part in an Android enrollment beyond
  the key provisioning the phone does for itself; the issuer and the
  validators read Google's public revocation list.
- **The chain** holds a pooled reserve. It never executes an individual
  offline payment and never reverses one. What it holds about wallets is this:
  - the registry row that binds a device id to an account, with E;
  - the raw vendor attestation of each device key. The validators verify it
    themselves at registration (§8.1), so the registration carries it, and
    anyone who reads the chain reads what the issuer reads: the patch levels,
    the build digest, the security level, the app version, and the brand and
    model where attested. Where R8 is on the same holds for each renewal, so
    the chain shows how each device's patch level moves over time. A scheme
    that kept only a digest on the chain would hide this, and the validators
    could then not verify the attestation themselves. That trade is the
    owner's (§12);
  - each load, and each unload as a whole RedeemSplit;
  - whole transitions wherever evidence is submitted;
  - where the issuer anchors acknowledged heads and renewal serials (§5.1,
    §7.2), the time of each sync and renewal of each device id;
  - where a fee schedule applies, a record of every payment whose fee is
    settled: the SendSplit and the receiver's credit, so the payer, the
    receiver, the amount and the payer's totals (§5.8). That record pays the
    fee. It does not move, confirm or undo the payment.
- **Bystanders.** §5.6 says what someone near the two phones can read.
- **Why the fields are there.** The fields a peer sees are what the
  receiver's checks on the last hop (§5.2) and the evidence rules (§5.3) test:
  the lease, the limits and the receiver's own tally, the block list, and
  the payer's E against the policy entry it names. Hiding them removes those
  checks.
  Recursive V1 specified a payment that showed the receiver no payer
  credential (`specs/kagemusha_v1.md:36-40, 240-251`). This design gives that
  up for the last hop and keeps it for every earlier hop.

### 5.8 Optional fees

The owner said: "optional fees sound good, but can only be received to an
online account when someone syncs." The rules below are this document's
reading of how to do that. They are untested. Two rules hold throughout. A
fee never makes a received payment less final. A fee never adds a step to a
payment or delays one: the two phones compute and check it during the
exchange, and everything else about it happens later, on the ledger, between
the pool and the beneficiary.

- **Fee policy.** A fee policy is a root-signed record: a rate in parts per
  million, a fixed part, a ceiling, and one beneficiary, which is an online
  account. The fee on an amount is
  `min(ceiling, fixed + floor(amount × rate / 1,000,000))`. The formula is
  fixed so that the payer, the receiver and the ledger compute the same
  number. `fee_policy_id` is the digest of the record. The ledger keeps
  every record ever installed, keyed by its id, and never changes a record's
  rate, fixed part or ceiling (§6). A scheme with no record has no fees.
- **Which policy a wallet pays under.** The issuer writes one fee policy, or
  none, into each device certificate: `fee_policy_id` and the three numbers.
  A wallet pays under the policy in the certificate it holds. A new policy
  reaches a wallet at its next renewal and not before. With R8 off a wallet
  may keep its first policy for as long as it lives. The rule is there so
  that a fee change never stops an offline wallet paying and never sends one
  online: a fee is not a regulatory control.
- **The payment.** The SendSplit carries `fee` and `fee_policy_id`. Both are
  in the signed preimage, so the payment binds the policy it was computed
  under. The payer's balance falls by the amount plus the fee, and the
  payer's app shows both before the user confirms. The receiver is credited
  the amount. Nobody holds the fee offline. It leaves offline circulation
  when the SendSplit commits. It becomes a claim of the beneficiary on the
  pooled reserve when the receiver credits the payment.
- **The receiver's check.** Before it commits, the receiver checks that
  `fee_policy_id` equals the id in the payer's certificate, which the Payment
  carries, and that `fee` equals the formula applied to the numbers in that
  certificate. If the certificate has no fee policy, the SendSplit must carry
  no fee. Otherwise the receiver answers `Refused`. The receiver uses the
  payer's certificate and not its own copy of the scheme descriptor, because
  the two phones may hold different descriptor epochs. This is the only
  offline check. A modified payer and a receiver who agrees with it can leave
  the fee out.
- **A fee never makes a received payment less final.** The receiver's credit
  is the full amount. It is the receiver's when the receiver's wallet reports
  the payment complete (§5.2), and the fee is no part of that report.
  Settlement happens later. No result of settlement debits, holds or delays a
  wallet, a renewal, or a registry row's claims. A fee that fails a ledger
  check is not paid, and nothing else follows from the failure.
- **Refusal returns the fee.** A RefundFold returns the amount plus the fee,
  and a refused payment pays no fee. The ledger therefore pays a fee only
  when the settlement also carries the receiver's credit: its signed
  ReceiveFold or `Credited` Outcome for that payment id. A SendSplit alone
  pays nothing, because the chain cannot tell whether it was refused. If the
  payer never took the Outcome, the fee waits for the receiver's sync. A fee
  paid for a payment that was also refunded needs a credit and a `Refused`
  Outcome from one receiver, which is evidence under §5.3. The alternative, a
  fee kept on a refused payment, charges an honest payer for a payment that
  did not happen.
- **A payment that did not complete.** If the Payment never reaches the
  receiver, or a `Refused` Outcome never reaches the payer, the payer's
  wallet shows "sent, not confirmed" and counts the amount and the fee as
  spent until the two phones meet again (§5.2). No credit exists, so no fee
  is paid on it.
- **Counters and evidence.** Every transition carries a cumulative
  `cum_fee_after`. Let Φ(t) be the fee of a SendSplit and zero for any other
  kind. The rules of §5.3 that name `cum_out_after` and Δ apply in the same
  way to `cum_fee_after` and Φ. `cum_fee_after` is gross: a refunded fee stays
  in it.
- **Limits.** `max_payment`, the day and month counters, `cum_out_after`,
  `cum_refund_after` and the receiver's tally count the amount only. The fee
  is outside them. The payer's balance must cover the amount plus the fee.
- **Under a proof.** The fee and `fee_policy_id` are in the transition
  preimage, and the relation debits the amount plus the fee. The shape of the
  formula is fixed with the relation (§2.3, Q0).
- **Settlement.** The signed SendSplit and the receiver's credit reach the
  issuer when the payer or the receiver syncs. Either side may bring both: a
  receiver holds the SendSplit inside the Payment it stored, and a payer
  holds the `Credited` Outcome once it has taken it. The issuer may also join
  a SendSplit from the payer's sync with a credit from the receiver's. The
  issuer submits `SettleKagemushaFee` (§6) carrying both. The chain checks,
  with these inputs:
  - the SendSplit's signature, against the device key in the payer's registry
    row;
  - the ReceiveFold's or `Credited` Outcome's signature, against the device
    key in the registry row of the counterparty the SendSplit names, and that
    it names this payment id;
  - that `fee_policy_id` is a record in the ledger's table, and that `fee`
    equals the formula under that record;
  - that neither row is under a hold. A hold is on a row whose own key signed
    two successors of one state, or on the row that took over such a key's
    balance by Migrate;
  - that this payment's fee has not been paid before.

  It then pays `fee` from the pooled reserve to the record's beneficiary. The
  check is against the record the SendSplit names, however late the
  settlement arrives and whatever the tier table says by then. The chain does
  not see the payer's certificate, so it does not check that the policy named
  is the one the issuer gave that payer. The receiver's check above is the
  only place that is tested. If the numbers in a certificate differ from the
  record its id names, the ledger's check fails and the fee is not paid; that
  is the issuer's error and costs no holder anything.
- **Once per payment.** The chain keeps, for each payer row, the set of
  sequence numbers whose fee it has paid, stored as ranges. A device signs one
  SendSplit per sequence number, so the pair of device id and sequence number
  identifies the payment. A SendSplit at a sequence number already in the set
  is refused. The set is never pruned, because nobody has to sync and a
  SendSplit can arrive after any delay; it stays when the row is retired. It
  costs one range per gap, and one entry per fee at worst. A counter cannot
  replace it, because SendSplits arrive in any order. The row also holds the
  total of fees paid for it.
- **After a resume.** A wallet that resumed (§5.10) no longer holds the
  commits inside the gap. Its resume record carries `cum_fee_after` and no
  SendSplit from the gap.
  - A payer's gap. Each fee in it is settled from the receiver's copy, when
    that receiver syncs: the receiver stored the Payment with its ReceiveFold.
  - A receiver's gap. Each fee in it is settled from the payer's copy, if the
    payer took the `Credited` Outcome and syncs.
  - Both copies gone, or the only holder never syncs. The fee is not paid.
  - The issuer does not settle from `cum_fee_after`. That total is gross: it
    includes fees that were refunded, and it names no receiver's credit. A
    payout from it would pay fees on refused payments.

  The cost of a gap is the beneficiary's: a fee it never receives. No holder
  pays twice, loses anything, or waits.
- **Order and rate of payout.** A fee is never paid ahead of an amount a
  holder is waiting for: while any payout to a holder is waiting (§8.2,
  §8.4), the ledger pays no fee. Fee payouts for one payer row are capped per
  unload window by a scheme parameter, `fee_limit`. It bounds what a
  compromised phone can route to a beneficiary that colludes with it, and it
  delays only the beneficiary. Its value, including no limit, is the owner's
  (§12). A settlement that cannot be paid for either reason is refused,
  records nothing, and can be submitted again later. The cap is per payer
  row, so one row's fees do not delay another's.
- **Beneficiary gone.** A fee is paid to the account in the record. If that
  account can no longer receive, the root may name a replacement payout
  account for that record. It cannot change the record's numbers. Until it
  does, settlements for that record are refused and can be submitted later.
- A payment whose record never reaches the issuer pays no fee. Nobody has to
  sync.
- **What the chain then sees.** Each settled fee puts a whole SendSplit and
  the receiver's credit on-chain: both device ids, the amount, the device time
  and the payer's totals. With a fee policy in use the chain sees every
  payment whose fee is settled, not loads and unloads only (§5.7, §14). Each
  one costs the chain two device-signature checks. `SettleKagemushaFee` is
  accepted only from the issuer's registry authority (§6). That restriction
  is for privacy, not for money: the objects prove themselves, and anyone who
  filmed the codes of a payment holds them.
- **A compromised phone.** It can sign payments that never happened, each with
  the highest fee any record allows, to a second device it controls. That
  gains the attacker nothing unless a beneficiary colludes. `fee_limit`
  bounds it per payer row per window, and a hold on either row stops it. A
  fee on counterfeit value has no load behind it, like the value itself;
  §3.2 says what follows from value of that kind.
- **Pool accounting.** A fee paid leaves the pool like a redemption, and a
  fee not yet settled is a claim on it. §8.3 carries both in its figures.
- **Size**, estimated and not measured: about 65 bytes on a SendSplit (`fee`,
  `fee_policy_id`, `cum_fee_after`), about 16 bytes on any other transition,
  and about 70 bytes on a certificate that holds a fee policy. A Request and a
  Payment each carry one certificate.
- **Open** (§12): who pays, the payer on top of the amount as drafted here, or
  the receiver out of it; one beneficiary per scheme or one per issuer; taxes.
  If the receiver pays, the fee must be taken off the credit before the
  ReceiveFold commits and never afterwards. One more choice is open. The
  issuer could settle fees in aggregate, one instruction per beneficiary per
  period, with no SendSplit on-chain. The chain then sees no payment and
  keeps no per-payment set, and the fee total rests on the issuer's word
  instead of on device signatures.

### 5.9 User authentication

The owner said: "pin/biometric is a ux functionality but generally it should
be related to the secure hardware". The owner named no mechanism. The design
below is this document's reading. Nothing in this section is device-tested.

**The design.** The device key carries no authentication requirement. Before
the app asks the key to sign a payment, it shows the platform's
authentication prompt and continues only on success: Android
`BiometricPrompt` with the device credential allowed, iPhone `LAContext`
with the device-owner policy. The platform checks the PIN or biometric in its
secure hardware and tells the app the result. The key would sign without it.

How this relates the PIN or biometric to the secure hardware: the hardware
verifies it. It does not make the hardware refuse to sign. Whether that meets
the owner's words is for the owner to judge (§12).

**Why the key is not bound to authentication.** Both platforms can make a key
that the secure hardware refuses to use until the user has authenticated. The
design does not use one, for the device key or for the marker.

- On Android the platform invalidates such a key for good when the secure
  lock screen is removed or reset, and a key that needs a biometric at every
  use when biometric enrollment changes (documented). A device administrator
  can reset the lock. The balance is held by that one key, so an ordinary
  settings change would destroy it. That fails P1 while every assumption
  holds.
- On iPhone the binding is an access-control flag that nothing attests, so no
  verifier could see it. What a passcode change does to such a key is not
  verified.
- A bound key cannot sign with the user absent. Renewal and receiving would
  then need the user present and authenticated.

**What it gives and what it does not.**

| Case | Result |
|---|---|
| Thief holding the phone locked | Cannot pay. The app cannot be opened |
| Thief holding the phone unlocked, without the PIN or biometric, phone not compromised | Cannot pay, except inside a window the tier left open |
| Thief without the PIN or biometric who compromises the phone | Can pay. Modified code skips the prompt |
| Thief who knows the PIN | Can pay |
| Double spend by the holder | No effect. The holder authenticates willingly, and authentication does not bind what is signed |
| What the issuer or a receiver can verify | Nothing. The prompt is the app's own rule, like every rule that rests on T3 |
| Screen lock removed or reset; a biometric enrolled or removed | The device key is unaffected. On Android the marker is unaffected, where the form of entry §5.10 chooses holds on that phone. On iPhone removing or resetting the passcode discards the marker (last rule below) |
| Signing with the user absent (a renewal in the background, an unattended receiver) | Possible. On iPhone only while the phone is unlocked, because the marker and the payment key cannot be read on a locked phone (§5.10) |
| Receiver prompted during a payment | No |
| Changing the mode | A tier field. The app reads the new value at renewal |

Authentication is a confirmation step and a theft control. It is not a
double-spend control. A thief who can authenticate spends the balance, and so
does a thief who compromises the phone. P1 does not cover a stolen phone in
those two cases: the holder no longer keeps the wallet (T6).

**Rules.**

- The mode is a tier field: none, a window in seconds, or each payment.
  Recommended: a short window. The app keeps the window. "None" is for an
  unattended payer.
- The app prompts before it signs a SendSplit, a RedeemSplit or a Migrate, and
  before the declaration of loss of §7.2. It does not prompt before a Request,
  a ReceiveFold, an Outcome, a RefundFold, a MintFold, a MigrateFold, a
  Recertify or a sync. Receiving and renewal therefore need no user present.
- The payer's successful authentication is the payer's confirmation. Any
  time target for a payment (§2.3) runs from that success, not from the
  prompt appearing. While a window is open the platform asks for nothing, and
  the confirmation shown is the app's own.
- A phone with no screen lock cannot show the prompt. The wallet then asks for
  a plain confirmation and tells the user that the phone is unprotected. A
  tier may instead refuse to pay until a screen lock is set. The user ends
  that stop on the phone, with no network, and loses nothing by it.
- Neither the device key nor the Android marker is created with a
  user-authentication requirement or with the unlocked-device requirement.
  AOSP documents that on Android 12 to 14 removing the screen lock deleted
  every key that had the unlocked-device requirement. On the same releases,
  by a reading of the key-store source, removing the screen lock also deletes
  an entry that holds no key. §5.10 therefore chooses the form of the marker
  entry so that it survives screen-lock removal, and the evidence gate tests
  it on each Android release a supported phone runs (§10.4).
- Enrollment needs no screen lock on Android. On iPhone it needs a passcode,
  because the marker sits in a keychain class that exists only while a
  passcode is set (§5.10). Apple documents that the items of that class are
  discarded when the passcode is removed or reset. The wallet says so before
  the first load. That loss follows from where the marker is kept, not from
  anything in this section; §4 (T6) and §5.10 state it.

**Considered and not used.** A second key, bound to authentication, that
co-signs every payment and is checked by receivers. Its death would cost a
renewal and not a balance. It adds a hardware signature and about 64 bytes to
every payment (estimate), a second P-256 check per hop inside a proof, and a
sync after every screen-lock change. No regulatory control asks for that
sync, so it fails P5.

What the evidence gate records for this section (§10.4): that the device key
and the marker still work after the screen lock is removed, changed and reset
by an administrator, and after a biometric is enrolled; the time from the
prompt to success; and that a renewal and a receive complete with no prompt.

### 5.10 Marker, checkpoint and crash recovery

The marker is how an unmodified wallet keeps its current state where no
backup, restore or file copy reaches it (§4.1). The marker carries a
checkpoint: the wallet's current state, small enough to sit in the phone's
key store. The key-store entry decides the state. The files are a record of
history.

- Files older, missing or damaged: the wallet resumes at the checkpoint,
  offline, with its current balance.
- An earlier balance does not come back. Every earlier checkpoint is deleted
  before anything signed after it is released, and no backup holds a
  checkpoint.
- A compromised phone is not constrained. It writes any checkpoint it likes
  (§4.1).

The first two statements hold under T2 (§4). This section uses these
behaviours of T2, by name:

- App separation. No other app and no tool can use the device key, or read,
  change or selectively delete a key-store entry of the wallet. Files may be
  read and replaced by restore tools; this section is built for that.
- No copy of a key-store entry. No backup, transfer, clone or rollback tool
  puts an older entry back.
- No selective removal. No platform path removes a marker, or another
  key-store entry of the wallet, while the device key stays usable. Removal
  or reset of the iPhone passcode is the stated exception (T6).
- Durable key-store writes. A creation or deletion that returned and was
  confirmed by a read survives a power cut. A key store that loses writes
  loses its latest ones and never keeps a later write while losing an
  earlier one.
- Durable file writes. A journal commit that returned survives a power cut.
- Distinct answers. The calls named below tell "absent" from "cannot be read
  now".
- No rollback by a failed update. A failed OS update does not roll the key
  store or the files back after the wallet has run.
- The key store answers again after an unlock or a restart.

Every platform statement in this section is a reading of source code or
documentation. Nothing in it has been run on a device. Every size is an
estimate; no encoder exists. The evidence gate (§10.4) tests each behaviour
per tuple.

**Terms.**

- A commit is one durable journal transaction. It holds one signed object,
  its signature, its proof if it is a transition, and its state change; a
  ReceiveFold and its Outcome are one commit. Each commit has a commit
  counter, one higher than the commit before. Each commit has a digest over:
  the previous commit's digest, the object's digest, a digest of the
  wallet's state after the commit, and the epochs and block list version
  held. The head is the last commit.
- A checkpoint is the wallet's state after one commit, in the form given
  under "What the checkpoint holds".
- A marker is an entry in the phone's key store that no backup carries. It
  carries the checkpoint of the commit it belongs to. Its name is: a fixed
  prefix; the first 8 bytes of the device id; the commit counter as 16
  hexadecimal digits; the first 8 bytes of the commit's digest; and a 4-byte
  random salt. The digest prefix ties the name to the commit, so that
  recovery can match a marker to the journal from a listing alone. The salt
  keeps a name from being used twice when a second attempt is made at one
  counter.
- A marker is valid if its checkpoint's checksum holds, its device id is
  this wallet's, and the counter and digest in its name equal those in its
  checkpoint. A marker that returns data and fails any of these is damaged.
  A marker is unreadable if it is listed and its data still cannot be read
  after the phone has been unlocked and the app restarted.
- The current marker is the valid marker with the highest commit counter. An
  extra marker is any other marker of this device id.
- The terms entry, the proof entry and the start entry are three further
  key-store entries, defined below.
- The gap is the run of commits between the journal's head and the
  checkpoint's head, when the files no longer hold them. To resume is to
  continue at the checkpoint's state across a gap.

**What the checkpoint holds.** Everything the wallet needs in order to keep
paying and receiving offline with no files, and everything whose loss would
let value be paid, credited, folded or refunded a second time. History does
not go in.

| Field | Bytes | Why it is in |
|---|---|---|
| Format version; flags, one of which is "Migrate folded" | 2 | A countersigned Migrate is folded once. The issuer returns it on request with no time limit (§7.2), so the files must not be what remembers the fold |
| Device id, first 8 bytes | 8 | A marker of another wallet is not read as this one |
| Commit counter | 8 | Orders checkpoints. One per commit, never reused |
| Start number | 4 | Orders two markers at one counter (recovery, below) |
| Head commit digest; previous commit digest | 64 | Compare with the journal; link two checkpoints |
| Balance | 16 | No signed object carries it (§5.7) |
| Redeemed total | 16 | The next RedeemSplit needs it |
| Last transition, complete, with its signature | 315 | It holds the sequence number, the previous digest, the cumulative out, refund and fee totals, the day and month counters, the last signed time and the certificate digest. The next transition is built on it. It is also what the issuer verifies after a gap |
| Voucher number of the last voucher folded | 8 | The issuer numbers the vouchers of a device id (§5.1, §7.1). The wallet folds only the next number. A repeated request returns the same voucher, so the files must not be what remembers the fold |
| Versions held: descriptor epoch, highest notice epoch, list epoch and counter, root succession number, rules version | 30 | The wallet never uses an older set |
| Time records of §5.4 as of this commit: last known time, boot reference, anchor state | 25 | A restore of older files must not open an earlier limit window |
| Digest of the terms entry | 32 | Names the terms entry in force |
| Digest over the payment ids of unresolved SendSplits beyond the two lists below, and their count | 34 | Permission to refund them stays in the key store while the members stay in the files |
| Counts of the lists below | 4 | |
| Checksum, SHA-256 cut to 16 bytes | 16 | Tells damaged data from valid data |
| **Fixed part** | **about 582** | |
| The proof-carrying design adds: the public commitment of the proven state, and the digest of the proof entry | 64 | Ties the checkpoint to its proof |
| Requests that can still be decided, at most 4: digest, nonce, amount and maximum, `created_at_ms`, the monotonic deadline and boot reference, `rules_lo` and `rules_hi`, flags, signature | 174 each | Only a Request in this list can be paid. With the terms entry it can be shown again and its Payment judged |
| Unresolved SendSplits, the newest 2, complete with signature, plus the receiver's device public key | 348 each | The Payment can be presented again, and a `Refused` Outcome verified and refunded |
| Unresolved SendSplits, the next 6, short form: payment id, amount, fee, `device_time_ms`, receiver's device public key | 105 each | A `Refused` Outcome can be verified and refunded, and the refund is counted in the window the SendSplit was signed in (§5.4). The Payment cannot be presented again from this form |
| Latest RedeemSplit with no ledger receipt, complete, if it is not the last transition | 315 | It can be sent to the ledger again. The total is cumulative, so only the latest matters |
| Last 4 decisions: payment id, verdict, reason | 34 each | The Outcome can be signed again over the same bytes and shown again |

Sizes: about 0.58 KB with nothing pending, about 0.76 KB with one Request
listed, about 3.0 KB with every list at its cap; 64 B more under the
proof-carrying design. The wrapper of the Android forms below adds about
0.3 KB. The caps are this document's choice (§12).

"Unresolved" means a SendSplit for which no RefundFold is committed and no
`Credited` Outcome was stored when the checkpoint was made. A `Credited`
Outcome is stored without a commit, so its entry leaves the list at the next
commit. Until then the checkpoint still lists the payment, which is harmless:
no `Refused` Outcome exists for it.

Four parts are in the checkpoint because the files must never decide what
can be credited, folded or refunded.

- The Request list. If the files decided which Requests are open, an ordinary
  receiver could be credited twice: receive a payment, put back files from
  before the credit while the Request is still inside its few minutes, and
  take the same Payment again.
- The unresolved SendSplits. If the files decided which payments can still
  be refunded, an ordinary payer could refund twice: refund a refused
  payment, put back files from before the refund, and scan the same `Refused`
  Outcome again. The digest covers every unresolved payment beyond the eight
  that are listed: the wallet refunds such a payment only if the set of
  unresolved payment ids in its files hashes to the checkpoint's digest, and
  the RefundFold's checkpoint carries the digest of the set without that id.
- The voucher number. Without it an ordinary user could load, pay the value
  away, lose the files, ask for the same voucher again and fold it a second
  time.
- The Migrate flag, for the same reason on a successor wallet.

**The terms entry.** A wallet with no files must still build a Request or a
Payment and judge the other side's. That needs objects that change only at a
sync or when a notice arrives. They are kept in a second key-store entry so
that they are not rewritten at every signed object.

- Content: the device certificate, which carries E in full (about 0.53 KB
  on Android, §5.1), the registration receipt (about 150 B, 67 B per extra
  witness), the policy head and the policy entry that the wallet's own E
  names (0.3 to 0.5 KB, §5.1), the scheme id, the root public key and
  `next_root_digest`, each issuer and witness key the wallet has verified
  with its role and key id (about 45 B each), the revocations held, the tier
  table, the rules range, the scheme status, how long a Request is open, and
  the fee policy in force with its `fee_policy_id`. About 2 KB with one
  witness and four tiers. §5.1 gives no size for a tier row, so the figure is
  rough. Other policy entries stay in the files. A wallet that lacks the
  entry a payer's E names takes it from the payer as a notice (§5.1).
- The checkpoint names the terms entry by digest. A Recertify, or the
  adoption of a notice, first writes the new terms entry under a new name;
  the next commit's checkpoint names it; recovery deletes a terms entry that
  the current checkpoint does not name. A notice adopted and not yet named
  by a checkpoint is, after a loss of files, a notice not received (§5.11).
- If the terms entry is missing or damaged while the checkpoint is valid,
  the wallet uses the copy in its files if that copy's digest matches.
  Otherwise it keeps its balance, can still unload, and can pay or request
  again only after a sync returns its certificate. Under T2 (no selective
  removal) this does not arise. It is listed below as a case where the
  criterion is not met if it does.

**The proof entry.** Under the proof-carrying design a wallet with no files
has its balance and cannot pay it offline unless it also holds the proof of
its current state and what the prover needs to extend that proof. Files that
were restored are not current, and a proof cannot be rebuilt across a gap. So
the proof and that material must sit in the key store as well. The proof
entry is written in step 4 of each transition, before the marker. The
checkpoint names it by digest, and the previous one is deleted with the
previous marker.

- It needs durability and not protection against an older copy. An older
  proof does not match the checkpoint and is useless.
- Size: the proof that travels is 6,528 B at the repository's ceiling. What
  the prover must keep to extend it may be larger and is not known. The
  repository's relation keeps a set of consumed credits that grows with every
  receive; a witness that includes that set has no bound.
- This is not designed. Q0 (§2.3) has to fix a relation whose witness has a
  size bound that a key-store entry can hold, and the gate has to show that
  each tuple's key store takes an entry of that size on the payment path.
  Until both exist, the statement "a wallet resumes and keeps paying
  offline" holds for the balance and for the checks that need no proof, and
  is not shown for a proof-carrying payment.

**The start entry.** At every run of recovery the wallet writes one small
key-store entry and reads it back before it lists the markers. The entry
carries a start number, one higher than the highest start number in any
start entry present. Every checkpoint made until the next recovery carries
that number. The write has two uses.

- Both key stores are single-writer databases in the sources read. A write
  that is confirmed has passed every earlier call through the same writer,
  so a marker creation left in flight by a killed process has landed, or
  failed, before the listing.
- If one lands later all the same, it carries a lower start number than any
  marker made after it. Recovery uses that to order two markers at one
  commit counter.

**Where it is stored: Android.** Android Keystore keeps, per alias, an
optional key blob and an optional certificate, and an alias may hold a
certificate with no key (AOSP `keystore2` `database.rs`, `service.rs`;
`AndroidKeyStoreSpi.java`). No file location is usable: a package rollback
snapshots and restores the app's storage, the no-backup directory included,
and an older ciphertext put back is still valid.

A marker on Android is, unless the gate shows otherwise for a tuple, a
Keystore entry that has a key blob with no authentication requirement. The
reason is one path. On Android 12, 13 and 14, removing the screen lock makes
the key-store service delete every app entry of that user except an entry
whose key blob carries no authentication binding (AOSP release branches
`android12`, `android13` and `android14`: `keystore2` `database.rs`,
`unbind_keys_for_user`, called from the reset path of `super_key.rs`). An
entry that holds a certificate and no key has no blob, so it is deleted,
while the device key, which has a blob and no authentication requirement, is
kept. The wallet would then be stopped with its key alive after an ordinary
settings change. On the Android 15 release branch and on `main` the path
deletes only entries that carry a user secure id
(`unbind_auth_bound_keys_for_user`), so a certificate-only entry survives
there. Source readings; no vendor build was read and nothing was run.

Three forms were examined. Each stores the checkpoint under the marker's
name.

| Form | How the checkpoint is stored | Screen lock removed, Android 12 to 14 (source reading) | Cost per marker | Other |
|---|---|---|---|---|
| Key with certificate | An EC key pair generated in the TEE, never used, with no user-authentication and no unlocked-device requirement. Its certificate slot is then replaced by a well-formed self-signed X.509 certificate with the checkpoint in one private, non-critical extension (`KeyStore.setKeyEntry` with the entry's own key; `AndroidKeyStoreSpi.setPrivateKeyEntry` updates the certificate of an existing Keystore key) | Kept | One key generation in the TEE and one database write | Reading the content goes through the platform's certificate parser |
| Alias | The same kind of key. Its alias is the marker name followed by the checkpoint in base64, encrypted under a second long-lived Keystore key so that log lines and error messages, which contain aliases, do not show the balance. No length limit on an alias was found; the listing is fetched in batches once a reply passes 358,400 bytes (`utils.rs`, `RESPONSE_SIZE_LIMIT`) | Kept | One key generation, one encryption | No parser. The content is read from the listing. The encrypting key becomes a second key that must survive |
| Certificate only | No key. `KeyStore.setCertificateEntry` stores the wrapper certificate as given, in one database transaction, with no call to the secure hardware | Deleted | One database write | Usable only where the gate shows that the entry survives |

The design uses the key-with-certificate form. The alias form is the fallback
for a tuple on which the certificate parser or the certificate update does
not behave as read. The certificate-only form may replace both on a tuple
for which the gate shows that the entry survives screen-lock removal; it
takes the key generation off the payment path. The marker name's prefix
names the form, so recovery reads whichever it finds.

The gate test that decides it is the first Android marker test, on each
Android release that a target phone runs: with a funded wallet, set the
screen lock to none, change it, enroll and remove a biometric, and apply a
device-administrator lock reset; after each, list the entries, read the
marker, the terms entry and the start entry byte for byte, and sign with the
device key. Pass: every entry is present and identical, and the wallet is
ready at the same head. A tuple that fails with every form is unsupported.

Further points for Android.

- The certificate wrapper is signed with a fixed software key that is a
  constant of the app. The signature protects nothing; it is there so that a
  conforming parser accepts the certificate. The platform parses stored
  bytes as X.509 when the app reads them and returns nothing if they do not
  parse (`AndroidKeyStoreSpi.toCertificate`). The app builds the wrapper with
  the platform's own provider, named explicitly, and installs no security
  provider ahead of it.
- The terms entry, the proof entry and the start entry use the same form as
  the marker.
- The device key's alias sorts after every other alias the wallet uses. The
  key-store service lists aliases in order and the framework's enumeration
  ends silently on an error, so a listing counts as complete only if it
  contains the device key's alias.
- `setCertificateEntry` overwrites a certificate entry of the same name
  without an error. That a name is never used twice is the wallet's own
  rule; the key store does not enforce it.
- If the read after a creation in the key-with-certificate form does not
  return the bytes written, the wallet writes that marker again in the alias
  form.

What the paths an ordinary user can run do to these entries, by AOSP source
reading. No vendor build was examined.

| Path | Files | Key-store entries (device key, marker, terms, proof, start) | Result |
|---|---|---|---|
| Uninstall without keeping data; clear storage | Deleted | Deleted | The wallet is gone |
| Uninstall with "keep app data" | Kept | Kept (`RemovePackageHelper.java`) | Nothing lost |
| Archive and unarchive (Android 15) | Kept | Kept | Nothing lost |
| Package rollback with data restore (developer options) | Put back to the snapshot | Untouched | Resume at the checkpoint |
| Auto Backup or device-transfer restore onto the installed app | The restore engine first clears the app's data, and with it its key-store entries, if the app declares no backup agent (`FullRestoreEngine.java`) | Deleted with the data, in the no-agent case | The wallet is gone, in the no-agent case. See the manifest points |
| A vendor's backup, clone or transfer tool, same phone | Older files, where the tool restores app data | Not documented by any vendor | Not known. Entries untouched: resume. Entries cleared: the wallet is gone. Older entries put back: T2 fails on that tuple |
| Screen lock removed or changed; biometric enrolled or removed | Kept | Kept, in the two key forms | Nothing lost |
| OS update | Kept | Kept. A failed first boot of an A/B update rolls the files and the key-store database back together; AOSP commits the update checkpoint before ordinary apps can run (`ActivityManagerService.finishBooting`) | Nothing lost |
| Second profile, dual app, private space | A separate instance | A separate namespace | A separate wallet |
| Factory reset | Deleted | Deleted | The wallet is gone |

Manifest points. Each rests on documentation or AOSP source and is not
tested on any build.

- A backup agent that saves nothing and restores nothing, together with the
  opt-outs from backup and device transfer. By source reading the restore
  engine then does not clear the app. A cleared key store is a destroyed
  balance; older files put back by a restore are harmless.
- `android:hasFragileUserData="true"`. The system then asks the user, at
  uninstall, whether to keep the app's data. Where the user keeps it, the
  files and the key-store entries stay.
- `android:manageSpaceActivity`. AOSP's settings app opens that activity in
  place of its own clear-storage dialog (`AppStorageSettings.java`), so the
  wallet can warn there. Vendor settings apps were not read.
- `rollbackDataPolicy="retain"`. It spares the holder the loss of history;
  the balance does not depend on it.
- The app is not direct-boot aware, so that it cannot run before an update's
  checkpoint is committed.

**Where it is stored: iPhone.** A marker is a keychain item of class
`kSecAttrAccessibleWhenPasscodeSetThisDeviceOnly`. Its value data is the
checkpoint. The terms entry, the proof entry and the start entry are items of
the same class.

- Why this class and no other. It is the only class Apple documents as never
  backed up (Apple Platform Security, keychain data protection). An item of
  the device-only classes that survive passcode removal is included in
  backups and returns when a backup is restored onto the same phone. The
  payment key is itself kept as such an item, as a blob only this phone's
  Secure Enclave can open, and nothing documented stops it working after a
  restore without erase. An older checkpoint in such a class could therefore
  return together with a working key: back up, pay, restore. So no copy of
  the checkpoint is kept in another class.
- Passcode. Apple's API page says that no item can be stored in this class
  on a phone without a passcode and that disabling the passcode deletes
  every item in it. Apple's Platform Security guide says the items become
  useless when the passcode is removed or reset. Whether an administrator's
  passcode clear on a managed phone, or the reset offered after a forgotten
  passcode, counts as "reset" is not documented. No source read says that a
  plain passcode change keeps the items.
- Lock state. Items of this class can be read only while the phone is
  unlocked.
- Size. Apple states no maximum for an item's data and describes the keychain
  as a store for small pieces of data. A checkpoint of 0.6 to 3 KB is inside
  every figure seen. A proof entry of 7 KB or more is not tested.

What each path does, as far as sources say.

| Path | Files | Payment key | Marker and other items | Result |
|---|---|---|---|---|
| Delete the app and install it again | Deleted | Survives on current iOS. Apple engineers call this an implementation detail that may change | Survive on the same basis | Resume at the checkpoint, offline. The App Attest key is dead; the re-attestation of §7.2 is needed before the next sync, not before paying |
| Offload and reinstall | Kept | Kept | Kept | Nothing lost. The App Attest key may die |
| One app's data restored by a desktop tool | Older | Untouched, by the tool vendor's guide | Untouched, by the same guide | Resume at the checkpoint |
| Whole-phone backup restored onto the same phone, no erase | From the backup: older, or absent if excluded | Not established. The item returns from the backup; one developer report from 2018 says such a key was gone | Not in the backup. Whether the existing items are left in place or removed is not documented | Not known. Items kept and key alive: resume. Items removed and key alive: stopped. Key dead: the balance is gone |
| Erase, then restore; iCloud restore, which needs an erase | From the backup | Dead. Apple documents that an erase discards what the Secure Enclave needs to rebuild it | Gone | The balance is gone |
| Transfer to a new iPhone | Maybe copied | Does not move | Do not move | The new phone has no wallet. The old phone is unaffected |
| Passcode changed | Kept | Kept | Not known; not tested | Not known |
| Passcode removed | Kept | Not known (§5.9) | Deleted | Stopped. The balance is out of reach |
| Passcode cleared by an administrator; reset after a forgotten passcode; Reset All Settings | Kept | Not known | Not known; not tested | Not known |
| iOS update | Kept | Kept | Kept | Nothing lost |

**Rules.**

1. The wallet signs and releases only when it is ready: exactly one marker of
   this device id exists, it is valid, and the journal's head is the commit
   that its checkpoint names. Enrollment is no exception: the wallet's first
   marker is created together with the device key (below), so the Bootstrap
   commit has an earlier marker like every other commit.
2. The wallet deletes a marker in two cases only. A valid marker that
   outranks it is present: one with a higher commit counter, or at the same
   counter the one that recovery keeps (V4). Or the marker is damaged or
   unreadable, and a valid current marker is present and confirmed. No rule
   deletes the current marker, the device key or the journal.
3. A marker name is never used twice.
4. The checkpoint decides the state. The journal is used only where it
   agrees with the checkpoint, or extends it with commits that verify.

What they give, under T2: an object is released only after every marker of
an earlier commit is gone, and the one marker then present carries the state
after that object. Whatever happens to the files afterwards, the wallet's
state includes that object.

**Steps for one signed object.** These are the seven steps of §5.2, with
what makes each one done. Start: head k with marker `old`, which carries the
checkpoint of commit k.

1. Pre-check. The wallet is ready, and every check that could refuse the
   object passes when evaluated now.
2. Sign in memory.
3. Prove in memory, where the object is a transition (§5.2). For a credit,
   the `Credited` Outcome is signed after the proof.
4. Create the marker `new` for commit k+1. It carries the checkpoint of
   commit k+1, which includes the signed object where the table above lists
   it. For a transition the proof entry is written first and confirmed by a
   read. `new` is done when the call returns success and a read of `new`
   returns the same bytes. In the key-with-certificate form the creation is
   two calls, and `new` is done only when the read returns the checkpoint.
5. Commit k+1 to the journal. Done, and durable, when the transaction
   returns.
6. Delete `old`. Done when the call returns success and `old` is definitely
   absent.
7. Release.

- Signing comes before the marker creation because the checkpoint holds the
  signature. The signed object reaches the key store before it reaches a
  file. A copy of the files can therefore never hold a signed object that
  the key store does not hold, and while `new` exists the wallet is at k+1.
- Step 4 fixes the state. An error before it leaves the wallet at k with
  nothing stored. After it there is no discard: if the journal write of step
  5 fails, the wallet waits at k+1 until the write succeeds.
- If any call or read in steps 4 and 6 returns unknown, the wallet is no
  longer ready and runs recovery before it signs or releases anything else.
- An object is complete, in the sense of §5.2, when step 6 is done. A credit
  is shown and an Outcome or a Payment is released only then.
- Within one process the wallet knows which markers it created and deleted.
  One process at a time holds the journal.

**Read outcomes.** Every read has three outcomes. Unknown is never treated
as absent. A read that returns data adds a fourth: the core checks the bytes
and decides valid or damaged.

| Platform call | Present | Definitely absent | Unknown |
|---|---|---|---|
| Android, existence, key forms: `KeyStore.getKey(name, null)` | returns a key | returns null. AOSP returns null only for the key-not-found code | throws any exception |
| Android, content, key-with-certificate form: `KeyStore.getCertificate(name)` | returns a certificate; the core checks the checkpoint in it | | returns null while the existence read says present. The marker is then unreadable |
| Android, certificate-only form: `KeyStore.getCertificate(name)` | returns a certificate | a listing that counts as complete does not contain the name | returns null while the listing contains the name; the listing is unknown |
| Android, create, then a read | the read returns the bytes written | | the call throws; or the read says anything else |
| Android, delete: `KeyStore.deleteEntry(name)`, then a read and a listing | | returns without an exception, the existence read says absent, and a complete listing does not contain the name | throws; or either check says anything else |
| iPhone, read: `SecItemCopyMatching` returning the data | `errSecSuccess` | `errSecItemNotFound`, while the app reports protected data available before and after the call | `errSecInteractionNotAllowed` (the phone is locked); any other status; `errSecItemNotFound` while protected data is not available |
| iPhone, create: `SecItemAdd`, then a read | the read returns the bytes written | | any other status; or the read says anything else |
| iPhone, delete: `SecItemDelete`, then a read | | `errSecSuccess` or `errSecItemNotFound`, and the read says definitely absent | any other status, or the read says anything else |

A listing has two outcomes: the list, or unknown.

- Android: `KeyStore.aliases()`. The result counts as the list only if it
  contains the device key's alias; otherwise it is unknown.
- iPhone: `SecItemCopyMatching` for all items of the marker service.
  `errSecSuccess` gives the list. `errSecItemNotFound` while protected data
  is available is the empty list. Any other status is unknown.

Notes on the table.

- Android applies to version 12 and later, the version the Pixel 6 shipped
  with. `containsAlias`, `isKeyEntry` and `size` are not used: AOSP returns
  false or zero from them on any key-store error. `getCertificate` returns
  null both for a missing entry and for a key-store error
  (`AndroidKeyStoreSpi.getKeyMetadata`), and for a certificate-only entry
  `getKey` returns null whether or not the entry exists; so for that form
  absence is taken only from a complete listing. In the alias form the
  content is part of the name, and a complete listing gives it.
- iPhone. Apple documents that the marker class behaves like the
  when-unlocked class, so no marker can be read while the phone is locked.
  Apple's published keychain source ends a query with
  `errSecInteractionNotAllowed` when a matching item cannot be decrypted
  because the phone is locked, and with `errSecItemNotFound` only when the
  query had no error and matched nothing (`SecItemDb.c`). That source may
  differ from what a given iOS version ships. Not tested.
- iPhone, no passcode. If the phone reports that no passcode is set, every
  marker is definitely absent. What a read returns then is not tested.
- The journal is read the same way. Readable. Unknown: the storage is
  locked, or any input-output or database-busy error. Definitely absent: the
  storage is available, its directory can be listed, and it holds no journal
  file. Failing its own checks: the file opens and a commit does not chain,
  or stored state does not match the head's state digest.

**Recovery.** It runs inside `open` at every start. It runs again before any
operation if the previous marker step did not end with a confirmed result.
Until it ends in "ready" the wallet signs nothing and releases nothing. The
steps are labelled V1 to V8. Every step can be repeated after an
interruption.

- V1. Read the device key's entry. Unknown: wait. Definitely absent: stop;
  the key is gone.
- V2. List the start entries. Unknown: wait. Create a start entry with the
  next start number and read it back. Not confirmed: wait.
- V3. Only now list the markers of this device id. Unknown: wait. Read every
  listed marker. A read that is unknown: wait, and read again after the next
  unlock or restart. A marker whose data still cannot be read then is
  unreadable, and V4 handles it.
- V4. Sort the markers into valid, damaged and unreadable, and find the
  current marker.
  - Two valid markers with one commit counter. The journal decides: the one
    whose commit the journal holds at that counter is kept. If the journal
    holds neither, the one with the higher start number is kept. The other
    was created by a process that was killed before its creation call
    returned, so nothing was released from it; it is deleted in V7. Two
    valid markers that agree in counter and in start number cannot be
    produced by the steps. If the journal does not decide between them, the
    wallet stops; the key store has then not behaved as T2 says.
  - The marker with the highest counter is damaged or unreadable, and the
    journal's head has that counter and a digest that begins with the
    digest prefix in the marker's name. The files are then current: that
    marker exists, so no object of a later commit was ever released. The
    journal is verified as in V5, a fresh marker is created for its head and
    confirmed, and the damaged or unreadable one is deleted in V7. This
    needs only the listing.
  - The marker with the highest counter is damaged or unreadable, the
    journal does not match it, and a valid marker with a lower counter is
    present. While an older valid marker exists, nothing of the later commit
    was released. The valid one is current, and the other is deleted in V7.
    Its object is gone, never released.
  - The marker with the highest counter is unreadable, the journal does not
    match it, and no valid marker is present. Wait. Nothing is deleted.
  - No marker is valid and none of the cases above applies: stop. Nothing is
    deleted. The marker is gone while the key lives.
- V5. Read the journal. Unknown: wait. Then one of four cases holds.
  - Same head. Nothing to do.
  - The journal is ahead: it contains the checkpoint's head commit and
    commits after it. Every later commit must chain by digest, carry a valid
    device signature over its object, and hold the state digest the core
    computes by applying that object. If all do, create a marker for the
    journal's head, with its proof entry taken from the commit record, and
    confirm it; that marker is now current. If one does not, the commits
    after the checkpoint's head are set aside and not used. This case arises
    only when the key store lost a confirmed write.
  - The journal is one commit behind: its head is the commit the checkpoint
    names as previous. The core rebuilds commit k+1 from the checkpoint and
    writes it. This is the ordinary interruption between steps 4 and 5. The
    rebuilt commit holds the wallet's own signed object and state. It does
    not hold what another party supplied and the checkpoint does not carry:
    the received Payment of a ReceiveFold, the voucher of a MintFold, the
    countersigned Migrate, the `Refused` Outcome of a RefundFold. It is
    marked as rebuilt, and the issuer treats it as a gap of one commit.
  - Anything else: the journal is definitely absent, fails its own checks,
    ends earlier, or does not contain the checkpoint's previous commit. The
    wallet resumes. It sets the old file aside, unchanged. It starts a new
    journal whose first record is the checkpoint, marked as a resume, with
    the commit counters at the two ends of the gap. Its state is the
    checkpoint's.
- V6. Check the terms entry that the current checkpoint names, and under the
  proof-carrying design the proof entry. Unknown: wait. Missing or damaged:
  as "The terms entry" and "The proof entry" say.
- V7. Delete every extra marker, every terms entry and proof entry that the
  current checkpoint does not name, and every start entry but the newest;
  confirm each definitely absent. Any unknown: wait.
- V8. The wallet is ready. `open` returns the head commit's object, which
  may not have been released, and every other object that can be presented
  again (§9). An Outcome that only the decision list holds is signed again
  over the same bytes.

An enrollment starts with the device key and the wallet's first marker. The
wallet creates that marker as soon as the key exists and before anything
about the key leaves the phone. Its checkpoint has commit counter zero, no
transition, a zero balance and voucher number zero, and it stands for an
empty journal. The Bootstrap commit of §7.1 then replaces it by the seven
steps. If the first marker cannot be confirmed, the wallet abandons that
key, which nobody has seen, and generates another. A device key with no
marker is therefore never an enrollment in progress: recovery stops (V4),
and nothing creates a marker for a key that has none. The rule is there
because an enrollment interrupted before its Bootstrap could otherwise not
be told from a wallet whose marker is gone. Continuing it would let an
ordinary iPhone user remove the passcode, delete the app, install it again,
finish the enrollment of the same key and fold every voucher of that device
id a second time (§7.1). Markers of
another device id are never this wallet's extra markers. While an earlier
wallet's key and a valid marker are on the phone, the app offers to resume
that wallet and enrolls a new one only after a declared loss (§7.1); the
earlier wallet's entries are deleted only where §7.2 deletes its key.

**Rules after a resume.** The core applies them. They need no network. They
are labelled N1 to N8.

- N1. Balance, totals, counters, last transition, voucher number, flags,
  versions and time records are the checkpoint's. An anchor made after the
  last commit is lost with the files; the wallet is then unanchored until
  its next issuer exchange (§5.4). Under `require_anchor` it does not pay
  until then; that is the reboot policy of R8 acting, where the scheme
  switched it on.
- N2. The old file is kept as a record. A commit in it is used only if it
  chains from the file's start and carries a valid device signature. A
  stored Outcome found that way may be shown again.
- N3. Only the Requests in the checkpoint's list can be decided. For a
  Payment addressed to any other Request created before the resume, the
  wallet returns a stored Outcome if N2 or the decision list holds one for
  that payment id. Otherwise it signs nothing. It does not answer `Refused`
  (§5.2).
- N4. A RefundFold is made only for a payment id in the checkpoint's two
  lists of unresolved SendSplits, or for one beyond them if the unresolved
  set that N2 finds in the old file hashes to the checkpoint's digest. If
  that set does not match, the payments beyond the lists can no longer be
  refunded; the wallet shows their count and starts an empty set.
- N5. No inbound credit is folded twice. A Payment is credited only for a
  Request in the list. A voucher is folded only if its number is the
  checkpoint's voucher number plus one; the issuer returns, on a request
  signed by the device key, every voucher of that device id above a stated
  number (§7.1), so a load that was pending when the files were lost is
  still folded. A countersigned Migrate is folded only while the flag is
  clear.
- N6. The per-payer tally restarts from the receives N2 finds in the current
  day and month.
- N7. The wallet uses the block list only if its files hold the version the
  checkpoint names, or a later one. If they do not and R6 is switched on, it
  neither pays nor creates a Request until it holds that version again. A
  short list can come from a peer; a list of more than about 200 entries
  arrives only at a sync (§5.5). Whether the list is kept in the key store
  as well, so that a resume never waits for it, is an owner decision (§12);
  it costs about 0.1 KB per segment and 40 B per entry of key-store space.
- N8. At the next sync the wallet uploads what the old file holds above the
  acknowledged head, then the resume record. The issuer accepts a resume
  record on three checks, each on an input it holds: the device key's
  signature on the checkpoint's last transition; that this transition is the
  acknowledged head itself, with the same sequence number and digest, or has
  a higher sequence number; and that its cumulative totals are not below
  those at the acknowledged head. It acknowledges that transition as the new
  head. It cannot replay the gap. The same holds at a renewal and for the
  journal check of a Migrate. This acceptance is part of the design: without
  it a wallet that resumed could never renew or migrate, and under R8 a rule
  other than an enabled control would end its sending. The second check
  covers a wallet that made no transition since its last sync, for example
  one that only created Requests.

Why accepting a gap adds nothing for an attacker. An ordinary user with the
unmodified app can produce a gap only by losing files, and under T2 the
checkpoint is then the true state. A compromised phone could already present
any journal that is consistent with itself (§4). What the issuer gives up is
the replay of honest wallets' commits inside the gap.

What is lost with the files, and what that costs.

| Lost | Cost | Who bears it |
|---|---|---|
| Journal history inside the gap | The issuer cannot replay those commits. It verifies the last transition's signature and totals instead. Replay finds arithmetic faults and some compromised phones (§4); it finds neither inside the gap | The issuer loses a detection tool. The holder loses nothing |
| Requests that are no longer in the list, and Outcomes older than the last 4 decisions | A Payment presented again for one of them gets no signed answer. If that Payment had been refused and the payer never received the Outcome, or if it arrives late for a Request that had closed, the payer cannot refund | The other party, the payer. The amount stays in neither wallet |
| Unresolved SendSplits beyond the two lists, where N4 finds no matching set | A `Refused` Outcome for one of them can no longer be refunded | The holder |
| The complete form of unresolved SendSplits older than the newest two | Those Payments cannot be presented again; a refund is still possible | The holder, if the receiver never saw the Payment |
| The per-payer tally for the current day and month (R7) | The tally restarts. One payer can then pass its limit at this receiver once more in that window. An ordinary receiver can cause it by restoring older files | Nobody while the payer is unmodified: it keeps its own limit. It weakens the per-receiver bound on a compromised payer (§3.2) |
| The block list (R6) | N7 | The holder, until the list returns. The cause is R6, an enabled control |
| SendSplits inside the gap, for fee settlement (§5.8) | A fee is settled per payment from the SendSplit and the receiver's credit. For a payment inside the gap it is settled when the receiver syncs, or not at all | The fee beneficiary |
| Stored copies of Outcomes received from others | The wallet asks again by presenting the Payment | Nobody |
| Under the proof-carrying design, the proof and its witness, if the proof entry does not hold them | The wallet cannot pay offline. It cannot unload either while the core commits a RedeemSplit only with its proof (§5.2, §8.2) | The holder. See "The proof entry" |

**States.**

- Ready. The wallet signs and releases.
- Waiting. Some read, list, create or delete returned unknown, or a journal
  write has not yet succeeded. The wallet signs nothing, releases nothing
  and deletes nothing. It tries again when the phone is unlocked, when the
  app comes to the foreground, and on each call. It never concludes that
  anything is lost. An object that is committed and not released stays that
  way until the wallet is ready. If the wait does not end, the app shows the
  platform error. No length of time turns waiting into another state.
- Stopped. The device key is definitely absent; or no valid marker of this
  device id exists while the key is present and recovery found no case that
  applies. The wallet signs nothing and deletes nothing. The state is
  evaluated again at every start, so a reading that later proves wrong costs
  nothing.
- Two more causes exist only where the key store has not behaved as T2
  says. One is the pair of markers that V4 cannot order. The other can be
  seen only at a sync: the issuer shows a transition signed by this device
  key with a sequence number above the checkpoint's. The wallet decides on
  the transition and its own signature on it, not on the issuer's word. It
  means the key store went back to an earlier state.
- A journal that is absent, damaged or older is not a cause of the stopped
  state. The wallet resumes.
- A resume is an event, not a state. `open` reports it with the two ends of
  the gap and what was lost, and the app shows it.

§7.2 names the same states from the user's side.

**Crash points.** Object k+1 on head k. `old` carries checkpoint k; `new`
carries checkpoint k+1. Copy A is the wallet's files as they were at head k.
"Older or none" is the files from any earlier head, or no files. Step
numbers are those above. Each cell assumes that the key store kept its
confirmed writes, except where it says otherwise.

| Interrupted after | Process killed, then start | Power cut, then start | Copy A restored, then start | Older or none, then start |
|---|---|---|---|---|
| Step 2 or 3 (signed, or proven, in memory) | Ready at k. The signature and the proof are gone; neither was stored | The same | The same | Resume at k, with a gap |
| Step 4, after the proof entry and before `new` | Ready at k. The proof entry names no marker and is deleted. The signature is gone | The same | The same | Resume at k, with a gap |
| Step 4 (`new` created, nothing in the journal) | `new` is current. Commit k+1 is rebuilt from it; `old` is deleted. Ready at k+1; `open` returns the object, unreleased | The same. If the key store lost `new`: ready at k, and the object is gone, never released | As a plain start: ready at k+1 | Resume at k+1; `old` is deleted; `open` returns the object |
| Step 5 (committed, `old` not deleted) | `new` is current; `old` is deleted. Ready at k+1; `open` returns the object | The same. If the key store lost `new`: the journal is ahead of `old` by one verified commit; a marker for k+1 is created again. Ready at k+1 | The journal is one behind `new`; commit k+1 is rebuilt. Ready at k+1 | Resume at k+1 |
| Step 6 (`old` deleted, not released) | Ready at k+1; `open` returns the object | The same. If the key store lost the deletion: `old` is deleted again. If it lost both writes: as the row above | Commit k+1 is rebuilt. Ready at k+1 | Resume at k+1 |
| Step 7 (released) | Ready at k+1. The object can be presented again | As the row above | Ready at k+1 | Resume at k+1. **The one exposure:** if the key store lost both writes in the power cut and the files are replaced or removed before the next start, the wallet is at k and the released object is forgotten |

The exposure in the last row applies to the last two columns alike: with
both key-store writes lost, copy A leaves the wallet ready at k, and older
or no files leave it resumed at k. If only the deletion is lost, `old` and
`new` both exist and the higher counter wins.

A process killed while the creation call of step 4 is in flight: the call
either lands before the start entry of the next recovery is confirmed, and
the row "Step 4" applies; or it does not land, and the row "Step 2 or 3"
applies; or it lands later, and V4 deletes it by its lower start number.

Two requirements, checked against the table.

- An unreleased signed object never becomes releasable twice. In every cell
  the wallet ends at k with the object gone, or at k+1 with the same object.
  It never ends at k while a copy of the object exists in a file, because
  the object is written to a file only after `new` exists, and while `new`
  exists the wallet is at k+1.
- A released object is never forgotten, under T2. After step 6 the only
  marker carries state k+1, and no restore of files changes the key store.
  The one exception is the exposure, which is a failure of T2 (durable
  key-store writes).

Restore paths, in the ready state at head n, with no crash.

| What is done to the phone | Result |
|---|---|
| Files replaced by a true earlier copy | Resume at n with a gap. Balance unchanged |
| Files deleted, or damaged | Resume at n with a gap. Balance unchanged |
| Files edited and put back | The edit is outside what the checkpoint names, so the files count as older. Resume at n. An edited record is not used: N2 needs a device signature |
| The stored Outcome of a refunded payment removed from the files, and the `Refused` Outcome scanned again | No second refund. The payment id is no longer in the checkpoint's lists, and the set in the edited files does not hash to the checkpoint's digest |
| Files replaced by a copy that holds a different object at a counter the wallet has passed | Such a copy can exist only if the key store once lost a confirmed write. If it does exist, it is not ahead of the checkpoint and is set aside |
| The app's key-store entries removed (Android uninstall without keeping data, or clear storage; erase) | The device key is removed with them. The wallet is gone |
| iPhone passcode removed or reset | The marker is deleted. Stopped |
| An older key-store entry put back while the key works | T2 says no ordinary user can. Where a tool can, the wallet resumes at the older state: a reset. The wallet detects it only at a sync |

Six sequences these rules are built to stop, each with an unmodified app.

- Copy the files; commit; kill before the deletion; start again and pay;
  restore the copy. The files are older than the checkpoint. The wallet
  resumes at the true state.
- Commit and kill before the deletion; copy the files; restore copy A and
  try to pay someone else; restore the second copy. After the first restore
  the wallet is at k+1, not k, because `new` exists. There is no second
  object at k+1.
- Receive and refuse; let the payer refund; restore files from before the
  refusal; accept the same payment. The Request is not in the checkpoint's
  list, and the decision list holds the refusal.
- Receive; restore files from before the credit while the Request's time
  still runs; take the same Payment again. The Request left the list in the
  commit that credited it.
- Load; pay the value away; lose or restore the files; ask the issuer for
  the same voucher and fold it. The voucher's number is not the checkpoint's
  number plus one.
- Take a signed, unreleased object out of a copy of the files and show it
  with other software after paying someone else from the earlier state. The
  wallet never returns to the earlier state while the marker that holds the
  object exists, so the object shown is the same payment, released early,
  and not a second one.

**The iPhone durability gap.** It is open, and the source reading is
unfavourable.

- What the sources say. Apple's published keychain source opens its database
  in write-ahead-log mode and sets no synchronous or full-sync option
  (`SecDb.c`; `SecItemServer.c` opens the keychain with the write-ahead log
  on). Apple's SQLite build as read on macOS defaults a write-ahead-log
  database to `synchronous=NORMAL`, which SQLite documents as able to lose a
  committed transaction on power loss. The iOS build was not read. Nothing
  was tested. On Android the reference key-store service sets no such
  option, and Android's SQLite build flags set no other default, so each
  transaction is synced before it returns; vendor builds were not read.
- The exact sequence, for an ordinary user with no computer. Pay a first
  receiver; the wallet creates `new`, commits the journal with full sync,
  deletes `old` and releases. Within seconds, force a restart with the
  buttons. If the keychain's two writes had not reached storage, the
  keychain after the restart holds `old` only. Before opening the wallet,
  delete the app; the files are gone and the payment key and `old` survive.
  Install the app again. The wallet resumes at k. Pay a second receiver the
  same value. Only the loss of the creation matters: if only the deletion
  is lost, both markers exist and the higher counter wins.
- Why the journal does not help here. The journal commit and the key-store
  write back each other for an honest holder: if the key store loses its
  write and the files are intact, recovery adopts the journal. An ordinary
  user controls the files, so against that user the journal gives nothing.
- The first iPhone gate test is this sequence: pay; force a restart 0, 2, 5,
  10 and 30 s after the release; delete the app; install it again; start.
  Pass: the wallet resumes at k+1 every time. One failure is a fail.
- The candidate barrier is a wait before release. If the gate shows that a
  keychain write survives a forced restart once a certain time has passed,
  the wallet holds step 7 for that time after step 6. The wait is charged to
  the payment time (§5.2). A second candidate is tested beside it: after
  step 6, call `sync()` and then `F_FULLFSYNC` on the journal file, and then
  release. Neither is tested. `sync()` may return before the data is
  written.
- Until that test passes, with or without a barrier, P2 against an ordinary
  user is not shown on iPhone. If no barrier works, the choices are: accept
  this exposure; have an iPhone wallet stop when its files are absent, which
  costs an honest holder the balance after an app deletion or a one-app
  restore; or treat the tuple as unsupported. The choice is the owner's
  (§12).

**What an ordinary user can still do**, with the unmodified app.

- Not bring back an earlier balance, under T2.
- Lose history and records by restoring older files or deleting them. The
  costs are in the table above. The balance does not change.
- Restart the per-payer tally of the user's own wallet, as a receiver.
- Leave a payer unable to refund, by losing the record of a Request for
  which that payer's Payment or `Refused` Outcome never crossed. The user
  gains nothing by it.
- Use the one exposure on a tuple whose key store loses a confirmed write in
  a power cut. Whether any target phone has that window is not tested.
  Reading the sources, the Android reference does not and the iPhone may.
- Use any tool that restores key-store entries. None is known. None has been
  tested.

**What an honest user can still lose.** Under T2 a balance is lost only where
the device key or the marker itself is gone.

| Event | What is lost | Can the checkpoint help |
|---|---|---|
| Phone lost, stolen or destroyed | The balance | No. The key is gone |
| Phone erased or factory reset | The balance | No |
| Android: app uninstalled without keeping data; storage cleared | The balance | No. The manifest points make each harder to do by accident |
| Android: a backup or transfer restore that clears the app before restoring | The balance | No. The backup agent is meant to prevent the clear; not tested |
| iPhone: whole-phone restore, if it kills the payment key or removes the marker | The balance | No. Whether either happens without an erase is not known |
| iPhone: passcode removed or reset | The marker. The balance is out of reach | No |
| Files restored, deleted or damaged | History and records only | Yes. This is what the checkpoint is for |

Where T2 fails on a tuple, three more losses exist, and the gate is there to
find them before a tuple is supported.

| Event | What is lost |
|---|---|
| The key store keeps a later write and loses an earlier one | The marker. The wallet is stopped with its files intact |
| The key store loses the writes of a released object's marker step, and the files are then lost | The released object is forgotten. The phone may later sign twice at one sequence number, which brings a hold on its row (§5.3) |
| The certificate parser stops accepting the wrapper while the files are also older | The wallet waits without end. With current files, V4 continues from them |

The rows about a dead key are one fact: the value is held by a key in one
phone, and what destroys that key destroys the value. Any way to give the
balance back without that key is a way to spend it twice (§7.3). T6 states
these events as a condition on the holder, apart from the two restore rows:
whether a restore that needs no erase destroys the key or the marker is a
finding of the gate per tuple (§10.4). Whether "durably" in P1 may rest on
that condition is the owner's decision (§12).

**A second anchor for the iPhone passcode was evaluated and is not in the
design.** The idea: every commit also makes one App Attest assertion with a
key used for nothing else, and records its counter; if the marker is gone and
the files remain, one more assertion whose counter is exactly one above the
recorded one shows that the files are current. It would cover one case: the
passcode removed, a passcode set again, the anchor key alive and the files
intact. It needs four things that are not established: the counter steps by
exactly one; no path an ordinary user can run leaves the key working with a
lower or repeated counter; the anchor key, the payment key and the files
survive passcode removal; a call that fails uses no count or the wallet can
tell. Apple documents only that the counter increases, and Apple engineers
have said that the loss of such keys on restore is to be changed. The match
would have to be exact, because any tolerance can be turned into a reset.
Without the anchor, a wallet with no marker never resumes, and that rule
depends on nothing untested. So the design keeps the passcode as a condition
(T6): enrollment on iPhone requires a passcode, and the wallet says before
the first load that removing or resetting it puts the balance out of reach.

**What a compromised phone can do.** Everything. It writes, keeps or copies
any checkpoint and signs from any state. The checkpoint is the paying app
policing itself. A receiver cannot check it, and a proof does not check it.
Nothing in this section is a control against a compromised phone (§4.1).

**Where this mechanism cannot be shown to meet the criterion.**

- P1 after removal or reset of the iPhone passcode. The marker is deleted and
  nothing on the phone then shows which state is the latest. Not removed by
  this design.
- P1 after anything that destroys the device key. Not removable on a stock
  phone.
- P1 and P5 under the proof-carrying design after a loss of files, until the
  proof entry is designed and sized.
- P1 and P5 if the terms entry is lost while the marker survives and the
  files hold no matching copy. Outside T2.
- P2 on iPhone until the durability test passes. An evidence gap whose source
  reading is unfavourable.
- P2 on any vendor's Android build until the restore paths are run there. No
  vendor documents what its backup, clone or transfer tool does to Keystore
  entries.
- P2 against a compromised phone. Outside what a checkpoint can do.
- PC rests on T2 (durable key-store writes): the debit and the credit are
  established only if the key-store writes are durable when confirmed.

If T2 fails on a tuple, an ordinary user of that tuple can pay from an
earlier state. The receiver cannot tell, and under P4 the receiver's value
stands. §3.2 says what follows; this section decides nothing about it.

**Cost.** Per signed object, on the phone that signs it: one marker creation,
one read back, one deletion, and the reads that confirm the deletion. On
Android in the two key forms a creation includes a key generation in the TEE;
in the key-with-certificate form also one database write. Under the
proof-carrying design each transition also writes and later deletes a proof
entry of 7 KB or more. That is once on the payer and once on the receiver
inside the payment. A Request is not a transition (§5.2): it writes no proof
entry, and its marker names the one already there. At each start: one start
entry created and one deleted, which in the two key forms is one more key
generation; two listings; one read per marker; and a check of the stored state
against the head's state digest. None of this is timed. The effect of many
thousands of creations and deletions on a key store is not tested. The evidence
gate times each part and runs the endurance test (§2.3).

**What the gate must show for this section**, per tuple, none of it run:
entries of 0.6 KB, 3 KB and the proof-entry size written, read back
identical, listed and deleted, after a reboot and after an OS update; the
read mapping of the table above with the phone locked, before first unlock,
and with the key-store service failing; the screen-lock test above; every
restore path of the two path tables, with the pass condition that the wallet
resumes at the balance after the payment and the entries are byte-identical;
forced power-off after a creation and after a deletion returns, by both
power-cut methods; a process kill while a creation call is in flight; the
iPhone sequence above; every cell of the crash table and every row of the
restore table; and, in the core's test mode with no device, the six
sequences, a damaged checkpoint, an unreadable marker with current and with
older files, two valid markers at one counter with and without a journal,
and a journal ahead with one commit that fails verification.

### 5.11 Versions, policy changes, key rotation and closure

A scheme changes after phones hold value: the rules get a new version, a tier
row is tightened, the policy table gets a new entry, a key is replaced, the
scheme closes. This section says how each change reaches a phone that is
offline and what the phone can do before it arrives. Three rules hold
throughout. No change reduces value a wallet has received. No change makes a
wallet delete a key or a balance. No change stops a working wallet until it
goes online, except through a regulatory control the scheme has switched on.
Nothing in this section is implemented or tested.

**Rules versions**

- A rules version is one integer per scheme, counted from 1. It fixes the
  transition layout, the checks of §5.2, the time rules of §5.4, the fee
  computation and the evidence rules of §5.3. Every transition carries the
  version it was signed under as its first field, and the place and width of
  that field never change.
- The descriptor holds `rules_max`. Raising it switches a new version on. A
  wallet signs nothing under a version above the `rules_max` it holds.
- **No version is ever switched off for wallets.** There is no lowest allowed
  version. Every release of the app accepts, and can sign under, every
  version the scheme has ever allowed, from 1 to its own highest. Nobody can
  check this offline; it is a rule for the operator's releases, and part of
  what T3 assumes. With it, any two wallets share a version. The rule is
  there because a lowest allowed version would stop an app that has not been
  updated from paying or being paid, and an update needs a connection that no
  regulatory control asked for.
- **Choosing the version for a payment.** The Request states `rules_lo` and
  `rules_hi`: the versions the receiver's app can judge and its descriptor
  allows. Under the release rule `rules_lo` is 1; the field is carried so
  that a faulty build shows itself. The payer signs the SendSplit under the
  highest version in that range that its own app supports and its own
  descriptor allows. If there is none, the payer signs nothing and nothing is
  debited; both apps say so. That can happen only with a build that breaks
  the release rule. Checked by the payer's unmodified app, with the signed
  Request as input.
- The receiver refuses a SendSplit whose version is outside the range its own
  Request stated. The signed preimage is `tag ‖ scheme id ‖ rules version ‖
  payload` under every version, so the receiver can verify the device
  signature over a payload it cannot read and answer with a signed `Refused`.
  The payer then refunds. Only a modified or faulty payer sends such a
  SendSplit.
- A wallet signs its own other transitions (ReceiveFold, RefundFold,
  RedeemSplit and the rest) under the highest version it supports that is not
  above `rules_max`. A journal therefore mixes versions. Each transition is
  judged under its own version, and each new version states how it follows a
  state left by an earlier one.
- The Request, the Payment envelope, the Outcome, the certificate, the
  receipt, the notices and the list segments each have one layout for the life
  of the scheme, with an extension area. A reader ignores an extension it does
  not know, unless the extension is marked critical; then it treats the object
  as one it cannot judge, and no payment is made. A critical extension is used
  only for a regulatory control the scheme has switched on. Otherwise a new
  build could shut out old ones by that route. About 2 to 4 B per object
  (estimate).
- The issuer's replay and the ledger keep the checks of every version that
  was ever allowed, without time limit. A RedeemSplit, a fee settlement or
  evidence signed under an old version is judged under that version.
- Under a proof a rules version also fixes the relation. Every release then
  carries a verifier for every relation ever allowed and a prover for every
  version it can sign under. That size is not estimated. How value proven
  under an earlier relation is accepted is fixed in Q0 (§2.3). The reverse
  case is not designed. A release holds no verifier for a relation switched
  on after it was built, so a wallet that has not updated cannot verify
  value whose history holds a hop under the newer relation. It answers
  `Refused`, the payer refunds, and it cannot be paid such value until it
  updates the app, which no regulatory control asks for. Until Q0 fixes a
  form of proof that every earlier release verifies, the rule that no
  change stops a working wallet until it goes online is not met under the
  proof for such a wallet (§3.1). The other
  direction is not designed. A wallet that has not updated cannot verify a
  proof whose history holds a transition under a relation newer than its
  app, unless the proof that travels verifies under a key that does not
  change between releases (§2.1, "Policy inputs"). Until that exists, a new
  relation stops such a wallet being paid that value until it updates its
  app, and no regulatory control asks for that (§3.1). The other
  direction is open. A release cannot carry a verifier for a relation that
  is allowed after it was built. A wallet that has not updated can then be
  paid value proven under the newer relation only if the proof that travels
  verifies under a key that does not change between releases. That is not
  designed (§2.1, §2.3). Until it is, the rule that no change stops a
  working wallet is not met for such a wallet (§3.1).
- **What this gives up.** A version with a defect that lets value be created
  cannot be shut out offline. Every receiver keeps accepting it, because the
  receiver cannot tell an old honest build from a wallet that exploits the
  defect. That is a failure of T3, and §3.2 lists it. What bounds it is in
  the next item.
- **An app-build floor at renewal.** A policy entry can carry a lowest app
  build for each platform (§5.1). Where a scheme uses it, the issuer applies
  it at enrollment and at renewal, from the app identity in the evidence: a
  wallet on an older build must update the app before its certificate is
  renewed. With R8 on, no build below the floor holds a live certificate one
  lease after the entry takes effect, among receivers whose clocks are right.
  With `Never` certificates an old build lives as long as its holder stays
  offline. A renewed wallet keeps accepting every version ever allowed, so
  the floor shuts out no wallet offline. Whether the floor is part of what R8
  checks is an owner decision (§12). If it is, it is one of the reasons §7.1
  lists for refusing a renewal. If it is not, no renewal is refused for the
  app's build or version, and a defective build is retired only by its users
  updating.
- A build floor acts on what a wallet runs when it renews. Nothing offline
  shows which build a wallet runs: the rules version in a SendSplit is the
  payer's own statement, and E shows the build of the last renewal.

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
- If a wallet still cannot verify the other's certificate, or lacks the
  policy entry the other's E names, no payment is made, and the other
  wallet's app can show its notices as a separate code. That costs one extra
  scan, once per wallet per change.
- With an app update, which may carry the newest notices.
- There is no push to an offline phone. A wallet that has received a notice
  from nobody applies the older terms, and nothing relies on it having done
  otherwise: the counterparty that does hold the notice applies it.
- Adopting a notice takes a marker step (§5.10). The notices a wallet holds
  are in its key store, and the marker names them. Putting older files back
  therefore does not return a wallet to looser terms. The block list is the
  one exception: it is kept in the files, and §5.5 says what a wallet does
  when it has lost it.

**Tier-row notices**

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
- No certificate is voided and no holder is stranded. A tightened row is R7
  or R8 with new values. At its strictest (a limit of zero, or a lease that
  has ended) the holder cannot send until the next window or the next sync.
  The holder still receives, the balance is unchanged, and it unloads at face
  value.
- A row that brings in a lease switches R8 on for certificates that were
  issued without one. This document reads the root doing that as the scheme
  explicitly enabling the control. The owner confirms or rejects that reading
  (§12). If it is rejected, a new lease reaches a certificate only at a sync
  its holder chooses.
- A change of fee policy does not alter a payment already signed: the
  SendSplit names the `fee_policy_id` it used, and the ledger keeps every
  record ever installed (§5.8). A wallet pays under the policy in its
  certificate until its next renewal.

**Policy entries**

- A new policy entry acts at enrollment and at renewal, and nowhere else. The
  issuer and the validators judge the evidence of an enrollment or a renewal
  under the newest entry. Between renewals a certificate keeps the E it has,
  and a receiver judges that E under the entry it names (§5.1).
- The reason is P5. A receiver that applied a newer patch floor to a payer
  offline would send that payer online, to update its system and renew, with
  no lease having run out. Where the scheme wants evidence to have a maximum
  age, it sets a lease: that is R8.
- So a raised patch floor, a newly revoked vendor certificate, a new app
  signer and a class that is no longer admitted all reach an enrolled phone
  at its next renewal. With R8 off they never reach it.
- A policy entry that stops admitting a platform class applies to new
  enrollments. Whether it also ends renewals for phones of that class that
  are already enrolled is an owner decision (§12). If it does, those holders
  stop sending at their lease end, and they can Migrate to an admitted phone
  or unload.

**Renewal: the expiry for attestation (R8)**

The owner asked for "some optional expiry for attestation so users will have
to sync online before they can send offline again, optionally". This document
reads R8 as that. Where a tier has a lease, the certificate ends, and the
renewal that follows carries fresh evidence and returns a refreshed E. Where a
tier has no lease, there is no renewal, and E stays what it was on the day of
enrollment for the life of the wallet.

What the phone presents, and what that shows:

| | Android | iPhone |
|---|---|---|
| What the phone presents | A key generated for this renewal, with the issuer's nonce as challenge, certified by the app attestation key. One certificate, about 0.6 to 0.8 KB (estimate from Google's vectors for leaves under a system key; a leaf under an app's key has not been captured) | An assertion by the App Attest key that was certified at enrollment, over the renewal request. About 140 B on iOS 26, about 185 B on iOS 27 (estimates) |
| Network the phone needs for it | None for the leaf itself, by source reading: the app attestation key signs it and the vendor's provisioning service is not called. Not tested | None for the assertion itself, by statements of Apple's engineers. Not tested in airplane mode |
| What it shows | The boot state the secure hardware was told at the current boot (bootloader locked, boot verified, the verified-boot key), the OS, vendor and boot patch levels of the current boot, and the app identity as the operating system reports it. By inference from how the key store binds key blobs, it comes from the secure hardware that holds the app attestation key; not tested | That something holding the certified key signed the request. From iOS 27, a launch category and a bundle version, which are collected on the device |
| What it does not show | That the running system has not been taken over since it booted. That the released app asked for it. Whether the state had one successor | Anything about the operating system: no version, patch level, boot state or jailbreak state. That the payment key is in the Secure Enclave |
| What the issuer and the validators check | The leaf's signature under the app attestation key in the registry row; locked and verified; the patch levels against the newest entry's floor; the app identity; the stored vendor certificates against the vendor's revocation list of that day; the app build, if the owner makes that part of R8 | The assertion under the certified key; the App ID; on iOS 27 the launch category and the bundle version against the entry. No patch floor can be applied: there is no field |
| What E holds afterwards | The fields of the new leaf, the entry they were judged under, and the new enrollment epoch | The same App Attest facts, the entry, the new enrollment epoch, and on iOS 27 the bundle version of that day |

- **What comes back.** A certificate with the next serial and the refreshed
  E, and the receipt that names it (§5.1). The issuer first submits the
  renewal with its evidence. The validators verify the leaf or the assertion
  when that write executes, and write the new serial, the refreshed E and
  `e_digest` to the row. The issuer then signs the certificate, and the
  witnesses sign its receipt (§7.1).
  The wallet adopts the certificate with a device-signed `Recertify`. A
  repeated request returns the same certificate and receipt.
- **What a renewal depends on.** The issuer, one ledger write being final,
  and k of the n witnesses. With R8 on, an outage of any of the three that
  lasts longer than what is left of a lease plus `expiry_grace` stops that
  phone sending until it ends. With R8 off no wallet renews and such an
  outage stops nothing offline. §7.1 has the flow.
- **Why a renewal is refused.** For three kinds of reason and no other
  (§7.1). A regulatory control that is on: the bound account is blocked (R6).
  The content of R8: the evidence does not satisfy the newest policy entry;
  the app-build floor is among that entry's checks only if the owner makes
  it so. Or the row is under a hold, which needs two conflicting signatures,
  verified on-chain, by the device's own key or by the key whose balance it
  took over by Migrate. A renewal is never refused because of who paid the
  wallet, because of a key revocation, or because the wallet presents a
  resume record (§5.1) in place of a full journal.
- **What the patch floor does.** It is checked at enrollment and at each
  renewal, against the leaf. It excludes phones that still carry holes
  published and patched before the floor's date. It does not exclude a hole
  that is not yet patched, and it shows nothing about a system taken over
  while running (§2.1, §4).
  - Between renewals it does nothing. A phone that met the floor on its
    renewal day and has not been updated since still pays until its lease
    ends. The lease is therefore the longest age of the evidence behind any
    payment that a receiver with a right clock accepts.
  - A floor raised by a new entry reaches a phone at its next renewal. The
    holder must first install a system update at or above the floor, which
    needs the vendor and not the issuer.
  - A phone whose vendor issues no more updates cannot meet a floor above its
    last patch level. Its holder then cannot send after the lease ends, and
    cannot request after `receive_not_after` where the tier sets one. The
    balance is not lost: the holder can Migrate to a phone that meets the
    floor, or unload at face value. Google's update commitment for the Pixel 6
    ends in October 2026 (§10.3). What the floor is for each class, and
    whether it applies to phones past their vendor's commitment, is the
    owner's decision (§12).
  - On iPhone no floor exists. An iPhone renewal refreshes the epoch and, on
    iOS 27, the bundle version. It adds nothing about the operating system.
  - HarmonyOS NEXT attestation carries no boot, lock or patch field. No
    renewal content is designed for it (§10.2).
- **A vendor certificate revoked since enrollment.** The issuer tests the
  device's vendor certificates against the vendor's revocation list at each
  renewal. A device whose certificate is on the list is refused. Its holder
  cannot send after the lease ends, and can Migrate to another phone or
  unload. A factory-provisioned attestation key is shared by many phones, so
  honest phones that share a revoked one are refused with it. A remotely
  provisioned key belongs to one device. Whether a pool admits
  factory-provisioned chains is the owner's decision (§2.1, §12). With R8 off
  a revocation never reaches a phone that is already enrolled.
- **A phone without the app attestation key feature.** Android offers it from
  Android 12 where the phone's key store supports it. Third-party firmware
  listings show the feature file on most target models and not on all; no
  phone has been tested. The evidence gate records it per tuple (§10.4).
  Where it is absent, the fallback is a fresh key under the vendor's own
  chain whose public key the device key signs. That shows the app that holds
  the device key. It does not show the same hardware, and on a phone that
  gets its attestation keys over the network it needs that network.
- **An iPhone whose App Attest key died.** Apple reports an error for a key
  that is attested twice, so a renewal cannot repeat the attestation of the
  enrolled key. A dead key is replaced by the one online re-attestation of
  §7.2, which gives the device a new E under the same device id. Offline the
  wallet pays and receives as before in the meantime.
- **What the proof then says.** §2.1 states it in full. In short: each paying
  phone's secure hardware was told at boot that a vendor-signed image was
  verified on a locked bootloader, with patch levels at or above the floor,
  as of enrollment or the last renewal, and the operating system named the
  wallet's package. On iPhone the statement is about an Apple Secure Enclave
  key and this scheme's App ID, with nothing about the operating system.
  Neither says that the operating system was uncompromised when a payment
  was signed.

**Planned key rotation**

- Replacing a key on schedule uses no revocation. The old key stops signing.
  Everything it signed stays valid on its own terms: a certificate until its
  own expiry, a `Never` certificate without limit of time, a receipt for as
  long as its certificate. Wallets keep every issuer key certificate and
  every root key they have held. A planned rotation therefore sends nobody
  online. Revocation is for compromise only (§5.1), and it stops no wallet
  either.
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
- **Compromise.** A stolen issuer key is revoked (§5.1). The revocation stops
  no wallet and changes no value. A stolen root key can sign notices and
  issuer key certificates until wallets hold the succession to the committed
  next key; it cannot forge that succession. What stays accepted after an
  issuer or a root key was stolen, and what that can cost, is in §3.2.
- The scheme cell on the ledger must accept a new descriptor epoch, a new
  policy entry, a new root and a status change. The existing governance
  pattern for KAGEMUSHA installs a policy once against an empty predecessor
  and has no path to replace it
  (`crates/iroha_data_model/src/governance/types.rs:755-781`), so this is new
  ledger work (§6, §8.5).

**Closure**

Redemption never ends. A scheme can be closed to new value. It cannot be
closed to the value phones already hold. Closure means the following and no
more.

- **Closed to loads.** From a descriptor epoch on, the ledger refuses Load and
  the registration of devices for new holders, and the issuer issues no
  voucher. Payments between wallets, renewals, unloads, fee settlement and
  evidence go on as before. A wallet that learns of the status shows it to
  the user and changes nothing else. Closure by itself sends nobody online.
- **Moving to a new phone after closure.** A device that is registered as the
  successor of a Migrate, for an account that already has a live row, is
  still registered after closure. Otherwise a holder whose phone is failing
  could no longer keep the balance spendable.
- **Bringing value home.** The operator can wait. It may also want sending
  to stop, so that holders unload. Two tools exist, and each is the owner's
  to allow (§12). A tier-row notice can bring in a lease for every tier; that
  is R8 switched on for certificates already issued (above). And the issuer
  could stop renewing after a date, so that sending stops at each lease end.
  That is a renewal refused for a reason §7.1 does not list. It is in the
  design only if the owner adds it to those reasons. Either tool stops
  sending. Neither stops receiving, and neither stops unloading.
- **What never ends.** An unload pays at face value whenever it is presented.
  The ledger keeps the registry, the pooled reserve, the unload instruction,
  fee settlement, evidence, the checks of every rules version, every policy
  entry and every fee policy record.
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
  wallets, subject to its own certificate, and can unload at any time.

## 6. Roles

No role below takes part in an offline payment except the two wallets (R1).

| Role | Keys it holds | What it does | What it learns |
|---|---|---|---|
| Ledger (validators, and anyone who can read the chain) | Consensus keys, and the validators' seal keys that the mint path uses today (§8.1) | Holds the pool, the device registry, the scheme cell, the policy table with its set of revoked vendor serials, the fee policy table, the block index and, where recovery is enabled, the recovery account. Verifies the vendor evidence of every registration and renewal, writes the enrollment statement E into the registry row and seals it (§8.1). Executes the instructions below | The raw vendor evidence of each registration and renewal, and E: device key, platform class, security level, on Android the boot state, patch levels and app identity. Each row's tier, bound account, status, totals and counters. Each load and unload with amount and time. The time of each renewal and each sync, with the sequence number. Successions. Every pair of transitions submitted as evidence. With fees, every payment whose fee is settled (§5.8). Not wallet balances, and no other offline payment |
| Scheme root | Root key, kept offline | Signs the scheme descriptor, issuer key certificates and revocations, fee policy records and policy entries | Nothing from operation |
| Ledger governance | The ledger's existing governance procedure | Installs the scheme cell, policy entries and fee policy records after checking the root signature | What the ledger learns |
| Block authority | Whatever §12 names | Sets and clears account blocks (R6) | What the ledger learns |
| Issuer service (off-chain) | Certificate key, voucher key, list key, registry authority key. Four separate governed roles | Checks vendor evidence before it authorizes a registration; signs certificates, vouchers, lists, registration authorizations and countersigned Migrates; accepts sync, including a resume record (§7.1); submits renewals, head anchors, retirements, fee settlements and newly revoked vendor serials; keeps every countersigned Migrate with no time limit | Each device's vendor evidence and bound account. Every transition of a wallet that syncs, apart from those lost in a gap (§5.10), so both sides of each such payment and its balance. When and from where each wallet connects |
| Registration witnesses | One witness key each | Sign a receipt for each certificate, after the ledger write that records its serial is final on their own node (§5.1, §8.1) | Each certificate and, from the chain, the registration and its vendor evidence |
| Enrollment prover (a server; only where the enrollment proof exists, §7.1) | None | Makes the enrollment proof from evidence that is public on the ledger. A proof is sound whoever makes it, so this role is not trusted | What the ledger learns |
| Fee beneficiary | Its online account key | Receives fees | The payments whose fee it was paid, from the chain |
| Recovery insurer (only where a scheme enables recovery, §7.3) | Its account key | Pays cash into the recovery account in advance | What the ledger learns |
| Bound account (the user online) | Its account key | Registers the device, loads, receives unload payouts, declares a loss | Its own row |
| Wallet | The device key in secure hardware. On Android the app attestation key, in the same hardware. On iPhone the App Attest key. The marker with its checkpoint, in the key store (§5.10). The journal in files | Pays, receives, unloads | What §5.7 lists |

No role is named as the party that adds cash to the pool. `FundKagemushaPool`
takes cash from any account (§8.4). Who does so, if anyone, is one of the
choices of §3.2.

Vendor revocation data. A policy entry names the root of the set of vendor
certificate serials that were revoked when the entry was made (§5.1), and that
set is ledger state. Between entries the issuer fetches Google's revocation
list daily and adds the new serials with `AddKagemushaRevokedSerials`. Google
serves the list without a signature, so the validators cannot check an
addition against its source. A serial added falsely, or one left out, is a
failure of T4 (§8.1). Apple publishes no revocation for App Attest keys; the
only Apple-side signal is the receipt risk metric.

The pool account has no signer. Only the instructions below move its balance.
The existing reserve account is already built that way: its id is derived from
a key nobody can sign with, and a direct transfer out of it is refused
(`crates/iroha_core/src/smartcontracts/isi/domain.rs:613-621`,
`crates/iroha_core/src/smartcontracts/isi/asset.rs:695-708`; read from code,
not run). The recovery account, where a scheme has one, is to be built the same
way.

**Ledger instructions.** These are all the instructions the flows of §5 to §8
need. None exists today. Each touches at most two registry rows and the pool
record, except evidence against a row retired by Migrate, which follows the
succession to its live end. None iterates over rows or claims.

| Instruction | Submitted by | The chain checks | Effect |
|---|---|---|---|
| `SetKagemushaScheme` | Ledger governance | Root signature over (epoch, digest); epoch above the cell's | Installs or replaces the scheme cell: role keys, tier table, ledger parameters (the unload window, `unload_limit`, the registration caps, whether Load is open, whether Load stays open while the pool is short), recovery parameters. A key revocation is a new epoch |
| `AddKagemushaPolicyEntry` | Ledger governance | Root signature; the entry holds the digest of the entry before it | Appends one entry to the policy table (§5.1, §8.1) and replaces the set of revoked vendor serials with the set the entry names. Entries are never changed or removed |
| `AddKagemushaRevokedSerials` | Registry authority | That authority | Adds vendor certificate serials to the revoked set that registrations and renewals are checked against. It removes none; only a new policy entry replaces the set |
| `AddKagemushaFeePolicy` | Ledger governance | Root signature; id not yet present | Adds a fee policy record. A second form replaces only a record's payout account (§5.8) |
| `SetKagemushaAccountBlock` | Block authority | That authority | Sets or clears `send_blocked` and `receive_blocked` for an account; consensus derives the device entries. Not needed if §12 names an existing ledger fact |
| `RequestKagemushaDeviceBlock` | The bound account | That the scheme has switched on holder-requested blocking as part of R6 (§7.2) | Sets or clears a block on one of the account's own device ids; consensus derives the entry. The instruction is refused in a scheme that has not switched it on |
| `RegisterKagemushaDevice` | The account being bound | Authorization signed by the registry authority key; submitter equals the account in it; account not blocked; device id is new and is the hash of the key; registration caps; the vendor evidence, by the checks of §8.1, under the newest policy entry in force | Creates the registry row with E, its digest and the first certificate serial. The validator quorum seals the registry root (§8.1) |
| `RenewKagemushaDevice` | Registry authority | Row is live, or retired by its account; row not held; bound account not blocked; no holder-requested block on the device id (§7.2); new serial is the row's serial plus one; the device key's signature over the renewal request; where the tier has R8 on, the renewal evidence by the checks of §7.1; for an iPhone re-attestation (§7.2), the new attestation by the checks of §8.1 | Raises the row's serial and, where evidence was checked, replaces E. A row retired by its account becomes live. The validator quorum seals the registry root. The issuer signs the new certificate, and the witnesses its receipt, only after this is final |
| `AnchorKagemushaHead` | Registry authority | That the device key in the row signed this head, and that its sequence number is not below the row's | Records the acknowledged head and its sequence number |
| `RetireKagemushaDevice` | Registry authority, or the bound account | From the registry authority: the old key's signed Migrate naming a successor row bound to the same account, and the new key's signature on the same request. From the bound account: its own signature (a declared loss, §7.2) | Sets the row to retired. For a Migrate it records the final redeemed total and the successor. It emits no block entry |
| `LoadKagemusha` | The bound account | Row is live and not held; account not blocked; Load is open; load id is new | Moves the amount into the pool; adds it to the row's loaded total; raises the row's load counter by one and records that value as the voucher number of this load |
| `RefundKagemushaLoad` | Anyone | The load exists and is not refunded; a statement signed by the voucher key that this load id and voucher number are void | Returns the load to the bound account as §7.1 says |
| `UnloadKagemusha` | Anyone | With a RedeemSplit: its signature against the row's key, the row's status and, where the relation fixed in Q0 requires it, its proof (§8.2). With a device id only: nothing more | Records the claim, if any; pays what is due to the bound account or queues it (§8.2) |
| `FundKagemushaPool` | Any account | That the account holds the amount | Moves the amount into the pool; adds it to the added-cash counter; serves the payout queue (§8.4). Nothing takes it out again except a payout |
| `SettleKagemushaFee` | Registry authority | §5.8: both device signatures, that the SendSplit's `fee_policy_id` names a record in the fee policy table, the fee under that record, that neither row is held, and that this payment's fee was not paid before | Pays one fee to the record's beneficiary |
| `SubmitKagemushaEvidence` | Anyone | Two objects signed by one registered device key that satisfy a predicate of §5.3, with both signatures checked against that row's key | Places a hold on that key's row or, if that row was retired by Migrate, on the live row at the end of its succession (§8.2). Evidence of an issuer-key fault is recorded and places no hold |
| `ReinstateKagemushaDevice` | Whoever the owner names (§3.2) | To be defined with that decision | Ends a hold and says what the holder gets back |
| `FundKagemushaRecovery`, `ClaimKagemushaRecovery`, `PayKagemushaRecovery` | The insurer funds; the bound account files; the payout is submitted as §7.3 defines | §7.3. All three are refused unless the scheme enables recovery. A payout changes no row's status and never touches the pool | Pays an insurance claim from the recovery account, within the cap |

Under the proof-carrying design, `LoadKagemusha` and `UnloadKagemusha` also
carry what the relation fixed in Q0 requires (§2.3), and the existing top-up
path stays (§8.5). Under witness model (B) one more instruction is needed, by
which the issuer flags a registration it cannot match to its log.

The head anchor has a cost that the normative spec must settle. The chain can
check the device's signature over a head in two ways. If the transition
preimage is laid out so that the sequence number and a digest of the rest are
enough to verify the signature, the anchor carries the head transition's own
signature and the wallet signs nothing more. Otherwise the wallet signs a
separate sync statement, which is one more hardware signature per sync. After
a resume the head is the last transition in the checkpoint (§5.10), and the
same check applies to it.

**What depends on the chain being live.**

| Operation | Issuer service | Chain finality | Witness quorum |
|---|---|---|---|
| Offline payment | no | no | no |
| Resume after older, missing or damaged files (§7.2) | no | no | no |
| Enroll | yes | yes | yes |
| Load | yes | yes | no |
| Unload | no | yes | no |
| Renewal, or any new certificate | yes | yes | yes |
| Sync without renewal | yes | no; the head anchor may follow | no |
| Migrate | yes | yes | yes, for the new device |
| iPhone re-attestation (§7.2) | yes | yes | yes |
| Block list refresh | yes | no | no |

- A renewal waits for chain finality and for k of the n witnesses. The
  validators check the renewal evidence and seal the result, the serial is
  recorded because a block entry covers serials up to the recorded one (§5.5),
  and the witnesses sign the receipt of the new certificate. With R8 on, an
  outage of the issuer, the chain or the witness quorum that lasts longer
  than what is left of a lease plus `expiry_grace` stops that phone sending
  until it ends. That is R8 at work: the control the scheme switched on needs
  all three. With R8 off no wallet has to renew, and such an outage stops
  nothing offline.
- Unload needs a ledger node and nothing else. It does not need the issuer, a
  valid certificate or a fresh list (§8.2).
- Where the tier has R8 off, a new certificate carries no renewal evidence,
  and the issuer could have several serials recorded ahead and issue
  certificates without a further write. That is not adopted here; it is an
  owner decision (§12). Where R8 is on it is not possible, because each
  renewal carries evidence made at that moment.

**What the chain learns.** Every registration puts the raw vendor evidence on
the ledger, where anyone can read it. On Android that is a certificate chain of
3.1 to 4.0 KB in Google's test vectors, and one more certificate of 0.6 to 0.8
KB (an estimate; none was captured). It shows the boot state, the patch levels,
the app identity and whatever else the vendor put in the chain. On iPhone it is
the attestation object: 5,906 bytes in the one sample read, of which 3,977 are
a receipt that the checks of §8.1 do not use. It names no model and no OS
version. Neither size was measured on a target phone. A renewal adds the leaf
on Android and an assertion of under 0.2 KB on iPhone (estimates). Every
renewal and every sync writes to the device's registry row. Anyone who reads
the chain sees when each device id, and so each bound account, went online, and
from the sequence number how many operations the wallet made between two syncs.
With R8 on, the chain also sees each phone's patch levels at each renewal. The
amounts and counterparties are not on-chain. Anchoring heads as one digest per
period over all devices that synced would hide which device synced; the flows
that read a row's acknowledged head (§7.2, §7.3) would then carry a membership
proof, and the check that a head was signed by the device would move from the
chain to the issuer. That is not adopted here. Privacy is not among R1–R9
(§5.7), so this is an owner decision (§12).

## 7. Flows

A payment needs no network (R1). Every flow in this section needs it, except
the resume of §7.2, which runs on the phone alone. A wallet starts a flow only
when it is ready (§5.10). The one exception is the declaration of loss in
§7.2.

### 7.1 Enroll, load, unload, sync

- **Enroll.** Enrollment gives one device key four things: the enrollment
  statement E in the registry, sealed by the validators; a device certificate
  that carries E; a receipt that names that certificate; and, where it
  exists, an enrollment proof.
  1. Keys on the phone.
     - Android. The app creates an app attestation key. That is a key in the
       phone's secure hardware whose only use is to sign the attestation of
       other keys the app generates there. The vendor's certificate chain
       attests it. The app then generates the device key and has the app
       attestation key sign its attestation. Both attestations carry the same
       challenge: the hash of the enrollment transcript, which names the
       scheme id, the account, the policy entry and a recent block hash. The
       block hash is there so that the validators can date the evidence. That
       an app attestation key can be created and used on each target phone,
       with no network after enrollment, is read from AOSP source and from
       third-party firmware listings. It is not tested on any phone; the
       evidence gate tests it per tuple (§10.4).
     - Android, where the phone has no app attestation key. The vendor's
       chain attests the device key directly. Whether such a tuple is admitted
       is a value of the policy entry (§8.1). What it loses is stated under
       Renewal below.
     - iPhone. The app creates the payment key in the Secure Enclave. It has
       Apple attest a separate App Attest key over a transcript that names the
       payment key, and it signs the same transcript with the payment key.
       Apple offers no way to attest the payment key itself (§4).
  2. The issuer checks the evidence and signs a registration authorization
     (§5.1). The issuer's check decides nothing by itself. It keeps
     registrations that would fail off the ledger. It also declines a tuple
     that the owner's ruling on the evidence gate does not support, as far as
     the evidence shows the tuple (§10.4, §8.1).
  3. The bound account submits `RegisterKagemushaDevice` with the raw
     evidence. Every validator verifies the evidence while it executes the
     block, with the checks of §8.1, from inputs that are all on the ledger.
     The row is created with E, and the height of that block is E's
     enrollment epoch. The validator quorum seals the registry root (§8.1).
  4. The issuer signs the device certificate. It carries E as the row holds
     it, and its serial is the one the registration recorded (§5.1).
  5. The witnesses sign the receipt that names that certificate. Each signs
     after it has seen the registration final on its own node and has checked
     the certificate's device key, serial and E against the row (§8.1).
  6. Where the enrollment proof exists, a server makes it from the evidence
     on the ledger and hands it to the wallet. It proves that the vendor chain
     verifies and that the statement meets the constraints of §2.1. It is the
     starting point of the phone's recursive proof. It is never made on a
     phone and is not on the payment path. Whether it can be built is a
     qualification item (§2.3): the repository has no P-384, RSA or SHA-384
     gadget (§8.5). Until it exists, the recursion starts from the statement
     the validators sealed.
  7. The wallet commits its Bootstrap as an ordinary commit (§5.10). The
     marker it replaces is the first marker, which the wallet created with
     the device key in step 1, before the key left the phone in any
     evidence. An enrollment that was interrupted is continued only while
     that marker is present.
     It cannot pay or request before it holds the certificate and the
     receipt, and under the proof-carrying design the proof of its Bootstrap
     state (§2.1).

  Every step can be repeated. A wallet that was interrupted asks again, by
  device id, for whatever it does not yet hold.

  What E says is in §4, and no flow here claims more. In short, for Android:
  the phone's secure hardware was told at boot that a vendor-signed image was
  verified on a locked bootloader, the patch levels it was told were at or
  above the floor as of enrollment or the last renewal, and the operating
  system named the genuine wallet package. E does not show that the running
  operating system is uncompromised. For iPhone E holds no OS, patch, boot or
  jailbreak field: it says that a genuine Apple Secure Enclave key acts for
  this scheme's App ID.

  Enrollment depends on the issuer, on chain finality and on the witness
  quorum, and every user needs an on-chain account able to submit the
  registration. Enrollment creates new keys under new aliases. It never
  deletes or overwrites a key, a marker or a journal that is already on the
  phone. If a device key and a valid marker of an earlier wallet are on the
  phone, the app offers to resume that wallet (§7.2). It enrolls a new one
  beside it only after a declared loss. Markers are named by device id
  (§5.10), so the two wallets' markers do not mix.
- **Load.** The bound account signs `LoadKagemusha` for the device id. The
  registry numbers the loads of a device id: the row's load counter rises by
  one, and that value is the voucher number of this load. When the load is
  final the issuer records a voucher and releases it. The voucher carries the
  voucher number, the load id and the amount (§5.1). The wallet folds vouchers
  in order and once each. It writes a MintFold only for the voucher whose
  number is one above the last number it folded, and the last number folded is
  in the marker (§5.10).
  - Why the number. A voucher can be fetched again, and it must be: a crash
    between the load and the MintFold must not lose the load. If only the
    journal recorded which vouchers were folded, a wallet that resumed after
    losing its files could fold a voucher a second time. The number in the
    marker survives the loss of the files. Checked by the wallet's own app,
    from the voucher and its marker.
  - A repeated request returns the same voucher. The wallet needs no file to
    ask: it asks for the vouchers above its last folded number, and the issuer
    finds them by device id. After a voucher key is revoked, a voucher that
    was issued and not yet folded is issued again under the new key with the
    same number (§5.1).
  - The wallet shows the amount as balance only after the MintFold has
    committed and its marker step is confirmed (PC).
  - The wallet starts a load only when it has folded, or seen voided, every
    earlier number. So at most one load of a wallet is open at a time.
  - A load that fails. A load is final on the ledger before its voucher
    exists, so a load can be left without one. Such a load is refunded
    on-chain to the bound account. The chain cannot see vouchers, so the
    refund needs the issuer's signed statement that the load id and its number
    are void. The issuer marks the load void in its own record before it
    signs, and issues no voucher for a void load. The chain checks that
    signature, that the load is committed to that account, and that it was not
    refunded before. The wallet takes the same statement in place of the
    voucher and steps its last folded number past it, with a MintFold of
    amount zero whose subject is the void statement. §5.1 and H7 of §2.1
    describe a MintFold for a voucher only; Q0 fixes the void form (§2.3).
    An issuer that signs a void statement and also releases a voucher has
    created unbacked value, as a stolen voucher key does (§8.1).
  - A load with neither a voucher nor a void statement. The amount stays in
    the pool as the account's claim, and the wallet cannot load again until
    the issuer answers. No rule here ends the wait. It is an issuer fault, and
    no offline value exists yet. A time limit enforced by the chain would need
    every voucher anchored on-chain before release, which is one more write
    per load. Owner decision (§12).
  - The refund first removes that load from the row's loaded total. If the
    row's paid total is above what remains, the refund up to that excess is
    released as a claim above the row's own loads is (§8.2); the rest is paid
    at once. None is forfeited: a device that received more than it loaded has
    a paid total above its loads without having cheated.
  - Under the proof-carrying design the wallet proves the MintFold before it
    commits it. What authorizes a mint inside the proof, the voucher key's
    signature or the validators' seal over the load, is fixed with the
    relation (§2.3). §8.1 says what a stolen voucher key can do in each case.
- **Unload.** The wallet signs a RedeemSplit that carries the increment and
  the cumulative total. The wallet is debited when the RedeemSplit commits.
  The marker keeps the latest RedeemSplit until the wallet has stored the
  ledger's receipt for it (§5.10), so it can be presented again after a crash
  or a resume. The chain records the claim in full (§8.2). The wallet shows
  an amount as paid only when it has stored the ledger's receipt for the
  payout; until then it shows it as claimed. The ledger's receipt is the
  ledger's record of that claim or payout, as the wallet reads it from a
  ledger node (§8.2). It is not the registration receipt of §5.1. Its form
  is not designed.
  - Payee. An unload pays only the account bound to the device id in the
    registry. The RedeemSplit names no payee, and the chain takes the payee
    from the registry row. Anyone may present a signed RedeemSplit, at any
    time; the presenter gains nothing.
  - Before it signs, the wallet reads its row from a ledger node. It shows
    what will be paid at once, what will wait and until when, and whether the
    pool is short (§8.4). It proposes the amount that is paid at once, so that
    no spendable value becomes a waiting claim unless the holder asks for
    that. With the ledger unreachable it signs nothing.
  - Send expiry, the clock-reset state, a key revocation, a stale block list
    and retirement of the device id by its account (§7.2) do not stop an
    unload.
  - Under the proof-carrying design the wallet proves the RedeemSplit before
    it commits it. Whether the chain requires that proof, and what follows
    for a phone that cannot make it, is in §8.2.
- **Sync.** A direct exchange with the issuer, authenticated by the device key
  with a signature over an issuer nonce (on iPhone also an App Attest
  assertion). That signature changes no state and is not a journal object. The
  wallet uploads its journal above the acknowledged head. The issuer replays
  it and returns what is due: a renewed certificate, the block list, issuer
  key certificates and revocations, the descriptor, and a time anchor (§5.4).
  The issuer then acknowledges the head it has replayed. Where the sync itself
  adds a transition (a `Recertify`), the wallet uploads that too and the
  issuer acknowledges it. If that last step is lost, the acknowledged head
  stays one transition behind until the next sync, and nothing follows from
  that.
  - A wallet that resumed (§7.2) has a gap in its journal. It uploads what its
    files hold above the acknowledged head and then a resume record: its last
    transition, complete and signed, as the checkpoint holds it (§5.10). The
    issuer accepts a resume record on three checks, each on an input it holds:
    the device key's signature on that transition; a sequence number above the
    acknowledged head's, or the same sequence number with the same digest;
    and cumulative totals not below those at the acknowledged head. It then
    acknowledges that transition as the new head. The second form of the
    sequence check is for a wallet that made no transition since its last
    sync, for example one that only created Requests.
  - The issuer cannot replay the commits in the gap. Replay finds arithmetic
    faults and some compromised phones; it finds neither inside a gap. That
    costs the issuer a detection tool. It gives an attacker nothing new: an
    ordinary user with the unmodified app can produce a gap only by losing
    files, and the checkpoint is then the true state; a compromised phone
    could already present any journal that is consistent with itself.
  - The issuer never refuses a sync, a renewal or a Migrate because of a gap.
    If it could, a wallet that resumed under R8 would stop sending at its
    lease end for a reason that is not a regulatory control (P5).
  - At a sync the wallet may also fetch the issuer's copy of its journal up to
    the acknowledged head and keep it as a record. It changes no state.
- **Renewal.** A sync that returns a certificate with a higher serial under
  the same device id. The wallet adopts it with a device-signed `Recertify`.
  A renewal does not reset the day and month counters (§7.2). Where the tier
  has R8 on, a renewal is the owner's "expiry for attestation": it refreshes
  E. What it carries:
  - Android. A fresh attested leaf: the app generates a new key in the secure
    hardware and the app attestation key that the registry row holds beside
    E (§5.1, §8.1) signs its attestation,
    with the hash of the renewal request as challenge. The leaf shows the boot
    state and the patch levels of the current boot. The validators check the
    signature under the app attestation key, that the phone is locked and
    verified, the app identity, the patch levels against the floor of the
    policy entry in force, and the certificate serials of E against the
    revoked set on the ledger. The new key is used for nothing else.
    Generating it needs no network by source reading; not tested.
  - Android, where the phone has no app attestation key. A fresh key attested
    under the vendor's chain, whose public key the device key signs. It shows
    the same fields for the hardware that made the fresh key. It shows that
    the app holding the device key reached that hardware. It does not show
    that the device key is in the same hardware.
  - iPhone. An assertion by the App Attest key named in E, over the hash of
    the renewal request. It shows that something holding that key signed the
    request. From iOS 27 Apple documents that it also carries the launch
    category and the bundle version, as the device reports them; not captured
    on a device. It shows nothing about the operating system. The validators
    check the signature, the App ID hash and that the assertion counter is
    above the last one the row recorded. Apple does not attest a key twice,
    so a renewal never repeats the
    attestation; an App Attest key that has died is replaced by the
    re-attestation of §7.2.
  - Both. The device key's signature over the renewal request, which names
    the device id, the new serial and a recent block hash.

  The issuer submits `RenewKagemushaDevice` with that evidence. The validators
  verify it, write the refreshed E and the new serial to the row, and seal the
  registry root (§8.1). The issuer then signs the new certificate, which
  carries the refreshed E, and the witnesses sign the receipt that names it
  (§5.1). A repeated request returns the same certificate and receipt, so a
  lost response uses up no serial. A renewal therefore depends on the issuer,
  on one ledger write being final and on k of the n witnesses. Where the tier
  has R8 off, a new certificate, for example one that moves a limit share,
  carries no evidence and leaves E as it is. It still needs the write and the
  receipt.

  A renewal is refused only for these reasons. Each names who refuses and on
  what input.
  1. The bound account is blocked on the ledger (R6), or, where the scheme
     has switched on holder-requested blocking, the account's own request
     blocks this device id (§7.2). The registry refuses the serial, from the
     block index.
  2. The renewal evidence fails a check above (R8). The validators refuse it,
     from the evidence, the policy entry and the revoked set. One
     case needs saying plainly: a phone whose vendor no longer ships updates
     falls below a rising patch floor for good. It then cannot send after its
     lease ends, and where receive freshness is on it cannot request after
     `receive_not_after`. It can still unload, and it can Migrate to a phone
     that passes. Google's update commitment for the Pixel 6 ends in October
     2026 (§10.3). The floor, and whether it applies to such phones, is the
     owner's decision (§12).
  3. The row is held: the device's own key, or the key whose balance it took
     over by Migrate, signed two successors (§8.2). The registry refuses the
     serial, from the hold.
  4. Only if the owner makes it part of R8 (§5.11): the attested app build is
     below a floor in the policy entry. On Android the validators read the
     version from the leaf. On iPhone they read the bundle version from the
     assertion, from iOS 27 only.

  A renewal is never refused because of who paid the wallet, because of
  evidence against, a block of or a hold on any other device, because the
  journal has a gap, because an issuer key was revoked, or because of the
  rules version the app signs under, unless the owner decides item 4. A
  retired row is covered in §7.2. If the policy entry also lists the OS major
  versions that the evidence gate has covered, refusing a version outside
  that list is part of the same owner decision as the patch floor; this
  document does not assume it. Whether a policy entry that stops admitting a
  platform class also ends renewals for phones of that class that are already
  enrolled is an owner decision too (§5.11).

  Two consequences follow from the chain write. If the chain does not finalize,
  or the witness quorum does not sign, no certificate is renewed, and under R8
  a phone whose lease runs out in that time stops sending. And the chain
  learns, for each device id and so for each account, when it renewed and how
  often, with its patch levels and a digest of its journal head (§6).

Sync is optional. A wallet that never syncs keeps paying and receiving, except
in the cases below. These are all the cases in which a rule of §5 or §7 makes
a working wallet need the network. Each is a regulatory control that the
scheme has switched on.

| What stops | When | Control | What ends it |
|---|---|---|---|
| Sending | The certificate is past `not_after + expiry_grace` | R8, lease | A renewal. It needs the issuer, one final ledger write and the witness quorum, and it can be refused for the reasons above |
| Sending | After a reboot, under `require_anchor`. Also after a resume that lost an anchor made since the last commit (§5.10, N1) | R8, reboot policy | Any direct issuer exchange |
| Requesting | `receive_not_after` has passed | R6, receive freshness | A renewal |
| Sending under a lease or limits. Being paid by a payer whose certificate has a lease or limits | The clock-reset state (§5.4) | R7 and R8 need time | The wall clock returning within the tolerance, where §5.4 allows that; otherwise a re-anchor |
| Paying and being paid, among holders of the entry | The device id has an R6 block entry from the account block index | R6 | The ledger unblocking the account, then a renewal above the entry's serial (§5.5) |
| Paying and being paid, among holders of the entry | The bound account asked for a block on its own device id (§7.2) | R6, only where the scheme has switched on holder-requested blocking | The account clearing its request, then a renewal above the entry's serial |
| Sending beyond the tightened terms, once either side holds the tier-row notice | A tier-row notice lowers a limit, or brings in or shortens a lease (§5.11) | R7 or R8 with new values | The next window; a renewal where the new lease has ended |
| Paying and requesting | After a resume, the files no longer hold the block list version that the checkpoint names (§5.10) | R6 | That version or a later one, from a peer or from the issuer. A long list arrives only at a sync (§5.5) |
| Paying, and being paid by, a wallet whose objects carry a critical extension that this wallet's app does not know | The scheme marked an extension critical (§5.11) | The control the extension carries. §5.11 allows a critical extension only for a regulatory control the scheme has switched on | An app update |

With R6, R7 and R8 off none of the rows occurs. Receive freshness,
`send_blocked`, the per-counterparty cap, `require_anchor`, a tier-row notice
that switches a control on for certificates already issued, and a block at
the holder's request are this document's additions under the owner's three
controls. Each counts as an
enabled regulatory control only if the owner confirms it (§12).

Stops that need no network, listed so that the table is not read as complete
for every stop:

- A day or month limit is reached (R7). It ends when the window turns.
- The wallet is waiting (§7.2). It ends on the phone, when the storage or the
  key store answers.
- A tier refuses to pay until a screen lock is set (§5.9). The user ends it on
  the phone.
- The wallet is stopped (§7.2). The network does not end it.

What does not stop a working wallet: a key revocation (§5.11); a new rules
version (§5.11); older, missing or damaged files (§7.2); retirement of the
device id by its account (§7.2); and anything learned later about a device the
wallet was paid by. A held row is outside the table. A hold needs two
successors signed by the device's own key, or by the key whose balance it took
over by Migrate, and a phone on which the assumptions hold does not produce
them (§3.2).

Enrolling, loading, unloading, a Migrate, a declaration of loss, an iPhone
re-attestation and a recovery claim need the network. Each is something the
holder chooses to do. None is needed to keep paying.

### 7.2 Key and phone changes

Common rules.

- Every journal the issuer accepts must extend its last acknowledged head for
  that device id, by commits that replay or by a resume record (§7.1). The
  acknowledged heads are anchored on-chain at sync. The table of acknowledged
  heads is what bounds a Migrate and a recovery claim (§7.3), and it is where
  a key store that went back to an earlier state would show.
- A wallet never deletes its device key, its journal or its current marker on
  its own reading of the phone. It deletes a marker only where §5.10 says so.
  It deletes an old key and old files only in the two places named below,
  each after a final on-chain fact. This is a wallet rule: a wallet never
  destroys its own key or balance on a condition that may pass.
- The marker decides the wallet's state (§5.10). The files are a record of
  history. Everything in this section follows from that.

**The wallet's states.** §5.10 owns the mechanism. This is what the holder
sees and can do.

| State | When | What the wallet does | What the holder sees and can do |
|---|---|---|---|
| Ready | The device key answers, one valid marker exists, and the journal's head is the commit that marker names | Signs and releases | Everything |
| Waiting | The storage or the key store did not answer: the phone is locked, a read failed, a call returned an error, a marker step is not yet confirmed | Signs nothing, releases nothing, deletes nothing. Tries again at the next unlock, at the next launch and on each call. Never concludes that anything is lost, however long it lasts | The platform's error and "try again". Nothing is lost. An object that is committed and not yet released is released once the wallet is ready |
| Stopped | The key store answered, and the device key is gone, or no marker of this wallet is left (§5.10 gives the exact cases) | Signs nothing, deletes nothing, keeps everything it finds | What was found and what is missing. The balance cannot be used. The holder can declare the loss (below). The state is worked out again at every start, so a reading that later proves wrong costs nothing |

A reboot, an app update and an OS update leave a wallet ready (T2). The
evidence gate tests each per tuple (§10.4).

**Resume.** When the device key and a valid marker are present and the files
are older, missing or damaged, the wallet resumes: it continues at the state
in the marker's checkpoint, on the phone, with no network. Its balance is its
current balance. It starts a new journal whose first record is the checkpoint,
and it keeps the old files as a record. The app tells the holder that a resume
happened and what was lost. A resume is not an error.

- Cases, by source reading and not tested: an iPhone app deleted and
  installed again; one app's data put back by a desktop tool; an Android
  package rollback; a damaged journal file; a power cut that lost the last
  journal write.
- Putting back older files gives no earlier balance. The older checkpoint was
  deleted before anything signed after it was released, and no backup holds a
  checkpoint. This rests on T2: no tool puts an older key-store entry back,
  and a key-store write that was confirmed survives a power cut. On iPhone
  the second point is open. Apple's published keychain source does not sync
  its writes, so the sequence pay, force a restart, delete the app, install it
  again may resume one state back. It is the first iPhone test of the evidence
  gate, and a wait before release is the candidate barrier (§5.10, §10.4).
  Until that test passes, this document does not claim P2 on iPhone against
  that sequence.
- After a resume the wallet folds no credit a second time: a Payment, a
  `Refused` Outcome, a voucher or a countersigned Migrate. The marker holds
  the open Requests, the unresolved SendSplits, the last voucher number folded
  and the Migrate flag for that purpose (§5.10).

What is lost with the files, and for whom.

| Lost | Cost | Who bears it |
|---|---|---|
| Journal history in the gap | The issuer cannot replay those commits (§7.1) | The issuer loses a detection tool. The holder loses nothing |
| Requests that are closed, and decisions older than the last few the marker keeps | A Payment presented again for one of them gets no signed answer. The wallet cannot rule out that it credited that payment in the gap, so it does not answer `Refused` (§5.10). If the payment had in fact been refused and the Outcome never reached the payer, or if it arrives late for a Request that had closed, the payer cannot refund | The other party. The amount stays in neither wallet. It concerns a payment that did not complete |
| Unresolved SendSplits beyond those the marker keeps complete | They cannot be presented again from this phone. A `Refused` Outcome for one that the marker no longer lists cannot be refunded | The holder |
| The per-payer tally for the current day and month | It restarts from what the old files still hold | Nobody while the payer's phone meets the assumptions. It weakens the per-receiver bound of §3.2 |
| The block list | With R6 on, the wallet neither pays nor requests until it holds the version its checkpoint names (§7.1 table) | The holder, until the list returns. The cause is R6 |
| Stored copies of Outcomes received from others | The wallet asks again by presenting the Payment | Nobody |
| Under the proof-carrying design: the last proof and its witness, if they were only in the files | The wallet has its balance and can prove no new transition, so it cannot pay offline. It cannot unload or Migrate either: a RedeemSplit and a Migrate are transitions, and the wallet commits none unproven (§5.2) | The holder. §5.10 keeps both in the key store for this reason. Whether a key store can hold them is a qualification item (§2.3). Until it is shown, P1 after a loss of files is not shown for the proof-carrying design |

How many Requests, decisions and SendSplits the marker keeps is set in §5.10.
Larger numbers protect other parties' refunds and cost a larger key-store
write for each signed object. Owner decision (§12).

**Stopped.** A wallet is stopped only when the key store has answered and the
device key or the marker is gone. Nothing on the phone then shows which state
is the latest, and no flow gives the balance back: any path that did would let
an ordinary user back up, pay, remove the marker and restore. The cases found:

- The device key is gone. The phone was erased or reset. On Android the app
  was uninstalled without keeping its data, or its storage was cleared; the
  app's manifest settings are chosen to make either hard to do by accident
  (§5.10; not tested). On iPhone a restore after an erase leaves the key
  dead. This is T6: the value is held by one key in one phone, and what
  destroys that key destroys the value.
- iPhone: a whole-phone backup restored onto the same phone without an erase.
  What that does to the marker and to the device key is not documented and
  not tested. If it removes the marker and leaves the key, the wallet is
  stopped; the evidence gate records which (§10.4).
- iPhone: the passcode was removed or reset. Apple documents that the items of
  the marker's keychain class are then discarded. The device key may still
  sign. The app requires a passcode at enrollment and says before the first
  load that removing or resetting the passcode puts the balance out of reach.
  T6 states it as a condition on the holder. A second anchor for this case is
  described in §5.10 and is not switched on; its properties are untested.
- Android 12 to 14: removing the screen lock deletes key-store entries of one
  kind, by source reading. §5.10 chooses a marker form that survives it, and
  a test of the evidence gate decides it per tuple. A tuple that fails is not
  supported.
- At a sync the issuer shows a transition signed by this device key above the
  checkpoint's sequence number. The wallet decides on the transition and its
  own signature on it, not on the issuer's word. It means the key store went
  back to an earlier state, which T2 rules out.

The holder of a stopped wallet can leave it as it is or declare the loss.
Where a scheme enables recovery insurance, a claim is the only way to any
money (§7.3). §5.10 is built so that a wallet is never stopped on a phone on
which T1 to T6 hold. The evidence gate tests that per tuple (§10.4).

The existing attested suite does not tell a failed read from an absent item.
Its Android key store uses the alias lookup
(`kotlin/client-android/src/main/java/org/hyperledger/iroha/sdk/crypto/keystore/KagemushaAndroidHardwareAppKeyStoreV1.kt:61, 81, 164`).
Its iPhone key store reads every keychain error as "no key" and every failure
to open the key as a lost key
(`IrohaSwift/Sources/IrohaSwift/KagemushaAttested/KagemushaAttestedHardware.swift:67-78`).
The wallet core of §9 does not copy either.

The flows:

- **Migrate** (replacement or key rotation, both keys alive). The balance
  moves to a new device key under the same account.
  1. The new device enrolls and holds its certificate and receipt (§7.1).
  2. The old wallet syncs. It then asks the issuer whether a Migrate to the
     named new device id would be accepted. The issuer makes every check of
     step 4 except the Migrate's own signature, and signs its answer. On a
     refusal the old wallet signs nothing. The step is there so that the old
     journal is not closed by a transition the issuer then refuses. The
     issuer accepts a Migrate it answered yes to, unless a hold has reached
     either row in between.
  3. The old key signs `Migrate` as its terminal transition. It names the new
     device id, the whole balance, the redeemed total, the old journal's day
     and month counters, and the Requests it carries (below). The new key
     signs the same request. The old wallet does not sign while one of its own
     Requests is still open for new payments (§5.2). The Migrate is committed
     like any signed object and is sent to the issuer by either phone. Until
     the issuer has accepted it the balance is in neither wallet. The old
     wallet keeps its key and its marker and sends the Migrate again at every
     open. After the Migrate the old wallet signs nothing, whether or not the
     issuer answers. Putting older files back on the old phone changes
     nothing: its marker holds the Migrate.
  4. The issuer accepts it only if the registry binds both device ids to one
     account, the new key has signed the same request, the old journal
     extends the acknowledged head by replay or by a resume record,
     neither row is held, and the new row has not already taken over
     another row's balance by Migrate. A wallet folds one countersigned
     Migrate and no more (§5.10), so a second Migrate into the same device
     id could never be folded and its balance would be lost. The issuer
     then retires the old id, records the succession and the Migrate's
     redeemed total in the registry, and countersigns. It
     keeps the countersigned Migrate and returns it to the new device key on
     request, with no time limit. A load of the old device id whose voucher
     the old journal, or the resume record, does not show folded can no
     longer be folded by anyone: the Migrate is the old key's last
     transition, and a voucher names its device id. When the issuer accepts
     the Migrate it signs the void statement of §7.1 for each such load, and
     the load is refunded to the bound account. This is the refund that
     §8.2 takes off the old row's loaded total before the rest moves.
  5. The new wallet folds the countersigned Migrate exactly once
     (`MigrateFold`). A flag in its marker records that it did, so a resume
     cannot fold it again (§5.10).
  6. The old wallet deletes its key once it has verified the countersignature
     and the issuer has shown the retirement final. It does this itself at
     its next open. Until the key is deleted, whoever controls the old phone
     and has taken over its operating system can sign above the Migrate. That
     is evidence, and the hold then falls on the successor's row, because the
     balance moved there (§8.2). It needs T2 to fail on the old phone (§3.2).

  Requests carried. A payer may hold a Payment for one of the old wallet's
  Requests that the old wallet never decided. The old wallet can no longer
  answer it. So the Migrate carries the Requests that the old wallet can show
  are undecided: those in its journal, where the journal chains to its
  checkpoint, that have no Outcome. The Migrate carries the digest of each
  such Request (§5.1). The successor answers `Refused` for a Payment
  addressed to one of those Requests, and for those only, with an Outcome that
  names the countersigned Migrate. For any other Request of the old wallet it
  signs nothing. The limit matters: if the successor refused a Request that
  the old wallet had credited, the payer would refund a payment that was
  received. After a resume the old wallet can show as undecided only the
  Requests in its checkpoint's list and those made since the resume. How a
  payer checks such an Outcome is §5.2.
  The size of the list is not estimated: a receiver whose customers often
  walk away carries one digest for each unpaid Request.

  What a Migrate leaves behind. The old wallet's own SendSplits with no
  stored Outcome are abandoned: a `Refused` Outcome that arrives later cannot
  be refunded, because the RefundFold would sit above the Migrate. The app
  lists them with their total before the user confirms, and the holder can
  first show each Payment to its receiver again. These are payments that did
  not complete. Carrying them to the successor is possible and not designed;
  owner decision (§12). A payment the receiver credited is unaffected.
  RedeemSplits signed before the Migrate stay payable (§8.2). The old phone
  still returns stored Outcomes while it has its files.

  What moves. Without a proof the migrated amount is what the old key signed.
  The issuer replays the old journal, or, after a resume, checks the resume
  record and cannot replay the gap. Under the proof-carrying design the
  Migrate and the MigrateFold are proven transitions (§2.1).

  A Migrate needs a new enrollment. After a scheme is closed to loads the
  ledger still registers a device as the successor of a Migrate, for an
  account that already has a live row (§5.11). A holder whose phone is
  failing can therefore still Migrate, or unload.
- **iPhone whose App Attest key died** while the payment key and the marker
  survive. Apple documents that an App Attest key does not survive a
  reinstall or a restore. Offline the wallet pays and receives as before; the
  payment key is not affected. Before its next sync it needs one online
  re-attestation. A new App Attest key is attested over a transcript holding
  the device id, the payment public key and the head, and the payment key
  signs the same transcript. The issuer submits it as a renewal; the
  validators verify Apple's chain and replace the App Attest facts in E
  (§8.1). Device id, balance and marker are unchanged. The certificate serial
  advances, so the wallet receives a new certificate and receipt.
  - It shows a live instance of the app and possession of the payment key. It
    does not show the same device.
  - The wallet asks for it only when it is ready or has just resumed, that is,
    when a valid marker is present. The journal need not be. With the state
    in the marker, a backup restored to the same phone, which kills the App
    Attest key and puts older files back, leads to a resume at the current
    state and not to an earlier balance.
  - The wallet treats the App Attest key as dead only when the platform
    reports the key itself as invalid, not on a failure to reach Apple or any
    other error. The re-attestation deletes nothing, so a wrong trigger costs
    one rate-capped call.
- **Declared loss and retirement.** The wallet never concludes by itself that
  it is lost. The user says so. The cases are a new phone with no old phone, a
  phone that was reset, and a stopped wallet. A waiting wallet is not declared
  lost on the same phone: a failed read may pass.
  - The issuer and the chain cannot check the statement, which is why it
    restores nothing. The app first shows what it found on the phone and what
    is given up: the offline balance of the old device id, unless that phone
    turns up again. If the user still has the old phone working, the app
    offers a Migrate instead.
  - The new wallet enrolls a new key under a new device id (§7.1), and the
    bound account retires the old id on-chain with `RetireKagemushaDevice`.
  - What retirement by the account does. The row takes no Load and no longer
    counts against the per-user device cap. Nothing else. It emits no block
    entry and has no effect offline. If the old phone turns up, it pays,
    receives and unloads as before, and its next certificate, requested with
    its device key's signature, makes the row live again. The rule is for a
    declaration that turns out to be wrong, and for an account key in someone
    else's hands: whoever holds the account key cannot stop a live phone by
    retiring it.
    One consequence: each declared loss that is followed by a return leaves
    the account with one device more than the cap.
  - Blocking at the holder's request. A scheme may switch on, as part of R6,
    an instruction by which the bound account blocks its own device id
    (`RequestKagemushaDeviceBlock`). Holders of the list then refuse the
    device, and it does not renew, until the account clears the request. That
    lets whoever holds the account key stop a live phone paying among holders
    of the list. Whether it is wanted, and whether it counts as a regulatory
    control, is the owner's decision (§12). With it off, a lost or stolen
    phone that can be unlocked can be spent by whoever holds it, as cash can.
  - The wallet deletes the old key and the old files of a wallet declared
    lost only after the retirement is final and the user has confirmed a
    second time. That deletion is tidying. It has no security function.
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
  - Renewal. The issuer knows the counters at the head: from the journal, or
    after a resume from the last transition in the resume record, which
    carries them. The `Recertify` carries the day and month counters of the
    transition before it. A new certificate never resets them. If the new
    share is below what the counters already show, the wallet sends nothing
    more in that window.
  - Moving share between two live devices of one account. The issuer raises
    one device's share only after it holds the other device's `Recertify` to a
    certificate with the lower share. A head declared in a sync request is not
    enough (§5.3).
  - Migrate. The `Migrate` transition carries the old journal's day and month
    counters. The `MigrateFold` adds them to the new journal's counters where
    the day index is the same, and separately where the month index is the
    same. The new wallet signs the `MigrateFold` at a time no lower than the
    Migrate's `device_time_ms`: for the rule of §5.4 the Migrate counts as the
    new wallet's last signed time. A clock set back on the new phone therefore
    cannot open an earlier window. The issuer then renews the new id's
    certificate with the old id's share.
  - After a lost phone. The old id's journal above the acknowledged head is
    unknown. The old certificate's share stays counted against the account
    for as long as that certificate can send: until the end of the UTC day,
    and for the month limit the end of the UTC month, in which `not_after +
    expiry_grace` falls. Until then the new certificate carries only what was
    not allocated, which is nothing if the lost phone held the whole limit.
    An issuer that keeps part of each account's limit unallocated lets a
    re-enrolled wallet send at once, at the price of a lower usable limit
    before the loss. If the old phone turns up after its share was given
    away, its next certificate carries what is unallocated then.
  - After a lost phone whose certificate is `Never`. There is no such date,
    and retirement stops nothing offline. The scheme chooses one of two rules
    in the tier row. Never return the share: the account's usable limit stays
    reduced by the lost device's share. Or return it after a set period: from
    then on the account can send up to the old share per window through the
    old phone, on top of its limit, if that phone still works. Owner decision
    (§12).

  Who checks. The wallet's core applies the counter rules. The issuer checks
  them by replay at the next sync, outside any gap, and it alone enforces the
  split across an account's devices. A receiver sees one certificate and its
  tier row; it cannot see the account or its other devices. The chain can
  check only that a device id is registered to the account and tier. The most
  that a faulty issuer or a stolen certificate key can give one account is
  therefore the tier row's limit times its registered device ids. The
  receiver's tally (§5.4) is keyed on the payer's device id, so it starts
  again after a Migrate. Each Migrate needs a new enrollment, which the device
  and registration caps bound (§8.1). The limit of §5.4 stands: among wrong
  clocks an expired certificate still sends, and the dates above move with it.

### 7.3 Recovery is insurance

An authentic journal need not be the latest: a journal at balance 100 verifies
even if a later transition paid that 100 away, and a dead key cannot sign that
nothing later exists. A journal copied to another phone is indistinguishable
from a journal whose key died. On a phone on which the assumptions hold, one
thing shows that a state is the latest: a marker still present on the phone.
Where it is present the wallet resumes (§7.2) and nothing needs insuring.
Where the key or the marker is gone, a claim that the balance was not spent is
a claim nobody can check. Paying such a claim is insurance. It is no part of
the argument for any property of §1, and no payment's security rests on it.

Recovery is off by default. The default scheme sets `recovery_cap` to zero,
and a lost wallet is then a lost balance. A scheme may switch it on. The
insurer is whoever pays cash into the scheme's recovery account. This document
does not say who that is.

Rules:

1. The recovery account is separate from the pool and funded in advance, by
   `FundKagemushaRecovery`. A claim is paid only from the cash in that
   account. No instruction moves cash from the pool to it or pays a claim from
   the pool. The rule is there because a false claim is open to an ordinary
   user (below): paid from the pool, false claims would make the pool short
   while every assumption held.
2. On-chain scheme parameter: `recovery_cap` per account per period.
3. A claim is an on-chain instruction signed by the account bound to the
   device id. One paid claim per device id. A held row accepts no claim. A
   row retired by Migrate accepts none, because its balance moved. A row
   retired by its account accepts one, like a live row.
4. A claim changes nothing about the device id. Neither filing nor payout
   retires, blocks or holds it. The registry row keeps accepting Load and
   Unload, no block entry is emitted, and a phone that still works is
   unaffected online and offline.
5. Reference head H: the head of the presented journal, which must extend the
   acknowledged head; with no journal, the acknowledged head itself.
6. Amount = min(remaining cap, cash in the recovery account, balance at H
   minus any outflow by that id above H that has reached the issuer or the
   chain by payout). The issuer computes the amount and signs it. The chain
   checks the account's signature, the issuer's signature, the cap, the
   account's cash and the one-claim counter. Neither can check that the
   balance at H still exists.
7. A journal above H uploaded by the device key before payout ends the claim:
   the wallet is in use. An optional `recovery_delay` before payout, which may
   be zero, gives time for that and for outflow above H to arrive. It suspends
   nothing. It is not a bound: under R5 no receiver has to sync within it.
8. Payout is on-chain to the bound account, booked against the device id in
   the row's recovery counter. It is never re-issued as offline balance and
   never counted in the row's loaded total or paid total (§8.2).
9. A claim is refused when the cap for the period is used up or the recovery
   account is empty. It may be filed again later. A claim is not a payment of
   §1, and §8.4 does not apply to it.

What this costs the insurer:

- A payout takes nothing back. If the phone still works, its balance stays
  spendable and redeemable in full, and the account has been paid twice. The
  second payment is the insurer's cost. The protocol has no way to recover it.
- An ordinary user with the unmodified app can do that on purpose: claim, keep
  the phone, keep spending. One paid claim per device id and the cap per
  account per period bound it for one account. A Migrate gives the account a
  new device id and so another claim in a later period. An account costs a
  keypair unless it is bound to a verified identity (§8.1).
- The insurer should therefore expect every unit it pays in to be drawn. An
  unmodified phone can take `recovery_cap` on every account in every period
  until the account is empty.
- Honest claims draw on the same cash as false ones. When false claims empty
  the account first, an honest claim is refused until it is funded again.
- Whoever holds the account key can file a claim and is paid on that account.
  That takes from the insurer and not from the phone.

What it does not cost: the pool. A true claim leaves the lost balance
unpresented for ever (Λ in §8.3). A false claim leaves the balance where it
was, with pool cash behind it as before. Either way pool cash and the claims
on it are unchanged.

## 8. Ledger integration

### 8.1 Device registry, witnesses, and what they bound

**The row.** The registry holds one row per device id. A row is never deleted:
a claim or a fee can arrive after any delay.

| Field | What it holds | Written by |
|---|---|---|
| Device key and device id | The P-256 public key and its hash (§5.1) | Registration |
| Bound account | The account that registered the device. No instruction changes it | Registration |
| Tier | The registered tier | Registration; a renewal that changes the tier |
| Status | Live, retired or held. Retired has two forms: by Migrate, with a successor and a final redeemed total, which is permanent; and by its account, which ends when the device next asks for a certificate (§7.2) | Retirement; evidence; renewal |
| Enrollment statement E, and its digest | Below | Registration; each renewal that carries evidence; an iPhone re-attestation |
| Sealed at | The height of the block whose registry root holds the row's current leaf | Registration; each renewal |
| Last serial | The highest certificate serial recorded | Registration; each renewal |
| Load counter | The number of loads of this device id. Its value at a load is that load's voucher number | Each load |
| Loaded total | Loads less refunded loads | Load; load refund; Migrate |
| Claimed total, paid total, release period, queue entry | §8.2 | Unload |
| Final redeemed total; successor id | Rows retired by Migrate only | Retirement by Migrate |
| Acknowledged head and its sequence number | §7.1 | Head anchor |
| Fee payout total, settled-fee set, fee payouts in the current unload window | §5.8 | Fee settlement |
| Recovery counter | Whether a recovery claim was paid for this id, and the amount | Recovery payout |
| Hold record | The two signed objects that placed the hold | Evidence |

Registration carries the issuer-signed authorization and is capped by a
scheme-wide per-window per-tier cap. Where the asset has an issuer-attested
retail identity, the per-user device cap is keyed on that identity.

**The enrollment statement E.** §5.1 defines the object and §2.1 says how a
proof uses it. The row holds it in full. It says what the vendor's hardware
attested about the phone and the app, and under which policy entry.

| Field | Android | iPhone |
|---|---|---|
| Attested key | The device key | The App Attest key, and the device (payment) key that its attestation names. Apple certifies the App Attest key, not the payment key (§4) |
| Platform class | Android under a remotely provisioned chain, or Android under a factory-provisioned chain | iPhone |
| Security level | TEE or StrongBox | Not attested. Apple documents the App Attest key as held in the Secure Enclave |
| Boot state | Bootloader locked; boot verified; digest of the verified-boot key | No field exists |
| Patch levels | OS, vendor and boot patch levels | No field exists |
| App identity | Package name, version and signing-certificate digest, as the operating system reported them | This scheme's App ID; the production environment; from iOS 27 the launch category and the bundle version |
| Key that signs renewal evidence. Not a field of E: the row holds it beside E, and on Android the certificate carries it (§5.1) | The public key of the app attestation key, where the phone has one | The App Attest public key, which E names by its identifier, and the last assertion counter the validators saw |
| Policy entry | The entry the evidence was checked under | The same |
| Serial commitment | A commitment to the serial numbers of the vendor certificates, so that they can be tested against a later revocation list | The same commitment (§5.1). Nothing is tested against it: Apple publishes no revocation list |
| Enrollment epoch | The ledger height at which the validators checked the evidence, at registration or at the latest renewal that refreshed E | The same |

A class whose attestation says nothing about the operating system, such as
HarmonyOS NEXT, would have an E without boot or patch fields. No wallet is
designed for it (§10.2).

**The policy table.** §5.1 defines the policy entry. The table is on the
ledger and append-only. A registration or a renewal is checked under the
newest entry in force, and E names that entry. Each entry names the root of
the set of vendor serials revoked at its date; the registry authority adds
serials to that set between entries and can remove none (§6). The pool's claim
about phones is that of the weakest class any entry admits. Whether iPhones,
phones with factory-provisioned attestation keys and HarmonyOS NEXT are
admitted to a pool that claims the OS constraint is the owner's decision (§2,
§12). How far a list of tuples can be enforced at registration differs by
platform. On Android the validators read the patch levels and the boot state
from the evidence, and the model only if the vendor attests it. On iPhone the
evidence names no model and no OS version, so nothing can be enforced from it
(§10.4).

**What the validators check at registration.** Every validator runs these
checks while it executes the block. All inputs are on the ledger: the evidence
in the transaction, the policy entry, the revoked set and the block's time.
Every validator therefore reaches the same verdict. The parser follows the
structure of each certificate from the top and never matches a pattern,
because the challenge and the app identity are bytes the applicant chooses.
§2.1 states the same checks as the enrollment relation; the native check and
a proof of that relation must accept the same evidence.

- Android.
  1. The chain for the app attestation key verifies, link by link, up to a
     vendor root key in the policy entry. Its chain class is one the entry
     admits. Certificate validity is judged at the block's time under the
     rules of §10.3. No serial of the chain is in the revoked set.
  2. That key's attestation shows the purpose "attest key" and no other,
     origin "generated", and a security level of TEE or StrongBox.
  3. The device key's certificate verifies under the app attestation key. It
     shows purpose "sign", curve P-256, origin "generated", the same security
     level and no user-authentication requirement (§5.9).
  4. Both attestations show a locked bootloader and a verified boot; OS,
     vendor and boot patch levels at or above the entry's floor; and, in the
     list the operating system fills in, the entry's package name and
     signing-certificate digest.
  5. The challenge in both is the hash of the enrollment transcript, and the
     transcript names this scheme, the submitting account, the policy entry
     and a block hash no older than the scheme allows.
  6. The device id is the hash of the device key and has no row.

  Where the phone has no app attestation key, the device key's own chain
  takes the place of the two: check 1 applies to it, it shows the key
  properties of check 3, and checks 4, 5 and 6 apply unchanged.
- iPhone.
  1. The attestation verifies up to Apple's App Attestation root key in the
     entry. Its App ID hash is the scheme's. The environment is production.
     The counter is zero. The key id is the hash of the App Attest key. The
     nonce covers the hash of the enrollment transcript. The credential
     certificate is within its validity at the block's time. From iOS 27 the
     launch category and the bundle version are the entry's.
  2. The transcript names the payment key, the payment key's signature over
     the transcript verifies, and the two keys differ.
  3. The transcript and the device id are checked as for Android.

**The seal.** The validators' check is of use off the ledger only if something
carries its result there. Two things do.

- For a proof: the validators' seal. Each registration and each renewal adds
  a leaf to the registry root of its block: device id, digest of E,
  certificate serial. The validator quorum seals that root with the keys and
  by the mechanism that seal a mint today. A block with top-ups carries "an
  exact quorum of signatures made with separately provisioned Pasta keys"
  over a top-up root
  (`crates/iroha_core_zk/src/kagemusha_v1_recursion/mint_finality.rs:1-7`),
  and that root is a field of the block's execution commitment
  (`crates/iroha_data_model/src/sumeragi_finality/commitment.rs:141-144`). A
  registry root beside it is new (§8.5). The wallet's recursion starts from
  its leaf under a sealed root (§2.1), until the enrollment proof exists to
  stand beside it. No circuit gains non-native arithmetic from the seal.
- For a receiver's own check of the last hop: the certificate and its
  receipt. E travels in the device certificate, and the witnesses' receipt
  names that certificate (§5.1). A witness signs only after it has seen the
  write final on its own node and has checked the certificate against the
  row. So what such a receiver verifies is the issuer's signature and the
  witnesses' signatures. It does not verify the validators' seal itself.
  Carrying that seal in a Payment is possible and is not in this design. One
  validator's seal is 132 bytes in the existing encoding
  (`crates/iroha_core/src/sumeragi/attestation.rs:39`), and an exact quorum
  is 3 seals for a committee of 4 and 21 for a committee of 31. The seals
  alone are then about 0.4 to 2.8 KB, before the path and before whatever
  shows which keys form the committee. Computed, not measured.

What the seal anchors is the time of the check and the revocation data it was
made against. A proof of the vendor chain alone can anchor neither: a proof
does not know when it was made. What the seal does not anchor is anything
after the check. A phone taken over after its last renewal has a valid seal
(§3.2).

**Witness model — owner decision.** With the validators checking the vendor
evidence, the witnesses no longer decide whether a device is genuine. What is
left for them is the receipt: a short signed statement, under keys the scheme
root certifies, that the registry holds this certificate's device key, serial
and E.

- (A) *Independent verification.* Each witness runs its own ledger node. It
  signs a receipt for a certificate only after the write that records that
  certificate's serial is final there, and only if the certificate's device
  key, serial and E are the row's. At a registration it also checks the
  vendor evidence again from the ledger, against its own pinned vendor roots
  and its own fetch of the vendor's revocation list, and that the registered
  tier is one the root-signed admission policy allows for that platform
  class. What this adds to the validators' check is the independent fetch:
  the validators use the revoked set on the ledger.
- (B) *Notary.* A device id's first receipt is signed only after the issuer
  has matched the registration to its log of issued certificates and a
  seasoning delay has passed. The delay is a scheme parameter, no shorter
  than the interval at which the issuer reconciles registrations against
  that log. Under (B) every enrollment waits that long before the wallet can
  pay or request, including the new device of a Migrate and a re-enrollment
  after a loss (§7.1, §7.2). Later receipts of the same device id, at
  renewals, are signed once the write is final. An unmatched registration
  never gets a receipt; it pauses registration under that key.

Recommended: (A), with at least one witness outside the issuer operator's
administrative control. Under either model a renewal waits for k of the n
witnesses (§6).

**What is bounded, stated narrowly.** Each line is about a stolen key, so
about a failure of T4. None is part of the argument for a property of §1.
§3.2 has the consequences.

- The registry alone bounds nothing offline; receivers do not read the chain.
  What reaches a receiver is the certificate with its receipt and, under the
  proof-carrying design, a proof that starts from a sealed leaf.
- Accounts cost a keypair and at most a fee, so a per-account cap does not
  gate a key thief. The gate is the scheme-wide registration cap, which honest
  enrollment shares; a thief can also exhaust it to deny enrollment.
- What sealing E changes. One statement for each carrier above.
  - In a proof. A device key has a place in a proven history only through a
    leaf under a root that the validator quorum sealed, and the validators
    seal only a key whose vendor evidence they verified. No issuer-side key,
    alone or together with the others, gives a software key that place. That
    needs a vendor attestation for the key: a genuine phone, or a leaked
    vendor attestation key that is not in the revoked set, which is a failure
    of T1. Or it needs the seal keys of a validator quorum, which is the
    trust the mint already rests on.
  - Without a proof. A receiver that checks the certificate and the receipt
    and nothing else gains nothing from the seal: whoever holds the
    certificate key and the witness quorum can show it a software key with
    any E. The signature-only form does not carry this protection (§2.4).
- Certificate key alone. A certificate is accepted only with a receipt that
  names it, and a witness signs only for a certificate whose serial the
  ledger recorded. So the key alone makes nothing acceptable: no new device,
  no later `not_after`, no serial above a block entry.
- Witness quorum alone. Receipts for certificates that nobody signed.
  Nothing is created.
- Certificate key and witness quorum. For a registered device whose holder
  cooperates: a certificate with any terms the tier row allows, with its
  receipt. That is a later `not_after`, fresh opening counters or a serial
  above a block entry. It weakens R6, R7 and R8 for those devices with or
  without a proof, because a certificate's terms are checked by the receiver
  and are not in a proof (§2.1). For a software key: acceptance by a receiver
  that checks no proof, and nothing under the proof-carrying design, as
  above. §5.1 bounds how long what the thief signed stands once a receiver
  holds the revocation.
- Voucher key. Unbacked value minted onto genuine phones, where a wallet or a
  proof accepts the voucher key's signature as the authority for a mint. If
  the mint is authorized by the validators' seal over the load instead, as the
  existing top-up path does, a stolen voucher key mints nothing; that choice
  is fixed with the relation (§2.3). The voucher key also signs the statement
  that releases a load refund (§6), so its thief can refund a load whose
  voucher was issued and spent; the bound is that load.
- List key. No value created; honest devices can be blocked among wallets
  that take the forged segments, until the correction reaches those wallets
  (§5.5). Merge-only entries stop a forged list lifting a block.
- Registry authority key alone. This is the issuer's on-chain account.
  - It signs registration authorizations. The validators still check the
    vendor evidence, so it registers only keys that a genuine phone attested.
    One phone can attest many keys, so the thief can fill the registration cap
    with rows for its own phone, and deny enrollment to others.
  - Such a row has no certificate, so no honest receiver accepts a payment
    from it. Its key can sign a RedeemSplit. Where an unload needs no proof,
    the chain cannot tell how much that key holds, and the row is paid as
    §8.2 says: at `unload_limit` per window above its own loads, or in full
    at once where the scheme sets no limit. Where an unload must carry a
    proof, the row has no proven value to redeem.
  - It submits renewals. It cannot pass renewal evidence the phone did not
    make.
  - It adds serials to the revoked set. Leaving a revoked serial out lets a
    leaked vendor key through at registration. Adding honest serials makes
    renewals fail, and with R8 on those phones stop sending at their lease
    end until a new policy entry replaces the set.
  - It cannot anchor a head the device did not sign, or a sequence number
    below the row's, because the chain checks both (§6).
  - It cannot retire a row without the device's signed Migrate or the bound
    account's signature. Retirement by an account stops nothing offline
    (§7.2). It supplies a retired row's final redeemed total; set too high,
    that leaves the row's unload door as open as a live row's and no wider.
  - It cannot refund a load without the voucher key's statement.
  - It submits fee settlements but cannot forge the device signatures in
    them.
  - It cannot place a hold. A hold needs two conflicting signatures by one
    device key, and falls on that key's row or on the row that took over its
    balance by Migrate (§8.2).
- All issuer-side keys together, without the voucher key. Rows, certificates
  and receipts for phones the thief holds, on any terms the tier table
  allows. Those are enrolled phones. Under the proof-carrying design value is
  created only if one of them is also compromised, or with a leaked vendor
  attestation key (§3.2). Without a proof, software keys are accepted as
  above.
- A validator quorum's seal keys. A seal over any statement. This is a
  failure of the ledger itself, and of T4 as §4 words it.
- Root key. Its thief signs notices and issuer key certificates that peers
  accept offline without the chain, and nothing revokes the root. Peers stop
  accepting the stolen key for later epochs only when they hold the succession
  to the committed next root key, which the thief cannot forge (§5.11). A
  policy entry or a scheme cell signed with the stolen key takes effect on
  the ledger only if ledger governance installs it; as a notice it reaches
  wallets peer to peer without the ledger. What a root compromise voids is
  not decided (§12).
- An account that adds cash to the pool, and the insurer's account. Each can
  only pay in.

### 8.2 Unload

- What an unload can and cannot establish. Nothing on-chain can show that a
  balance was not also paid away offline. A device that submits a redemption
  again earns nothing, because the chain pays only the increase in its
  cumulative total. On phones on which the assumptions hold, a wallet signs a
  RedeemSplit only for value it holds and debits itself when it does, so total
  claims never exceed total loads (§8.3). A phone on which they fail can
  redeem value it also paid away, and is paid.
- What the chain checks. To record a claim the chain checks the RedeemSplit's
  device signature against the key in the registry row, and the row's status
  as set out below. Under the proof-carrying design a RedeemSplit is a proven
  transition like any other (§2.1), and the chain can verify its proof with
  the node verifier that exists today and rejects everything while no release
  keys exist (§2.2). Whether the chain requires that proof is fixed with the
  relation (§2.3, Q0). Each answer has a cost.
  - Required. A recorded claim is then bounded by a balance that was reached
    from loads by valid transitions signed by enrolled keys. It still does
    not show that no other branch of the same state exists. A phone that
    cannot make the proof cannot unload, and its balance has no way out: a
    phone whose prover does not finish, or one that lost its proof material
    with its files (§7.2). A server could make that proof from the wallet's
    proof material and the signed RedeemSplit. That is not designed, and it
    shows the server the wallet's state (§2.3).
  - Not required. The chain then checks a signature only, and a key can
    record a claim of any size. A phone that cannot make the proof can still
    unload only if its wallet may also commit a RedeemSplit without a proof.
    §5.2 does not allow that, and no such exception is designed.

  Until the proof qualification ends (§2.3), the signature check is all that
  can be built and tested.
- What the chain does not check. The ledger does not refuse an unload because
  of the issuer, the certificate, its lease, the phone's clock state, a key
  revocation, a stale list, or anything about a device this row was paid by.
  The wallet signs a RedeemSplit in each of those states (§7.1), so that a
  wallet which cannot pay offline can still unload.
- Payee. An unload pays only the account bound to the device id in the
  registry. The RedeemSplit names no payee, and anyone may present it. The
  registry has no instruction that changes a row's bound account. A holder who
  loses the account key can still pay offline. Its unload payouts go to an
  account it no longer controls, and it cannot Migrate. A governed rebinding
  would be a theft path for whoever controls it; owner decision (§12).
- **Recording.** A valid RedeemSplit is recorded in full: the row's claimed
  total becomes its `redeemed_total_after`, if higher. No parameter caps what
  a row may record, and no pool condition refuses a claim. A recorded claim is
  never reduced or cancelled.
- **Release.** The part of a row's claims that keeps its paid total at or
  below its own loaded total is due at once. This part is the holder taking
  back what it loaded. The part above the row's own loads is value the row
  received from others. How fast it is released is set by `unload_limit`, a
  scheme parameter. Its value is an amount per unload window, or no limit.
  The unload window is a scheme parameter measured in ledger time. The value
  is the owner's choice, and this document does not choose it. The two costs:
  - With a limit. A row is released at most `unload_limit` per window above
    its own loads. That slows honest receivers while every assumption holds.
    A merchant who loaded nothing and took 10,000 from honest payers is paid
    `unload_limit` per window, while a holder who loaded 10,000 is paid at
    once. A merchant whose net takings per window exceed the limit falls
    further behind every window. Value in a recorded claim is no longer
    spendable offline, so the wallet proposes one window's worth at a time
    (§7.1). The limit is a residual-risk control (§3.2). While the
    assumptions hold it does nothing except delay.
  - With no limit. Every claim is due at once. Honest receivers are not
    slowed. A row whose key signs a RedeemSplit for more than it holds is then
    due the whole amount at once: without a proof that is any amount the key
    signs, and one such row can take all the cash in the pool in one
    instruction. Where an unload must carry a proof it is at most the row's
    proven balance, once for each branch of a fork.

  A tier may carry its own value in the tier table; that is part of the same
  choice. Precisely, with a limit: let `own = min(claimed, loaded)` and `above
  = claimed − own`. The row keeps a release period `(start, base)`. In window
  `w` the released amount is `own + min(above, base + unload_limit × (w −
  start + 1))`, and the amount due is the released amount less the paid total.
  A claim recorded in a window later than `start`, when everything recorded
  earlier was already released by the end of the previous window, starts a new
  period: `base` becomes the earlier `above` and `start` becomes `w`. The
  new-period rule is there so that unused allowance does not carry over: a row
  is released at most one `unload_limit` per window however its claims are
  split or timed. A change to `unload_limit` applies from the next window and
  never takes back what was released.
- A row's release dates depend only on that row's own claims and on
  `unload_limit`. They are known when the claim is recorded and change only if
  the parameter does. No cap is shared between rows, so the number of other
  rows that claim does not change them. A cap shared by all rows would bound
  the total that leaves the pool in a window, and would make an honest row's
  wait depend on other rows.
- The ledger does not use the witnesses' receipt. A rule that paid a row
  above its own loads only while a receipt for it was recorded on the ledger
  would be a second lock against a stolen registry authority key. With the
  validators checking the vendor evidence it would add a lock only under
  witness model (B), and after a witness-key revocation it would pause the
  payouts of honest rows until new receipts were recorded. It is not in this
  design. The owner may ask for it (§12).
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
  shortfall, and it is public. A place in the queue, once taken, is never
  lost: rows that claim later, however many, cannot pass it. Held rows are
  passed over (below). §8.4 has the rest. On phones and keys on which the
  assumptions hold, the pool is never short (§8.3).
- **Holds.** A hold is a status of one registry row.
  - What places it. Two objects signed by one registered device key that
    satisfy a predicate of §5.3, for example two different transitions at one
    sequence number. The chain checks both signatures against that row's key
    (`SubmitKagemushaEvidence`). Nothing else places a hold. The issuer cannot
    place one on its own judgment. Evidence that shows a fault of an
    issuer-side key, such as a certificate and receipt with no registry row or
    a MintFold whose voucher matches no load, is recorded and places no hold
    on the phone that holds the object.
  - Which row. The row of the key that signed both objects. If that row was
    retired by Migrate, the live row at the end of its succession, because the
    balance moved there (§7.2). A hold therefore reaches only a holder whose
    own key, or whose own earlier key, signed two successors. It never reaches
    a row because of a device that paid it.
  - What it does on the ledger. The row accepts no Load and records no new
    claim. Its recorded claims are kept and are not paid while the hold
    stands. Its renewals are refused (§7.1), and no fee is settled for it
    (§5.8). Its queue entry leaves the queue: no cash is set aside for it, the
    shortfall falls by its amount, and the entries behind it move up.
    Otherwise cash set aside for a held entry would keep other rows waiting
    while the pool has cash. What a hold does offline is §5.5.
  - What it does not do. It changes no value that the row paid to others, and
    it delays no other row (P4).
  - When it can happen. A phone on which T1 to T3 hold never signs two
    successors of one state (§5.10), and a tuple that fails the power-cut
    tests of the evidence gate is not supported. So a hold reaches only a
    phone on which an assumption failed: taken over by its holder, taken over
    by someone else's malware, or faulty. The chain cannot tell these apart.
  - What ends it. Nothing in this design yet. What the holder gets back, and
    who may order it, is an owner decision (§3.2). `ReinstateKagemushaDevice`
    is defined with it. Until it is, a held balance stays held.
  - What a hold is not. It is not what makes any payment secure. It is the
    one ledger event that stops a row whose key is shown to have forked from
    drawing on the pool, and it needs the two signed objects to reach the
    chain. A key that signs one RedeemSplit for more than it holds has signed
    no second successor, and no hold follows from that.
- **Retired rows.** A row retired by Migrate records a claim only up to its
  final redeemed total, which is fixed when it is retired: the redeemed total
  in the Migrate the issuer accepted. The rule is there so that a RedeemSplit
  signed before a Migrate and presented after it still pays, and so that a
  migrated key cannot go on recording claims. A RedeemSplit above that total
  is evidence (§5.3). A row retired by its account has no final redeemed
  total. It records and pays claims as a live row does, so that a phone
  declared lost by mistake can still unload everything it holds (§7.2).
- A row whose bound account is blocked on the ledger records claims as any
  other row. Whether the payout reaches a blocked account is decided by the
  ledger fact behind the R6 list (§12). The claim stays recorded either way.
- On Migrate, after any refund under §7.1, the old row's loaded total less its
  claimed total, if positive, moves to the new row; otherwise a user who
  changes phones would lose the right to take back own loads at once.

**Worked example.** The unload window is one day. Pool cash is 50,000 at the
start of day 1, all of it from loads. Merchant M loaded nothing, was paid
2,500 by honest payers, and records 2,500 on day 1. Holder H loaded 800 and
records 800 on day 3. Row X loaded nothing, and its key signs a RedeemSplit
for 1,000,000 on day 1, with no proof. X can exist only where T1, T2 or T3
failed on that phone, or where the row was registered under a stolen registry
authority key.

With `unload_limit` at 1,000:

| Day | Released to X | Released to M | Released to H | Pool cash after the day |
|---|---|---|---|---|
| 1 | 1,000 | 1,000 | | 48,000 |
| 2 | 1,000 | 1,000 | | 46,000 |
| 3 | 1,000 | 500 | 800 | 43,700 |

- M is paid in full on day 3. M waited two days, and would have waited them
  with no X at all. That wait is the cost of the limit.
- H is paid at once, because H takes back its own loads.
- X takes 1,000 a day. Nothing on the ledger stops it: one RedeemSplit that
  claims too much conflicts with nothing the chain can see, so no hold
  follows. If nothing else moves, the pool is short on day 47.

With no limit:

- Without X, M is paid 2,500 on day 1 and H 800 on day 3, each at once.
- With X, and X presented before M on day 1: X is paid 50,000 and 950,000 is
  queued. M's 2,500 joins the queue behind it, and H's 800 behind M. The
  shortfall is 953,300. M and H are paid only after 950,000 of cash has
  reached the queue ahead of them.
- With X, and M presented first: M is paid 2,500 at once, X is paid 47,500,
  and H's 800 queues behind X's 952,500.

Where an unload must carry a proof, X's RedeemSplit is refused, and neither
case arises in this form. A forked phone can still redeem each branch's
proven balance.

### 8.3 Exposure model

Let L be total loads less refunded loads, P redemptions paid, F fee payouts
paid, and A the cash added to the pool by `FundKagemushaPool`. Let C be the
counterfeit that has entered liabilities: value with no load behind it that a
rule-following wallet accepted, that was recorded as a claim, or that was
charged as a fee. Let Λ be value that will never be presented: balances on
phones that are lost or never used again, payments that were debited and never
delivered, and fees never settled. Recovery payouts are not in the model: they
are paid from their own account (§7.3).

- Pool cash is what came in less what went out: `L + A − P − F`.
- Liabilities are wallet balances, recorded unpaid claims and fee claims not
  yet paid. Loads created L of them and counterfeit added C. Each unit paid as
  a redemption or a fee removed one, and Λ will never be asked for. So
  liabilities that will be presented are `L − P − F + C − Λ`.
- Pool cash less those liabilities is `A − C + Λ`. Every claim is covered if
  and only if `A + Λ ≥ C`.

**Under the assumptions C is zero.** If T1 to T5 hold for every enrolled phone
and every issuer-side key, then every unit in every wallet came from a load
and no unit is in two wallets. That follows from P2 for every payment, from
the released app's arithmetic (T3), from the once-only rules for vouchers and
for the countersigned Migrate (§5.10), from the issuer signing a voucher only
for a load that is final, and from the ledger being final and running these
rules (T4). Pool cash then covers every claim with A at zero, and nobody has
to add anything. No loss rule and no outside funding
is part of that argument. The rest of this section, and §8.4, describe what
the ledger shows and does after an assumption has failed. That is residual
risk, and §3.2 treats it.

What can be observed on the ledger: L, P, F, A, pool cash, each row's loaded,
claimed and paid totals, the amounts due and queued, and the shortfall. Also,
from E: how many rows each platform class and chain class has, each row's
patch levels at its last check, and how long ago that was.

What cannot be observed: C, Λ, the balance of any wallet that has not synced,
the balance of a wallet inside a gap, and how many receivers a compromised
phone has reached. Where an unload needs no proof, the total of recorded unpaid
claims is not a measure of liabilities, because a device key can record a claim
of any size (§8.2). The figures from E show which rows a published flaw could
affect. They do not show which rows are compromised.

1. **No on-chain early warning.** Pool cash minus `(L − P − F)` equals A
   whatever C is, so no on-chain ratio shows counterfeit. Redemptions plus fee
   payouts exceed loads only after counterfeit exceeds Λ plus every balance
   still outstanding. Counterfeit consumes cash that backed genuine balances
   long before that. In today's code the state cannot occur at all; a
   redemption above loads is rejected.
2. **Where C can enter**, once an assumption has failed, and what each party
   can apply from its own inputs. §3.2 says what bounds each.
   - Through receivers. A compromised phone pays honest wallets, and each of
     them later redeems or pays onward. A receiver applies the payer's limit
     against its own tally (R7), the certificate's lease against its own time
     (R8) and its own copy of the block list (R6). The protocol does not limit
     how many receivers one phone reaches.
   - Through unload. A row whose key signs a RedeemSplit for value it does
     not hold. The ledger applies `unload_limit` per row and window, if the
     scheme set one, and a hold once two signed successors reach the chain.
     Where an unload must carry a proof, it also applies the proof.
   - Through fees. A payer row and a second device that sign payments that
     never happened, to a beneficiary in the root-signed table. The ledger
     applies `fee_limit` per payer row and window (§5.8).
   - Through a stolen voucher key, where a mint rests on that key (§8.1).
3. **Rate.** No pool-wide cap exists (§8.2). With `unload_limit` set, the most
   that can leave the pool above devices' own loads in one window is the
   number of rows times the limit, plus fees within their cap. Lowering the
   limit slows honest net receivers by the same factor. With no limit there
   is no bound on the rate.
4. **Issuer-side signal.** At a common past instant T, replay every journal
   synced after T up to T and sum the balances. Counterfeit at T is at least
   that sum minus `(L(T) − claimed(T))`. The bound stays valid, and is
   tighter, if the fees charged in the replayed journals up to T are also
   taken off the bracket. It is a lower bound only on a consistent cut: a
   ReceiveFold inside the cut pulls its SendSplit inside, and a MintFold
   inside pulls its load into L(T); a cut by device time alone can count one
   payment twice. It needs R8 on, lags one lease, and is reduced by every
   wallet not synced since T and by every wallet whose journal has a gap
   across T. Summing each wallet's last synced balance is unsound. With R8 off
   this signal does not exist.
5. **What the protocol does not supply.** The number of phones compromised at
   once (compromises are correlated: one exploit or one leaked attestation
   batch reaches many), the receivers each reaches per window, the windows
   until a block entry reaches them, and the share of receivers that never
   refresh a list. No empirical compromise rate for phone key stores was
   found. Any estimate of C rests on assumed values for these.

### 8.4 When the pool is short

This section says what the ledger does when an amount that is due cannot be
paid from pool cash. It does not say who supplies the difference. §3.2 lists
the possible suppliers and the owner's other choices, and this document
chooses none.

When it can happen. While T1 to T5 hold on every enrolled phone and key, pool
cash covers every claim (§8.3) and the pool is never short. A short pool
therefore means that an assumption failed somewhere. It need not have failed
on the phone of any holder who is now waiting.

What the ledger does.

- It records every claim in full and releases it on the same per-row dates as
  before (§8.2). No claim is cancelled or reduced. No claim is marked as
  genuine or counterfeit: pool cash is one balance, and the chain cannot tell
  one claim from another.
- It puts each amount that is due and cannot be paid into the payout queue,
  first in, first out (§8.2). Held rows are passed over. Each amount of cash
  that reaches the pool pays the queue from the front.
- It pays no fee while the queue is not empty, so a fee is never paid ahead
  of an amount a holder is waiting for (§5.8).
- Load. Either Load closes while the queue is not empty, or it stays open;
  the scheme cell holds the setting. Closed: no new holder's cash is spent on
  earlier claims, and no cash arrives from loads. Open: each new load pays the
  queue, and the new holder's offline value has less cash behind it. Owner
  decision (§12).
- It takes cash from any account through `FundKagemushaPool`. Cash paid in
  cannot be taken out again except as a payout.

What is visible. The shortfall, the three queue counters and each row's queue
entry are on the ledger for anyone to read. The wallet shows, before the user
signs an unload, what will be paid at once, what will wait, and whether the
pool is short (§7.1). Offline nothing is visible and nothing changes: no
wallet can know that the pool is short, and payments go on as before. A
received payment stays in the receiver's wallet and can be paid onward (P1,
P4).

What the ledger cannot do.

- It cannot make anyone add cash. If nobody does, the queue is paid only by
  what loads bring in, if Load is open, and otherwise not at all.
- It cannot place the shortfall on the phone that created the value. It can
  hold that phone's row once two signed successors reach the chain (§8.2).
- It cannot pay the holders in the queue otherwise than in order. Holders who
  stayed offline, as R5 entitles them to, come last.
- It cannot close the pool for good. No balance lapses, so a claim may be
  presented at any time. The pool can be closed to new loads (§5.11). It
  cannot be wound up while any loaded value may still be presented.

A scheme may require a stated amount of added cash in the pool before Load
opens; the ledger can enforce such a parameter. Whether to set one, and who
would pay it in, belong to §3.2. So does the case in which the party that adds
cash is also the issuer of the asset and adds newly issued units: the ledger
would show that as an ordinary mint into the paying account, and its cost
would fall on every holder of the asset.

### 8.5 What the existing ledger code can and cannot carry

Reusable: pool key, custody transfer, the non-signing reserve account (§6),
the index pattern (its admission is marked incomplete), the Torii command
surface, the governance proposal pattern (install-once, no rotation), and the
validators' seal over a root in the block's execution commitment, which the
mint path has today (§8.1). Also reusable is the data-model container for raw
vendor evidence. It carries the ordered Android certificates and the original
Apple attestation object, up to 128 KiB in all, and by its own header it
"performs no PKIX, KeyDescription, App Attest, Play, challenge/key or issuer
verification"
(`crates/iroha_data_model/src/kagemusha/kagemusha_platform_attestation_original_v1.rs:1-19`).

Not reusable as is, whether or not payments carry a proof:

- Request, record, receipt and planner types embed recursive fields.
- The pool holds two counters, total top-ups and total redemptions. The
  invariant `available = loads − redemptions` is enforced in the pool record,
  every receipt and the receipt chain, and a redemption above it is rejected
  whole with no record
  (`crates/iroha_core/src/smartcontracts/isi/kagemusha/kagemusha_v1_reserve.rs:256-297, 1986-1996`).
  §8.2 to §8.4 need a redemption above loads to be recorded, and paid when
  cash is there. That adds to the pool record an added-cash counter, a fee
  payout counter and the three payout-queue counters, and it changes the
  receipt layout and those checks.
- New state with no counterpart today: the device registry with the row
  fields of §8.1, the scheme cell, the policy table with its revoked set, the
  fee policy table, the block index with its account-to-device index, the
  per-row settled-fee set, and, where enabled, the recovery account. The
  scheme cell is replaced over time and the policy table and the fee policy
  table grow, which the install-once governance pattern does not allow
  (`crates/iroha_data_model/src/governance/types.rs:755-781`).
- Verification of vendor evidence in block execution. The node links P-256
  and SHA-2 (`crates/iroha_core/Cargo.toml:191, 225`). It has no P-384 or RSA
  signature check, and its only X.509 parser is a test dependency
  (`crates/iroha_core/Cargo.toml:243`). An Android chain needs ECDSA P-256,
  ECDSA P-384 and, for the older root, RSA-4096, with SHA-256 and SHA-384. An
  Apple attestation needs ECDSA P-384. A renewal needs P-256 only. Today the
  checks of §8.1 exist outside the node: in the issuer's Python
  (`python/iroha_app_attestation/`) and in the Kotlin attestation verifier
  (`kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/crypto/keystore/attestation/`).
  Writing them for the node adds dependencies to a consensus crate, and they
  must give the same answer on every validator: no network fetch, no local
  clock, the block's time only. Read from the manifests and the directory
  listings; nothing was built.
- The registry root. The execution commitment has a top-up root and a
  top-up count and nothing for the registry
  (`crates/iroha_data_model/src/sumeragi_finality/commitment.rs:141-144`). A
  second sealed root changes the commitment and the finality proof format, as
  any change to the mint path would. It is a consensus change.
- The gadgets for an enrollment proof. The repository has no P-384, RSA or
  SHA-384 gadget, so the server-made enrollment proof of §7.1 cannot be built
  from what exists. The validators' native check needs none of them.
- None of the instructions of §6 exists. The existing instructions are the
  top-up and the redemption of Recursive V1
  (`crates/iroha_data_model/src/isi/kagemusha_v1.rs:2853, 2863`).
- On a non-boundary block, a load that writes a top-up receipt under the
  existing witness tag gives a non-zero top-up count with no attestation flag,
  and the node enters recovery. So does any write under that tag that does not
  decode as the existing receipt type, which covers a reshaped unload, claim
  or funding receipt. A new tag avoids both.

Under the proof-carrying design the existing top-up path stays and gains the
missing mint-credit producer. The node verifier that block execution already
calls for a redemption
(`crates/iroha_core/src/smartcontracts/isi/kagemusha.rs:2617-2633`) verifies
the proof of an unload, where the relation requires one (§8.2). It rejects
everything while no release keys exist (§2.2).

## 9. Implementation shape

- **One wallet core in Rust**, sans-IO. The issuer's journal check and
  `iroha_core` use the same crate. The evidence gate's test app uses its
  marker, commit and recovery code, so that what is tested is what ships
  (§2.3).
- **Two pure steps.** `prepare(state, input) → (pending, bytes to sign)`
  runs the pre-check of §5.2 step 1 and builds the object. `finish(pending,
  signature, proof) → (checkpoint bytes, marker name, commit record,
  messages)` verifies the signature under the device key and normalizes it
  to low-S, and builds the checkpoint of §5.10. The core drives the seven
  steps of §5.2 through four traits the platform implements: `Signer`,
  `Prover`, `Store` and `Markers`. The order is fixed in the core: sign;
  prove, for a transition; for a credit, sign the Outcome; create the
  marker; write the commit record; delete the previous marker. Messages are
  returned only after the commit has succeeded and the previous marker is
  confirmed absent.
- **`Markers`** has four calls: create (name, bytes), read, delete and list.
  Read returns the bytes, absent or unknown. Delete returns absent or
  unknown. List returns the names or unknown. The platform maps its own
  return values as the table in §5.10 says and never reports absent for an
  error. The core checks the bytes and decides valid or damaged. The core,
  not the platform, decides what follows. The same trait stores the terms
  entry, the proof entry and the start entry.
- **`Prover`** has one call: prove (the predecessor proof, the transition,
  its signature, the witness) → proof, or failure. It is called between
  signing and marker creation and nowhere else. A failure is an error before
  the marker: nothing is stored and the state is unchanged. The core never
  writes a commit record for a transition without a proof. There is no state
  in which a committed transition waits for a proof, and no balance that is
  reported apart from spendable balance.
- **Recovery** (§5.10, V1 to V8) runs inside `open`. It runs again before
  `prepare` whenever the previous marker step did not end with a confirmed
  result. No other entry point signs or releases.
- **States.** `open` and `status` return one of three states with the
  reason: ready, waiting or stopped. Only ready permits `prepare`. In the
  waiting state every call that would sign returns "try again" and changes
  nothing. In the stopped state every such call is refused. The core
  evaluates the state at each open and each use and stores no verdict.
- **Resume.** `open` also reports a resume: the commit counters at the two
  ends of the gap, and which records were lost (§5.10). A resume is an
  event. It is not an error and not a state; after it the wallet is ready.
  The app shows it.
- **No destruction.** The core never deletes the device key, the journal or
  the current marker on its own reading of the phone. §7.2 names the two
  places where an old key and old files are deleted: after a declared loss
  and after a Migrate. The core does each only as a separate call that the
  app makes, after the final on-chain fact §7.2 requires.
- **One process at a time.** `open` takes an exclusive lock on the journal,
  and the journal stays open for the life of the process.
- **Failure contract.**
  - Before the marker is created (steps 1 to 3, and the proof entry write
    of step 4): every error leaves the wallet's state unchanged. The
    signature and the proof are discarded. A proof entry written for the
    attempt is removed by recovery.
  - Marker creation not confirmed (step 4): the wallet is no longer ready
    and recovery decides. If the marker is found valid, the wallet is at the
    new state and the object is returned as unreleased. If it is definitely
    absent, the wallet is at the earlier state and the object is gone; it
    was never released.
  - After the marker is created: nothing is discarded and nothing is thrown
    back to the earlier state. A call ends in one of three results:
    released; marker created and the commit record not yet written, in
    which case the wallet waits at the new state and writes it when storage
    allows; or committed and not yet released, because the previous
    marker's deletion is not confirmed.
  - `open` and `status` return, once the wallet is ready: the head commit's
    object, which may not have been released; every unresolved SendSplit
    that can be presented again, which after a resume is the newest two;
    the payment ids, amounts and receivers of the other unresolved
    SendSplits; and the latest RedeemSplit without a ledger receipt.
    Presenting any of them twice is harmless (§5.2).
- **What "complete" is in the API.** `handle(message)` reports a credit, and
  `pay` returns a Payment, only after step 6 is confirmed. The core reports
  "complete" from one place. For a received payment that place is after the
  receiver's proof, its commit and its confirmed marker step. For a sent
  payment it is the storing of a `Credited` Outcome; until then `status`
  reports the payment as sent and not confirmed (§5.2).
- **Proving and time.** Both the payer's and the receiver's proving run
  inside the payment, in the foreground. The repository's gate is 10 s for
  one proof at the 95th percentile; no prover exists that can make such a
  proof (§2.2) and nothing is measured. The core does not prove in the
  background after a commit and holds no shape in which it does. Shapes that
  could shorten the wait are choices for the qualification plan (§2.3); none
  is designed, and none is in this contract.
- **Public API**: `open`, `enroll`, `load`, `request`, `preview`, `pay`,
  `handle(message)`, `unload`, `sync`, `migrate`, `status`. `unload` takes
  no payee: an unload pays only the account bound to the device id in the
  registry (§7.1).
- **Test mode**: a separate testing artifact with software keys, an
  in-process test issuer and ledger, and a test scheme with its own root
  key. Its `Markers` implementation can return unknown, return damaged
  bytes, lose a creation, undo a deletion and let a creation land late, on
  command. Its `Store` can present older, missing and edited files. Its
  `Prover` can fail on command. Every cell of the crash table and every row
  of the restore table in §5.10, every branch of V1 to V8, and the
  sequences §5.10 lists are automated tests of the core. On iPhone the
  artifact uses a different bundle identifier, because before iOS 27 Apple's
  attestation cannot distinguish builds of one App ID. No build signed under
  the production App ID contains a software key store or a software marker
  store.
- **A build without the prover.** At commit `b2a3cd05bc` the mobile bridge
  links the prover into every build
  (`crates/connect_norito_bridge/Cargo.toml:42, 49`). A staged, uncommitted
  edit in the working tree removes those features from the bridge. Whichever
  lands, the bridge needs a feature that selects the prover, so that the
  evidence gate's test app and test schemes can be built without it. No build
  without the prover holds production value.

## 10. Platforms

Support is claimed per tuple and only after the evidence gate (§10.4). A tuple
is one phone model, one operating-system major version, one vendor build
family (for example a China build or a global build) and one key-store
security level (TEE or StrongBox). On iPhone the last two have one value each.
Nothing in this section has been tested on a device unless a line says so.

### 10.1 First cut

The owner named the families: "any user with a modern phone like pixel 6,
huawei, meizu, iphone, samsung, etc., that is modern and mainstream and has a
secure element". The first cut is a list of tuples that the owner fixes before
the gate starts (§12). This document proposes that the list starts with
iPhone and Pixel tuples, because those are the only two families with a
record in the repository, and that every other tuple joins it only with a
captured attestation chain and a gate finding.

What an enrollment statement can say differs by platform class (§2.1). A
pool's claim is that of the weakest class it admits, so the class is part of
what the owner decides for each tuple.

| Platform class | What E states about the operating system | What exists today | Open before any tuple of the class can be claimed |
|---|---|---|---|
| Android, remotely provisioned attestation chain | At the boot in which the key was generated the secure hardware had been told: bootloader locked, boot verified, this boot key, these OS, vendor and boot patch levels. The operating system named the wallet's package and signing digest. As of enrollment, or of the last renewal where R8 is on | Google's published test chains for Pixel 8a, 9 Pro and 9a. No chain captured by this project | A captured chain per tuple (group h). Whether the app attestation key works on the tuple, offline and for how long. The gate finding |
| Android, factory-provisioned attestation chain | The same fields. The attestation key is shared by a production batch, and such keys have leaked: Google's revocation list held 1,733 key-compromise entries on 2026-09-29. A leaked key that is not yet listed signs the same fields for a key held in software | The repository's Pixel 6 StrongBox chain, recorded as a factory chain under Google's first root (`specs/kagemusha_v1_production_readiness.md:416-419`) | The owner's policy on factory-provisioned roots (§10.3). Then as the row above |
| iPhone | No operating-system statement. Apple certified a Secure Enclave key for this scheme's App ID in the production environment, and from iOS 27 a launch category and a bundle version | One development-environment attestation and two assertions from an iPhone 17 Pro Max on iOS 26.7 (`specs/kagemusha_v1_production_readiness.md:327-342`) | A production-environment attestation on each tuple. Whether an old jailbroken device can enroll (test h7). Whether iPhones are admitted to a pool that claims the OS constraint: the owner's decision. The gate finding |
| HarmonyOS NEXT | None. Its documented attestation carries no boot, lock or patch field | Nothing | Not in the first cut (§10.2) |

Candidate tuples and what is known about each.

- **Pixel.** The repository's record is a Pixel 6 on an Android 17 user
  build, locked, with verified boot
  (`specs/kagemusha_v1_production_readiness.md:355-357`). Three facts bear on
  it. Google's update commitment for the Pixel 6 ends in October 2026, so
  under a patch floor at renewal it falls behind for good (§10.3). Its
  StrongBox chain is a factory chain, so a policy that refuses factory roots
  leaves only its TEE, whose chain class has not been captured. And the form
  of the Android marker depends on the Android release (§5.10), so each
  release a listed Pixel may run is its own tuple. A current Pixel, launched
  with remote provisioning only, is needed beside it.
- **iPhone.** The record is an iPhone 17 Pro Max on iOS 26.7. On iOS 26 the
  attestation carries no launch category and no bundle version, so beyond the
  environment the issuer cannot tell builds of one App ID apart (§9). iOS 27
  adds both fields; as read, it runs on iPhone 11 and later. The issuer
  cannot enforce a model or an iOS version from attested fields, because
  there are none; it relies on the app's minimum iOS version, which the phone
  enforces at installation.
- **Samsung and other brands' Google-certified builds.** They are expected to
  chain to Google roots, but no such chain has been captured for this
  project. Each becomes a first-cut tuple only after a captured chain and a
  gate finding. The minimum per family is one model per key-store
  implementation the family ships (TEE only, and TEE with StrongBox) and one
  per build family.

The floor is Android 12, the release the Pixel 6 shipped with and the first
with app attestation keys, and the iOS version the owner fixes.

### 10.2 Not in the first cut

- **HarmonyOS NEXT**: does not run Android apps natively. It needs an ArkTS
  shell over the Rust core, a HUKS attestation verifier and a pinned root
  identified from a real device. Its attestation carries a challenge, an
  application id, a key source and a product model, and no boot, lock or patch
  field, so E has no operating-system statement for it. A third-party app gets
  it offline only on the newest API level, under a certificate that lasts one
  month. Its marker is not designed: backup and clone are opt-in per app, and
  what they do to the key store has not been read. Whether such a phone may
  join a pool that claims the OS constraint is the owner's decision; if it
  joins, the pool's claim is the weaker one for everyone in it.
- **Huawei EMUI / HarmonyOS 2–4**: nothing captured shows what these devices
  return. Huawei models after the P30 and Mate 20 are absent from Google's
  list of certified devices. A device with no chain to a pinned root is
  unsupported, not "lowest tier".
- **Mainland-China builds of Xiaomi, OPPO, vivo and Honor**: Xiaomi, OPPO and
  vivo state in their own white papers of 2020 and 2021 that each phone is
  given a Google-issued attestation certificate at production, and mainland
  models up to 2026 are in Google's list of certified devices. No mainland
  chain has been captured. A model launched with Android 16 has no factory key
  and obtains attestation keys by remote provisioning, which AOSP's source
  allows on such a build only while Google's services app is enabled; whether
  the provisioning host is reachable from a mainland network is unknown.
  Nothing was found for Honor.
- **Meizu**: announced in February 2026 that it had suspended in-house
  hardware development of new domestic phones. Its recent models are in
  Google's list of certified devices; no primary information on its key
  attestation was found.

This narrows R2 and needs the owner's agreement. No claim can be made today
for two of the families the owner named, Huawei and Meizu.

### 10.3 Verifier and client changes

The verifier turns a vendor attestation into an enrollment statement E (§5.1).
It runs in two places: in the issuer service at enrollment and at each renewal,
and in every validator at registration and at each renewal that carries
evidence, natively, before the registry is sealed (§7.1, §8.1). Today the
checks of boot state, lock state, patch level and app identity exist only in
the issuer's Python and in a script
(`python/iroha_app_attestation/src/iroha_app_attestation/attestation.py:974-1061`,
`attested_enrollment.py:178-217`,
`scripts/android_attestation_certificate_profile.py:643-760`). The validators
need the same checks in the node, with one result on every validator.

Android chain.

- Anchor on Google's root public keys, not certificate bytes. There are two:
  the RSA-4096 root and the ECDSA P-384 root, which Google says began signing
  chains on 2026-02-01. The verifier accepts no operator-configured Android
  roots. Nothing is removed from the existing suite (§11).
- Classify factory versus remotely provisioned chains as Google's reference
  verifier does. Ignore expiry only for factory chains. For a remotely
  provisioned chain, check that its certificates were in date at the time of
  enrollment: they live two to four weeks in Google's test chains, and Google
  says the short life must be enforced. Never check leaf validity. Replace the
  exact version-pair set.
- Check every certificate serial against Google's revocation list. The issuer
  fetches the list. The validators check against a snapshot that is a ledger
  input named by the policy entry, so that every validator reaches the same
  result. The issuer re-checks stored serials daily (§6).
- Read the key description by walking the certificate's structure from the
  top, never by matching bytes: the challenge, the subject and the
  application id are bytes the caller chose. Take it from the certificate
  nearest the root that has one, with the one designed exception below.
- Require, in the hardware-enforced list: the two security levels equal, and
  TEE or StrongBox; origin generated; a root of trust with the bootloader
  locked and the boot state Verified; OS, vendor and boot patch levels at or
  above the floor of the policy entry. Require the application id in the
  software-enforced list and absent from the hardware-enforced list, with
  exactly one package and one signing digest, equal to the policy entry.
- Decide the boot-key rule from a captured production chain per tuple. Two of
  Google's published Pixel 9 Pro chains show a boot key of all zeroes on a
  locked, verified phone, so a list of allowed boot keys may not be usable.

The app attestation key.

- At enrollment the phone first generates an app attestation key: a P-256 key
  whose only purpose is to sign attestations, generated with the hash of the
  enrollment transcript as challenge, under the system's chain. The verifier
  requires, in that key's own description: a hardware-enforced purpose set of
  exactly "attest key", origin generated, the security level, and the
  root-of-trust and patch checks above. With those, every certificate the key
  signs was composed by the secure hardware. This is the exception to "nearest
  the root": the chain then holds two key descriptions, and the verifier reads
  both.
- The device key is generated with the app attestation key named as attester,
  at the same security level. Its leaf is one certificate signed by the app
  attestation key. The verifier requires purpose sign, P-256, origin
  generated, the digest authorization Q0 fixes (§2.3), and no use limit, no
  user-authentication requirement and no unlocked-device requirement (§5.9).
- At a renewal, where R8 is on, the phone generates a fresh key under the app
  attestation key with the hash of the renewal request as challenge. The
  verifier checks that leaf, applies the patch floor of the current policy
  entry, and refreshes E. That needs no system attestation key, so by source
  reading it works on a phone whose remotely provisioned certificates have
  expired. Not tested (test h2).
- Where a phone lacks the feature, the fallback at a renewal is a fresh key
  under the system's chain whose public key the device key signs. That shows
  the app that holds the device key. It does not show the same secure
  hardware. Whether a tuple without the feature is admitted is the owner's
  decision with its gate finding.
- Both key properties are fixed and attested when the device key is
  generated. No enrollment outside a test scheme happens before Q0 has fixed
  the digest authorization. Changing either later needs a new key and a
  Migrate for every enrolled Android phone.

Factory-provisioned roots. This is an owner decision (§12), and this document
does not take it. The two choices and their costs:

- Refuse them. E is then issued only under remotely provisioned chains. That
  excludes a leaked factory key, which is the one way found to enroll a key
  held in software on any machine without a stolen issuer-side key. It also
  excludes older phones: the Pixel 6 StrongBox chain on record, and by the
  vendors' own statements the Xiaomi, OPPO and vivo generations of 2020 and
  2021.
- Admit them, as a separate platform class. Those phones can enroll. A
  factory key that has leaked and is not yet on Google's list then enrolls
  software keys, and a pool that admits the class carries that for every
  holder in it.

iPhone.

- Pin the public key of Apple's App Attestation root. Require the production
  environment and reject the development one. Require the App ID of this
  scheme. On iOS 27 require the launch category "App Store" and the expected
  bundle version. Do not read the undocumented leaf extensions as a statement
  about the operating system.
- Enrollment and re-attestation require a payment-key signature over the
  transcript. The existing verifier requires the two keys to differ.
- A renewal carries an assertion by the enrolled App Attest key. It shows that
  something holding that key signed the request, and nothing about the
  operating system. It cannot carry a new attestation of the same key: Apple
  refuses to attest a key twice.
- The app is opted out of Mac availability and the verifier accepts only iOS
  attestations.

Admission and tiers.

- Admission keys on the platform class, the chain class and the patch level,
  not on StrongBox. For iPhone it keys on the App Attest facts above.
- Whether iPhone tiers carry lower limits than Android tiers while test h7 is
  open is the owner's choice, taken with the admission decision.
- A patch floor is applied at enrollment and at each renewal and nowhere
  between. It bites only where R8 is on. Google's update commitment for the
  Pixel 6 ends in October 2026; the owner decides the floor and whether it
  applies to a phone past its vendor's updates.
- Play Integrity and the vendors' own verdicts (Samsung Knox attestation,
  Huawei's integrity checks, Xiaomi's trusted-device token) each need the
  vendor's server to produce or to verify. None enters E as designed
  (§5.1). If the owner requires one, it enters E only as a field signed by
  the party that made the vendor call (§2.1). The issuer may otherwise use
  one at enrollment or renewal. Whether Play Integrity is required is an owner
  decision (§12); requiring it excludes phones without Google Play.

Client changes.

- The Android manifest of the released app carries the settings of §5.10:
  `allowBackup="false"`; data-extraction rules that exclude everything from
  cloud backup, device transfer and cross-platform transfer; a backup agent
  that saves nothing and restores nothing; `hasFragileUserData="true"`, so
  that the system asks at uninstall whether to keep the app's data; a
  `manageSpaceActivity`, so that the settings app opens the wallet's own
  screen where it would offer to clear storage; and
  `rollbackDataPolicy="retain"`. The journal is in the no-backup directory.
  The marker does not depend on these settings; group a of §10.4 tests with
  and without them. The repository's example manifests set
  `allowBackup="true"` and no rollback policy
  (`examples/android/retail-wallet/src/main/AndroidManifest.xml:4`,
  `examples/android/operator-console/src/main/AndroidManifest.xml:4`).
- The marker's storage form on Android is decided per Android release by test
  d1 of §10.4. Where the marker is a certificate entry, the app names the
  X.509 provider when it builds and reads the wrapper (§5.10).
- The key stores of the existing suite are not copied. The Android one uses
  the alias lookup, which answers "absent" on any key-store error
  (`kotlin/client-android/src/main/java/org/hyperledger/iroha/sdk/crypto/keystore/KagemushaAndroidHardwareAppKeyStoreV1.kt:61, 81, 164`).
  The iPhone one reads every keychain error as "no key"
  (`IrohaSwift/Sources/IrohaSwift/KagemushaAttested/KagemushaAttestedHardware.swift:67-78`).
  The wallet core takes "present", "absent" and "unknown" from the calls §5.10
  names.
- On iPhone the app requires a passcode at enrollment and says, before the
  first load, that removing or resetting the passcode puts the balance out of
  reach (§5.10).

### 10.4 The evidence gate

Nothing in this section has been run. It lists tests and a rule for reading
their results. It holds no results. Where it says how a platform behaves, the
statement comes from documentation or source code unless it says measured.

#### 10.4.1 What the gate is and why it comes first

The review the owner supplied with the acceptance criterion says: "The design
must satisfy it; documenting exceptions does not satisfy it." It also says:
"The next design gate should be evidence that the proposed hardware and
protocol meet it, before treating either B1 or B2 as an acceptable
implementation choice." B1 and B2 are that review's names for a payment that
carries a proof and for one that carries signatures only.

The evidence gate is that gate. It is:

- device tests in ten groups, a to j (10.4.5);
- a rule that turns the results into one of three findings per tuple:
  unsupported, supported with a stated assumption, or supported (10.4.6);
- a rule for when the results mean that the criterion cannot be met on stock
  phones, and the owner's options then (10.4.6).

It comes before the proof qualification (Q0 to Q3, §2.3) and before any
support claim, for three reasons.

- A proof does not check the marker. What stops a payer from bringing back an
  earlier state and paying from it is the paying wallet's own marker (§5.10).
  If that mechanism fails on a phone, it fails there with or without a proof,
  and no stage of Q0 to Q3 can repair it.
- P1 and PC rest on the same mechanism on the receiving phone: the credit is
  in the key store before the wallet reports it.
- The enrollment statement E is built from what each vendor's attestation
  carries. Which fields exist, and which the secure hardware enforces, is a
  fact about each tuple. Q0 fixes the relation of §2.1 from those facts.

What the gate does not do. It does not qualify the proof: no prover exists
(§2.2). It does not choose which assumption the owner accepts. It does not say
who supplies a shortfall when an assumption fails on some phone; §3.2 puts
that to the owner. It does not test the issuer, the ledger or the time and
limit rules; those tests are in 10.4.8.

Terms used in this section.

- An ordinary user has the released app on an uncompromised phone and uses
  whatever the platform, the vendor or a desktop tool offers: backup and
  restore, reinstall, clone tools, developer options, the settings app, the
  clock.
- A compromised phone is a phone on which T1, T2 a, T2 b or T3 does not hold
  (§4).
- A tuple is defined at the head of §10. Every result belongs to one tuple.
- The gate app is a test wallet. 10.4.4 says what it contains.
- A spare unit is a phone that may be erased, rooted, jailbroken or opened.
- Ready, waiting and stopped are the wallet states of §5.10. A resume is the
  event of §5.10: the wallet continues at the state its marker holds when its
  files are older, missing or damaged.
- Complete is what a wallet shows its user when a transfer is done (PC,
  §5.2). The receiving wallet shows it after its checks, its own proof, the
  durable commit of the ReceiveFold with its `Credited` Outcome, and the
  confirmed marker step. The paying wallet shows it after it has stored a
  `Credited` Outcome, and shows "sent, not confirmed" before that.
- Step numbers in this section are the gate app's, which has no prover:
  1 the wallet is ready; 2 sign in memory; 3 create the new marker, which
  carries the checkpoint; 4 commit to the journal, durably; 5 delete the
  previous marker and confirm it absent; 6 release. §5.2 has seven steps,
  because its step 3 is the proof. Steps 3 to 6 here are steps 4 to 7 of
  §5.2.

#### 10.4.2 What the tests take as given and what they turn into facts

The design claims P1 to P5 and PC under the assumptions T1 to T6 (§4). This
part repeats, in words, only what the tests need. Where §4 words an item
differently, §4 wins.

Taken as given by groups a to d, g, i and j. No device test can establish
these as general statements (10.4.9). Group h records what evidence a party
holds for each, and when.

- T1. The phone's secure hardware holds the device key, and the key cannot be
  exported.
- T2, as far as it concerns code. The phone runs the vendor's released
  operating system, started by verified boot. Nobody has gained the ability
  to run code as the wallet app or with more privilege than the wallet app.
  Then only the wallet app uses the app's key and its key-store entries.
- T3. The wallet app is the released build, unmodified.
- T4 and T5. The issuer-side keys, the ledger and the cryptography.

Turned into facts per tuple by groups a to d. These are the key-store and
storage behaviours that T2 names. After the gate each is shown for a tuple,
on the routes and the operating-system release that were tested, or the tuple
is unsupported.

- No path that the platform or the vendor offers puts an older key-store
  entry back. None removes a marker while the device key stays usable, the
  iPhone passcode, removed or reset, apart.
- A journal commit that has returned survives a power cut. A key-store
  creation or deletion that has returned and been read back survives a power
  cut. A key store that loses writes loses its latest ones, and never keeps a
  later write while losing an earlier one.
- The wallet can tell "absent" from "cannot be read now" by the calls §5.10
  names.
- The device key and the marker survive a reboot, an operating-system update
  and an app update, and on Android a change or removal of the screen lock.
- A failed operating-system update does not roll back the key store or the
  files after the app has run.
- The key store answers again after an unlock or a restart.
- The crash and restore behaviour of the marker order is the two tables of
  §5.10.

A test shows that each route tried, on each release tried, behaves so. It
does not show that no other route exists.

Tested against a compromised phone, on spare units, by groups e and f and by
the negative tests of group h. These tests ask one question: does the secure
hardware of any target phone give only one signature per state even when
code with operating-system privilege asks for two. The studies behind §4.1
found no such phone. Groups e and f are the tests that could overturn that
finding. Until one does, P2 holds only while T1 to T3 hold on the paying
phone.

Recorded for the enrollment statement by group h: what each tuple's
attestation carries, which of it the secure hardware enforces, and whether
the app attestation key works with no network.

#### 10.4.3 Claims that need device evidence, by property

Each claim names who enforces it, the input that party uses, and the tests
that bear on it.

P1. B owns the value durably and can spend it onward offline.

- A credit that the receiving wallet has reported complete is still in its
  marker and its journal after a power cut at any later moment. The receiving
  wallet enforces this by creating the marker and committing before it
  reports. Tests b6, c2, c5.
- When the receiving wallet reports complete it is ready, and it can pay the
  value onward at once with every radio off. The receiving wallet enforces
  this; the next receiver checks that wallet's certificate, receipt, E,
  signature and proof. Tests i1, i3. The proof itself cannot be tested; i4
  states the condition for Q2.
- When the wallet's files are older, missing or damaged while its key and its
  marker live, it resumes at its current state with no network. The wallet
  enforces this from the checkpoint in its marker. Group a, tests b3, b4.
- A locked or failing key store never makes the wallet conclude that its
  marker or its key is gone. The wallet enforces this from the exact result
  of each key-store call. Tests d2 to d5, d7, d8.
- A power cut at any step of the marker order leaves the wallet ready or
  waiting, never stopped. Tests b2, c2 to c4.
- Settings changes and updates that an ordinary user makes leave the wallet
  ready at the same head. Tests d1, d6, d7. One case is known from Apple's
  documentation and is not a test question: removing or resetting the iPhone
  passcode discards every item of the marker's keychain class (T6).

P2. A cannot spend the same value again.

- No path that the platform or a vendor offers gives the released app an
  earlier state together with a key that signs. The paying wallet enforces
  this from the checkpoint in its one marker. A receiver cannot check it.
  Group a.
- No crash or power cut, alone or followed by replacing or removing the
  files, does so either. Tests b1 to b5, b8, b11, c1.
- A marker write that has returned stays done. Tests c1 to c4.
- After a resume no inbound credit is folded a second time: not a Payment,
  not a `Refused` Outcome, not a voucher, not a countersigned Migrate. The
  wallet enforces this from lists and counters in the checkpoint. The
  receiver forms of group a, tests b4, b9, b10, b12, b13.
- A device key that the issuer certified is held in the phone's hardware, on
  a phone that started a verified operating system, under the released app,
  as far as E states it (§2.1). The issuer and the validators enforce this at
  enrollment from the vendor attestation. A receiver enforces it on the last
  hop from the certificate, the receipt, E and the device signature, and for
  earlier hops from the proof. Group h.
- Only if P2 is claimed against a compromised phone: the secure hardware
  gives one signature per state to code that has operating-system privilege.
  Groups e and f.

P3. B's payment does not depend on later reconciliation, approval or
settlement.

- A complete exchange runs with no network on either phone, and the app sends
  nothing afterwards until the user starts a sync. Test i2.
- Received value moves onward over many hops while no phone in the chain
  contacts the issuer. Test i3.

P4. Later discovery of misconduct by A cannot invalidate B's accepted value.

- After B's wallet learns of a block entry, evidence or a key revocation
  against A, B's balance and B's ability to pay are unchanged. B's wallet
  enforces this: it never removes a committed credit. Test j1.
- A third phone that already holds that knowledge accepts B's payment of the
  same value. Test j2.

P5. Only explicitly enabled regulatory controls may require connectivity.

- With every regulatory control off, the wallet pays and receives through
  reboots, clock changes, long idle periods, updates, a resume and every
  notice of §5.11, and never asks for the network. Test i5.
- With one control on, only that control stops the wallet, and the wallet
  names it. Test i6.

PC. Processing finishes before the wallet reports complete.

- The wallet reports complete only after the marker step and the durable
  commit, and the paying wallet only after it holds the `Credited` Outcome.
  Test b7.
- Nothing that P1 to P4 need runs after the report. Tests i1, i4.
- The time all of it takes, without a proof. Group g. With a proof it cannot
  be timed; §2.3 has the targets.

#### 10.4.4 and 10.4.5 The conditions and the tests

The conditions that hold for every test (10.4.4) and the tests themselves
(10.4.5), each with setup, steps, what to record, and pass and fail, are in
[`kagemusha_evidence_gate.md`](kagemusha_evidence_gate.md) under the same
numbers. That file also holds 10.4.7 (what exists in the repository and what
must be written), 10.4.8 (the tests the gate does not replace) and 10.4.10
(the sources for the platform statements). The groups are:

| Group | What it tests | Tests |
|---|---|---|
| a | No payment from a restored or rolled-back state | 20 |
| b | Crashes and power cuts at every step of the marker order | 14 |
| c | Key-store writes and commits across power loss | 7 |
| d | What the key store returns, and what survives a settings change | 8 |
| e | The one-use key on TEE and on StrongBox, per vendor | 7 |
| f | The iPhone assertion counter | 4 |
| g | The time of one complete exchange, without a proof | 5 |
| h | What attestation shows, and what an enrollment statement can say | 10 |
| i | Onward spending and running with no network | 6 |
| j | Later misconduct by the payer | 2 |

#### 10.4.6 Reading the results

**Per tuple.** One finding per tuple. It is recorded with the tuple's
platform class (§10.1) and with the fields of E that h1 found.

- **Unsupported.** Any one of these:
  - a reset in group a, in any build, or a path of group a that exists on the
    tuple and was not run;
  - destroyed, in the release build, on a path that T6 does not name;
  - a fail in b1 to b7 or in b9 to b13;
  - on iPhone, a fail in c1 at the wait the design uses;
  - a lost creation in c2 or a returned deletion in c3 at the delay the
    design uses, or a fail in c4, c5, c6 or c7;
  - no marker form that passes d1 on the tuple's Android release;
  - a fail in d2 to d5, d7 or d8, or in d6 apart from the iPhone passcode
    case;
  - a chain that the verifier cannot tie to a pinned root, or an accepted
    negative, in h1 or h3; a rejected release build or an accepted negative
    in h6 or h7 (i);
  - a fail in i1, i2, i3, i5, i6, j1 or j2;
  - a miss in g3, where the owner made the time target a gate.

  A tuple also stays unsupported until a wallet design for its platform
  exists (HarmonyOS NEXT today). Test b8 is not a tuple result: a fail there
  is a finding against the design on every tuple.

  Two of these causes are a holder's own action ending the wallet: destroyed
  on a path T6 does not name, and no marker form surviving a screen-lock
  removal. For each the owner may rule in writing that the action is a
  condition on the holder for that tuple. That cause then no longer makes
  the tuple unsupported. The tuple takes the finding its other results give,
  and the record names the action as a condition added to T6 for that tuple,
  which §4 then has to state. This document does not
  make that ruling. Each such ruling is a place where P1 rests on the holder
  not doing something the platform offers.
- **Supported with a stated assumption.** None of the above occurs. Groups e
  and f do not show that the secure hardware enforces one signature per
  state. The assumption is then T1 to T3 on that phone: its secure hardware
  is sound, its operating system is the vendor's released system and has not
  been taken over, and the app is the released app. The finding is recorded
  with the evidence that exists for the assumption on that tuple. On Android
  that is what E states, as of enrollment and of each renewal where R8 is on
  (h1, h2), together with h4's record that a takeover after boot leaves that
  evidence unchanged. On iPhone it is the App Attest facts and nothing about
  the operating system, and h7 (ii) may show that an old jailbroken device
  enrolls.
- **Supported.** As above, and in addition P2 is shown against a compromised
  phone. On Android that needs all of: tag 405 in the hardware-enforced list
  (e2 or e3); a pass in e4, e5 and e6; e7 workable inside the time target;
  and a relation, which this proposal does not contain, in which every hop
  proves that its one-use key was used (§2.1). On iPhone it needs f1 to f3 to
  pass, f4 (ii) to show that the counter cannot be forged on current
  hardware, h7 (ii) to be rejected, and a design that co-signs every payment.
  Even then one assumption remains: that the secure hardware itself is not
  broken.

What the sources lead one to expect. No tuple is expected to reach
"supported". The one measurement (Pixel 6 StrongBox) showed software
enforcement, the reference code allows two begun operations to finish, and
the one third-party source on the iPhone counter shows the operating system
building the assertion. That is an expectation from reading. It is not a
result, and groups e and f exist to test it.

**Per scheme.** Value moves between phones, and a proof does not show a
receiver which phones a value passed through. So a scheme's finding is the
weakest finding among the tuples it admits, and its claim about the operating
system is that of the weakest platform class it admits (§2.1). A scheme that
admits one tuple under an assumption is a scheme under that assumption. A
scheme that admits iPhones can claim, for any balance, no more than this:
each paying phone either stated a locked, verified boot as of its enrollment
or last renewal, or was a genuine Apple Secure Enclave acting for this App
ID. A scheme that admits a class with no operating-system statement claims
nothing about the operating system for any balance. A scheme also admits
every device its issuer cannot keep out. If h7
(ii) is accepted, a scheme that admits iPhones admits that old jailbroken
device as well, whatever its list says. Which classes a pool admits is the
owner's decision (§12).

**How a list of tuples can be enforced.** The finding is per tuple, so someone
has to keep phones outside the list out.

- Android. The issuer and the validators can refuse an enrollment from the
  attested operating-system version, patch levels, boot state and chain
  class, and from the attested brand and model if h1 shows that the vendor
  attests them.
- iPhone. They cannot. The documented attestation names no model and no
  operating-system version. The app's minimum operating-system version is
  enforced when the app is installed, by a system that a compromised phone
  controls.
- After enrollment, on either platform. A phone updates to an
  operating-system version that was never tested. Where R8 is on, the issuer
  learns of it at the next renewal: on Android from the fresh leaf, on iPhone
  only from what the app reports. A receiver offline learns what E says as of
  the payer's last renewal and nothing newer. Where R8 is off nothing reaches
  an enrolled wallet: it goes on paying on whatever release its phone moves
  to, tested or not. That is a consequence of P5 and is stated in §3.2.

**When the criterion cannot be met on stock phones.** Two cases.

1. The owner rules that P2 must hold against a compromised phone. The owner
   said: "because we cannot allow compromised OS, we should include as part
   of our proof constraint matrix that the OS is real and prove it somehow".
   E and the proof carry what can be proven: how each paying phone booted and
   which app its operating system named, as of enrollment or the last
   renewal. They do not show that the running operating system is
   uncompromised when a payment is signed (§2.1). If that is required, only
   the finding "supported" gives it. If no tuple reaches it, the criterion
   cannot be met on any stock phone on the list.
2. The owner accepts T1 to T3 as the stated assumptions, and every tuple of a
   platform is unsupported: for example a vendor tool returns key-store
   entries, or keychain writes are lost at a forced restart and no wait cures
   it. Then the criterion cannot be met on that platform with this marker. If
   that holds for every platform, it cannot be met on stock phones with this
   mechanism. For the keychain case §5.10 names two further courses and puts
   them to the owner: accepting the exposure, which leaves P2 unmet against
   an ordinary user on iPhone, and an iPhone wallet that stops when its
   files are absent, which leaves P1 unmet after an app deletion. Neither
   changes the finding.

A third result needs the owner and is not a phone result: the iPhone
passcode. Apple documents that removing or resetting the passcode discards
every item of the only keychain class that Apple documents as never backed
up. The marker is then gone while the key may live, and the wallet is stopped
for good (§5.10). No documented iPhone store was found that is outside every
backup and also survives the removal. The second anchor that §5.10 considers
rests on behaviour that f1 to f3 would have to show on every iOS release and
that Apple has said it intends to change. So P1 on iPhone holds only for a
holder who keeps a passcode set and does not reset it. T6 states that
condition. Whether P1 may rest on it is the owner's ruling (§12); this
section does not present it as met.

**The owner's options when the criterion cannot be met.** This document names
them and does not choose.

- Hardware that runs wallet logic: a secure-element applet, a smart card, or
  a trusted application from the phone maker. R2 excludes a custom applet,
  and the owner said: "right now smart cards are future optionality to
  explore but there are no plans right now to use."
- Narrowing the list to the tuples and platform classes the gate supports.
  The paragraph above on enforcing a list says how far that reaches. The list
  may be empty for the finding "supported".
- Accepting T1 to T3 as the stated assumptions. The finding "supported with
  a stated assumption" then meets P2 as the criterion words it, because P2
  holds "under the stated security assumptions". P4 has no such condition.
  What P4 implies for value made by a phone on which an assumption failed is
  the subject of §3.2 and is not answered here.
- No production release of offline value.

**The owner's options if the proof qualification fails.** Q0 to Q3 (§2.3)
decide whether a proof-carrying payment meets PC in a time the owner accepts.
Nothing is measured today; the repository's own gate is 10 s for one proof,
and both the payer's and the receiver's proving fall inside the payment. If
the qualification ends without such a proof, the owner chooses among these.
This document chooses none.

- Relax one named constraint and run the qualification once more under a new
  budget and date. The candidates are the timing wish for a proof-carrying
  payment, the 10 KB bound of R9, the absence of a trusted setup, and the
  list of phones.
- A narrower device list: only the tuples on which the proof met the
  targets.
- Adopt one of the candidate shapes that §2.3 lists, after it has been
  designed and checked. None is designed today.
- Payments in the signature-only form. A receiver then checks the last hop
  only: the certificate, the receipt, E as the validators sealed it, and the
  device signature. It learns nothing about earlier hops. Nothing then shows
  that a value came from a load through enrolled phones, an arithmetic defect
  in a wallet is not contained, and the constraint about the operating system
  that the owner asked for is carried for the last payer only.
- Hardware that runs wallet logic, as above.
- No production release of offline value.

**Results go stale.** A finding holds for its tuple and for the gate app or
wallet build that was tested. Run the affected groups again when: the
operating system's major version changes; a vendor ships a new version of a
backup or clone tool; the KeyMint version in the attestation changes; a
system module that holds the certificate parser changes; the wallet's marker
or commit code changes. When the wallet core exists as released code, groups
b, c and d and one path of group a per platform run again on it; the gate app
is not the wallet. A phone past its vendor's update commitment stays on its
tested tuple while public exploits for that tuple accumulate. Google's
commitment for the Pixel 6 ends in October 2026 (§10.3). Whether a finding
for such a tuple expires is the owner's to set; the gate cannot measure it.

#### 10.4.9 What the gate cannot show

- It tests chosen phones, chosen operating-system versions and chosen tool
  versions. It says nothing about a model, a version or a tool that was not
  on the bench. Phones already enrolled update themselves offline to versions
  nobody tested.
- It cannot show that an operating system has no exploit, that a
  secure-hardware firmware has none, or that no attestation key has leaked.
  No test can. Groups a to d test what the released app on an unmodified
  system does. Groups e, f and h test named attacks on spare units. A named
  attack that fails is one attack that failed.
- It cannot show that the operating system of a paying phone is uncompromised
  when a payment is signed. No field of any attestation changes when a
  locked, verified, patched phone is taken over after boot. Test h4 records
  that; it does not remove it.
- It cannot show that a rate is zero. A hundred clean power cuts bound a loss
  rate below about 3%.
- A forced restart is not a power cut. Only the opened unit tests the storage
  chip's own cache, and one opened unit is one storage part.
- It cannot list every restore path. Third-party desktop tools, repair-shop
  tools, device-management commands and tools not yet released are outside
  it. Each path of group a that passes is one path closed.
- Closed secure-hardware code cannot be read. For Samsung, Qualcomm, MediaTek
  and Huawei the gate sees only what one firmware did on the day. Where tests
  e4 to e6 cannot be run at all, nothing is shown.
- A forgery that works on an old jailbroken Apple device shows how that
  system builds an assertion. A forgery that fails there shows nothing about
  a current iPhone. The test on current hardware depends on access the
  project may not get.
- On iPhone no test produces evidence of the operating-system state at
  enrollment or later, because the attestation does not carry it.
- It cannot say how many users hold a compromised phone. The line is drawn by
  what the user does, and a user who follows a published rooting guide is on
  the other side of it.
- The gate app is not the wallet. The findings carry over only after the
  affected groups are run again on the wallet core.
- It cannot test the proof. No prover exists. Whether a proof-carrying
  payment finishes in a time the owner accepts is decided by Q0 to Q3.

## 11. Order of work

The owner said: "there should be only one design. we need to remove other
stuff and standardize on one design." This proposal names that design and
gives every other track an end state (§11.1). Accepting it deletes nothing.
Each removal is a separate owner decision, taken after its own dependency
check.

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

The design of this proposal is a fourth track from the day its first code
lands, because all its wire objects are new (§5). Until a removal is
approved, the tree holds one track more than it holds today. The count goes
down only by these decisions, each the owner's:

- Replacing the attested-app suite in place (§11.1 row 1). This returns the
  count to three. It needs no measurement, because nothing in the repository
  consumes the suite. The owner first says whether any app outside the
  repository uses it. If it is approved, the gate app and the wallet core are
  written in the suite's two stub crates.
- Withdrawing the ordinary profile's online-control path (row 4). This needs
  the owner's statement that this proposal replaces the record of 2026-10-01,
  a migration item for the adapters that record names, and the end-to-end
  trace. It is not taken before Q1 (§11.1 row 4), because the files this path
  shares with the ordinary circuits (row 3) have not been separated, and the
  per-hop relation is built on those circuits.
- Ending Recursive V1 on qualified hardware as a separate design. That
  happens when Q0 has specified the relation of §2.1 against this design's
  objects and the Guard for qualified hardware is retired (rows 2, 3 and 6).

The end state is one track: this design, with a proof on every payment. The
recursion code is its prover and its verifier, and it is kept. If the
qualification ends without a proof that meets PC in a time the owner accepts,
the owner chooses among the options of 10.4.6, and the end state of rows 2,
3, 10 and 14 is decided with that choice. This document does not anticipate
the choice, and it does not recommend removing the recursion code in any
case.

**While more than one track exists.**

- A wallet build admits one payment profile. The profile code in the envelope
  (§5.6) selects it, and the decoder rejects every other code before it reads
  the payload. The rule exists so that value accepted under one track's
  checks is never credited under another's.
- This design does not use profile code 2 or the text prefix `kga1:` unless
  the attested-app suite's objects are withdrawn in the same change. The
  suite claims both
  (`IrohaSwift/Sources/IrohaSwift/KagemushaAttested/KagemushaPeerMessage.swift:23-28`),
  and the shared envelope registers only codes 0 and 1
  (`IrohaSwift/Sources/IrohaSwift/IrohaPeerWireV1.swift:6-11`). The rule
  exists so that no decoder can read a suite object as an object of this
  design.
- No existing track is recorded as carrying production value. The recursive
  protocol is recorded as not production qualified
  (`specs/kagemusha_v1_production_readiness.md:3-6`), the node keeps a
  reject-all verifier while no release keys exist (§2.2), and the suite
  cannot make a payment. On that record no removal in §11.1 strands a
  production balance. This was read from the repository; no live network was
  queried. Value on a testnet is test value.
- The repository forbids parallel implementations in the first release
  (`AGENTS.md:26-27`), and the recursive specification allows one decoder and
  no protocol selector (`specs/kagemusha_v1.md:3-6`). Coexistence breaks both
  until the removals happen. Accepting this proposal accepts that exception
  for that period, and the owner should say so (§12).

**Order.** The stages follow one another: the evidence gate, then Q0, Q1, Q2
and Q3 of the proof qualification (§2.3). Building runs beside them as far as
the three lists below allow.

1. **The owner fixes the gate's inputs** (10.4.4): the list of tuples, the
   time target for the exchange and whether it is a gate, the number of
   trials, and whether P2 must hold against a compromised phone.
2. **The evidence gate** (§10.4). The gate app is written; its marker,
   checkpoint, commit and recovery code is the wallet core's (§9), built
   first. The existing probes are reused. The Android one-use probe is
   changed first, because today it returns on the feature flag before it
   generates a key
   (`kotlin/client-android/src/main/java/org/hyperledger/iroha/sdk/offline/probe/AndroidKeyMintSingleUseProbeV1.kt:79-81`).
   Tests c1, d1, and h1 with h2 run before the others. The gate ends when
   every tuple on the list has a recorded finding and the owner has ruled on
   it in writing: which tuples are supported and under which stated
   assumption, which platform classes a pool admits, or that the criterion
   cannot be met on stock phones. Nothing after this step is decided before
   that ruling: not the acceptance of the normative specification, not a
   support claim for any phone, not the start of Q0. A result here can change
   §4.1, §5.2, §5.10, §7.2, §10.1 to §10.3 and the fields of E.
3. **Q0, targets and relation** (§2.3). The owner fixes the targets. The
   relation of §2.1 is specified against this design's objects, for the
   tuples and platform classes the ruling supports, with the fields of E as
   the gate found them. This freezes the circuit-facing surface. Two
   properties of the device key are fixed when the key is generated: the
   digests it may sign, and that an app attestation key attests it (§10.3).
   On Android both are attested at key generation. Both are decided no later
   than this step, and before the first enrollment outside a test scheme.
   Changing either afterwards needs a new key and a Migrate for every
   enrolled phone it affects.
4. **Q1, host proofs** (§2.3). One complete lineage with real proofs on a
   host. The server-made enrollment proof of the vendor chain is a Q1 item.
   The repository has no P-384, RSA or SHA-384 gadget, so whether that proof
   can be built is not known. It is made on a server and never on a phone.
   Until it exists the recursion starts from the statement the validators
   sealed (§2.1).
5. **Q2, phone proofs** (§2.3), on the supported tuples, under the conditions
   that test i4 states.
6. **Q3, soundness and ruling** (§2.3). The stage ends in the owner's written
   ruling on whether the proof-carrying design meets PC in a time the owner
   accepts. If it does not, the owner chooses among the options of 10.4.6.
7. **Before this design carries production value.** These are necessary, not
   sufficient:
   - the gate has a finding of supported, or of supported with an assumption
     the owner has accepted in writing, for each tuple for which support is
     claimed; the affected groups have been run again on the released wallet
     core and on each new operating-system major version; and the tests of
     10.4.8 have passed;
   - step 6 has ended in a ruling that the design meets PC, or the owner has
     chosen in writing among the options of 10.4.6;
   - the owner has taken the decisions that §3.2 puts, among them who
     supplies a shortfall when an assumption fails, and has decided which
     platform classes a pool admits;
   - the owner has replaced the normative statements listed in §11.1;
   - the two key properties of step 3 are decided;
   - row 1 of §11.1 is resolved, so that the tree holds one design for
     stock-phone keys and not two.
8. **Removals.** §11.1 gives, per row, the earliest point and whose decision
   it is. The check before each removal is a search of the repository for
   callers, a build of the workspace and of the Swift and Kotlin packages,
   and the owner's statement about consumers outside the repository. The
   evidence appendix cites several of these files by line at commit
   `b2a3cd05bc`; its citations hold against that commit and not after a
   removal.

**What can be built before the gate's ruling.** One rule keeps early work
from fixing a format by accident. Every signed preimage starts with
`tag ‖ scheme id` (§5). Work done before Q0 completes uses a test scheme id.
Its encodings, vectors and enrolled keys are discarded when the relation is
fixed.

Built from now, and not redone unless the gate finds against the design
itself, which is what the gate is for:

- the wallet core's commit rule, marker, checkpoint, recovery and resume
  (§5.2, §5.10, §9), behind the four traits the platform implements. The
  gate app is this code. The storage form of the marker sits behind the
  `Markers` trait, so a form that test d1 or c1 changes is a change to one
  platform implementation;
- the core's test mode, in which every cell of the two tables of §5.10 is an
  automated test, and a finite-state model of the commit, marker, checkpoint
  and restore rules in `formal/`. Those rules can fail on a crash or a
  restore between two steps, and a model enumerates such interleavings within
  its bounds. Group b tests the same tables on devices;
- the carriers of §5.6, which move opaque bytes and are sized for a 7.6 KB
  Payment;
- the attestation verifiers of §10.3, in the issuer and in the node, with
  the chain through an app attestation key. Group h uses them;
- the time rules, limits, expiry and block-list checks as logic over typed
  fields (§5.4, §5.5);
- the block index, the registry and its registration flow, the ledger
  instructions of §6 and the accounting of §8, against a test scheme;
- the issuer service: enrollment, sync, replay, acceptance of a resume
  record, numbering of vouchers, key custody (§6, §7);
- the test artifact and test scheme of §9;
- the normative specification, as a draft.

Not fixed before the gate's ruling, because a finding decides it:

- the storage form of the Android marker on each release (d1), and the
  release rule on iPhone with any wait before release (c1);
- the manifest settings, which group a tests with and without;
- the list of supported tuples, the platform classes a pool admits, and the
  policy on factory-provisioned roots;
- the fields of E per platform class and the policy entries (h1, h2, h6): for
  example whether a list of boot keys is usable, and whether a renewal on a
  tuple can carry a fresh leaf;
- the time target for the exchange and the carriers a wallet must support
  (group g);
- every statement in §4 about what a tuple's key store does;
- acceptance of the normative specification, any conformance vector
  published as stable, and any support claim.

Not started before the gate's ruling: prover work for phones, and the
server-side enrollment circuit. Which roots and signature algorithms that
circuit must verify follows from the ruling on platform classes and factory
roots.

Not fixed before Q0, because the relation decides it. This is the
circuit-facing surface; §2.3 has the relation choices.

- the transition preimage and its digest function, and the state commitment,
  which carries the digest of E;
- the digest authorization of the device key;
- the signature schemes of the certificate, the receipt and the voucher, and
  the form in which the validators' seal reaches a proof;
- the shape of the load and unload instructions, and the replay guard for
  mints, of which the voucher number is the wallet's side;
- the payment envelope and the Outcome;
- how a SendSplit binds its `fee_policy_id` (§5.8);
- every conformance vector.

Code behind that surface is written behind one module boundary and is
provisional. Provisional formats are used in a test scheme only. No
enrollment or balance in a provisional format outlives the test scheme that
made it. The rule exists so that a later change of format never strands a
balance or re-enrolls a real user. The cost is accepted: if the relation
changes that surface, the code and vectors behind it are redone.

No effort estimate exists for the gate, for the design outside the proof, or
for any removal. §2.3 has what estimates exist for Q0 to Q3.

### 11.1 End state of every existing track

The facts below were read from the working tree at commit `b2a3cd05bc` on
2026-10-02. Nothing was built or run. File and line counts are of tracked
files; the repository holds 916 tracked files with "kagemusha" in the path,
about 482,000 lines. KAGEMUSHA code in files without that name, for example
`crates/iroha/src/client/ordinary_native.rs`, is not counted, so the counts
are lower bounds. "Replaced" means that a named part of this proposal takes
over the function. The code stays until its removal is approved. Every
removal decision is the owner's. The end state is given for the target: a
proof-carrying payment under the relation of §2.1. Rows 2, 3, 10 and 14 are
decided again if the owner chooses otherwise at the end of Q3 (§11).

| # | Component and where | Consumed today by | End state under the proof-carrying target | Replaced by | Earliest removal |
|---|---|---|---|---|---|
| 1 | Attested-app suite, signature only. 26 files, 10,435 lines. Swift `IrohaSwift/Sources/IrohaSwift/KagemushaAttested/` (9 files, 3,064 lines). Kotlin `kotlin/core-jvm/src/main/java/org/hyperledger/iroha/sdk/offline/attested/` (7 files, 3,149 lines). JavaScript `javascript/iroha_js/src/kagemushaAttestedV1.js` (2,424 lines). Python `python/iroha_app_attestation/src/iroha_app_attestation/attested_enrollment.py` and `attested_selection.py` (1,561 lines). Rust crates `crates/iroha_kagemusha_attested` and `crates/iroha_kagemusha_issuer` (7 files, 237 lines; a layout test, an empty vector generator, two empty tests, an issuer that exits with "service implementation pending") | Nothing in the repository. No test names it. The JavaScript package exports no entry for it. No file under `kotlin/client-android` or `kotlin/kagemusha-wallet-android` refers to it. The shared envelope does not register its profile code. Its Swift and Kotlin types are public, so a consumer outside the repository cannot be excluded from the repository alone | Replaced in place. Its wire objects are withdrawn. As a design it carries no proof, so a receiver learns nothing about earlier hops; what a receiver checks on the last hop of this design (certificate, receipt, E, device signature) takes over its role. Kept and changed: the enrollment verifier (§10.3) and the Secure Enclave and App Attest key code (`KagemushaAttestedHardware.swift`). The two crates become the wallet core and the issuer service | The objects of §5.1, the wallet core (§9), the issuer service (§6) | When the owner approves it. Needs no measurement |
| 2 | Recursive proof stack. `crates/iroha_core_zk/src/kagemusha_v1_recursion/` (193 files, 151,759 lines, including row 3); `kagemusha_polynomial_store_v1` (7 files, 4,141 lines); `kagemusha_p256_curve_gadget.rs`, `kagemusha_v1_poseidon.rs`, `kagemusha_v1_crypto.rs`; the data model in `crates/iroha_data_model/src/kagemusha/` (59 files, 47,031 lines, including row 4's types) | Node block execution (`crates/iroha_core/src/smartcontracts/isi/kagemusha.rs:2617-2633`), which keeps a reject-all verifier while no release keys exist. The mobile bridge, which linked the prover in every build at commit `b2a3cd05bc` (`crates/connect_norito_bridge/Cargo.toml:42, 49`); an uncommitted edit found in the tree later on 2026-10-02 removes both features from those two lines, so this cell and the last item of §9 are to be read again before either is relied on. The Rust client (`crates/iroha/Cargo.toml:30`). Row 14. No build produces a State proof (§2.2) | Kept. The relation of §2.1 is built from it and specified against this design's objects in Q0. Keys are generated for that relation. It has no P-384, RSA or SHA-384 gadget; the server-made enrollment proof needs them (Q1) | Nothing | Never under the target. Otherwise only as a separately approved consensus, genesis and finality-proof format change, which this document does not recommend |
| 3 | Ordinary circuits: the P-256 Guard and its State consumer. 84 files, 30,859 lines with "ordinary" in the name inside row 2's directory, for example `ordinary_guard_composition.rs` and `production_ordinary_guard.rs` | The production prover is built from them (§2.2). The production construction refuses the ordinary State (`crates/iroha_core_zk/src/kagemusha_v1_recursion/composite.rs:1961-1968`) | Kept. The per-hop relation is built on their P-256 equations: one device signature per hop under the key in the proven predecessor state. The Guard is specified again against this design's objects; the existing one is bound to the online profile's approval objects and runs four P-256 equations per transition. The production refusal is lifted for the new Guard, and an ordinary ReceiveFold consumer is added | The Guard specified against this design's objects | Not a candidate. With row 2 |
| 4 | Ordinary online-control path: an issuer approval for each operation. Spec `specs/kagemusha_app_owned_hardware_v1.md`. Code: the ordinary files of `crates/iroha_core_zk/src/kagemusha_v1_state/` (50 files, 33,042 lines); `kagemusha_ordinary_*` in the data model (24 files, 18,196 lines); `crates/iroha/src/client/ordinary_native.rs`; the route `/v1/kagemusha/ordinary/current-wallet` (`crates/iroha_torii_shared/src/ordinary_wallet_current.rs:11`); Kotlin files named `*Ordinary*` (18 files, 2,048 lines); Swift ordinary and approval files (17 files, 2,415 lines); Python `ordinary_*.py` (8 files, 1,831 lines). Which of these files serve row 3 has not been traced | The owner record of 2026-10-01 names thin BPNG, BOI and CBSI adapters (`specs/kagemusha_v1_production_readiness.md:8-25`). No adapter code under those names is in the repository | Not used: it needs the network to authorize each operation, which R1 excludes. Not traced end to end. Its enrollment half (hardware key, key attestation, Play Integrity, App Attest) continues in row 12 | Enrollment (§7.1) and the offline exchange (§5.2) | After the owner says this proposal replaces the 2026-10-01 record (§12), the adapters have a migration item, the trace is done, and the Guard of row 3 no longer uses its approval objects. Not before Q1 |
| 5 | V1 native wallet: coordinator, durable state, bootstrap. `crates/connect_norito_bridge/src/kagemusha_core_coordinator_v1.rs` and its directory (56 files, 34,840 lines); the other files of `crates/iroha_core_zk/src/kagemusha_v1_state/` (49 files, 43,702 lines); the bridge's bootstrap, reserve-finality and hardware-evidence files | The Swift, Kotlin and Java coordinator bridges and wallet classes of row 13. The generic bridge installs no coordinator and returns device-unavailable (`specs/kagemusha_device_bridge_v1.md:635-644`) | Replaced by the wallet core as the owner of wallet state. Which prover-facing parts are reused is decided in Q0 | The wallet core (§9) with the commit rule of §5.2 and the marker of §5.10 | When the Swift and Kotlin shells over the wallet core pass conformance and the prover runs behind the new core |
| 6 | Secure-element device path. Specs `specs/kagemusha_device_bridge_v1.md`, `specs/kagemusha_device_sender_v1.md`, `specs/kagemusha_receiver_admission_v1.md`, `specs/kagemusha_guard_bundle_v1.md`, `specs/kagemusha_pixel6_ese_service_contract_v1.md`; the Apple route in `specs/kagemusha_v1_phone_algorithm.md:524-538`. Code: `crates/connect_norito_bridge/src/kagemusha_device_bridge_v1.rs` and its directory (4 files, 3,518 lines); Kotlin `KagemushaOmapiDeviceLifecycleV1.kt`, `KagemushaSecureElementApduV1.kt`, `KagemushaDeviceLifecycleBridgeV1.kt`, `KagemushaDeviceOperationCodecV1.kt`; Swift `KagemushaSecureElement*.swift`, `KagemushaDeviceLifecycleBridgeV1.swift`, `KagemushaDeviceOperationCodecV1.swift` (5 files, 4,072 lines); Java mirrors | Nothing that runs. No applet exists in the repository, both routes need access the project does not have, and stock dispatch returns unavailable (`specs/kagemusha_device_sender_v1.md:5-7`) | Not used. R2 excludes a custom applet, and the design does not assume that wallet logic runs in secure hardware (§14). It is the one existing text that states a one-successor contract, which the finding "supported" of §10.4 would need | Nothing. This design has no hardware route | Candidate. Not before the gate's ruling on groups e and f and the host proofs of Q1. The decision says whether the one-successor rule and the command framing are kept for cards |
| 7 | Device probes. Android key probes in `kotlin/client-android/src/main/java/org/hyperledger/iroha/sdk/offline/probe/` (`AndroidKeyMintSingleUseProbeV1.kt`, `AndroidKeyMintOneUseSelectionCandidateV1.kt`, `KeyMintRestartDiagnosticV1.kt`, `AndroidPixel6TestnetStrongBoxObservationV1.kt`) and their device tests; iOS `examples/ios/KagemushaAppAttestProbe/` (250 lines of Swift) | The Pixel 6 measurements (`specs/kagemusha_v1_production_readiness.md:342-379`) | Kept and extended for the evidence gate (10.4.7) | Not replaced | Not before the gate has run and its results are recorded |
| 8 | Testnet probes. `crates/connect_norito_bridge/src/kagemusha_testnet_*.rs` and `platform_jni/kagemusha_testnet_*.rs` (11 files, 5,272 lines); Swift `KagemushaTestnet*.swift` (5 files, 774 lines); Kotlin `probe/KagemushaTestnet*.kt` and `probe/Pixel6Testnet*.kt`; `scripts/prepare_kagemusha_testnet_observation_bundle.py` | Testnet diagnostics only | Not used | The test artifact and test scheme of §9 | Candidate. Same earliest point as row 6 |
| 9 | Peer transports. The envelope `IrohaPeerWireV1` (Swift, Kotlin, C#); animated QR `IrohaPeerQRV1`; NFC `IrohaPeerNfcV1` with the CoreNFC and Android carriers; Nearby `IrohaPeerNearbyV1`. Together: Swift 7 files, 9,416 lines; Kotlin 6 files, 5,051 lines. A second QR framing in Rust (`crates/iroha_data_model/src/qr_stream.rs`, `specs/qr_stream.md`). `specs/peer_transport_v1.md` | The V1 wallet classes. The envelope registers profile 1 only (`IrohaSwift/Sources/IrohaSwift/IrohaPeerWireV1.swift:6-11`) | Kept. This design uses one envelope and one QR framing under a new profile code, with limits sized for the proof-carrying Payment (§5.6). The normative specification names the framing; the other framing is then a removal candidate. Profile 1 goes with row 13 | Not replaced | The carriers are not removed. The duplicate framing: when the normative specification names one |
| 10 | Ledger. Instructions `TopUpKagemushaV1` and `RedeemKagemushaV1` (`crates/iroha_data_model/src/isi/kagemusha_v1.rs:2853-2870`); execution and reserve in `crates/iroha_core` (12 files, 9,824 lines; `smartcontracts/isi/kagemusha.rs`, `smartcontracts/isi/kagemusha/kagemusha_v1_reserve.rs`); mint-finality seals; the governed verifier registry (`specs/kagemusha_v1_production_readiness.md:124-130`) | Every node: block execution, genesis and the finality-proof format | The top-up path stays and gains the missing mint-credit producer (§8.5). Redemption, receipts and the pool invariant change shape (§8.5). The instructions of §6 are added under a new witness tag. New with them: the validators verify the raw vendor attestation at registration and seal the registry (§8.1) | The instructions of §6 | Only the parts §8.5 lists as not reusable, when their replacements exist. A consensus change |
| 11 | Torii routes and SDK Torii clients. `/v1/kagemusha/readiness`, `/top-up`, `/redeem`, `/operations/{operation_id}` (`crates/iroha_torii/src/lib.rs:4956-4959`); `/authority-state/{asset_definition_id}` (`crates/iroha_torii/src/kagemusha_state.rs:14`); 11 files, 4,542 lines in `iroha_torii` and `iroha_torii_shared`; clients in Swift, Kotlin, Java, JavaScript, C# and Python | The SDK wallet classes and operators | The command surface is reused (§8.5). Schemas follow row 10. The ordinary route goes with row 4 | Routes for the instructions of §6 | With row 10 |
| 12 | Attestation verifiers and enrollment clients. Python `python/iroha_app_attestation/` (25 files, 7,605 lines; raw verifiers `attestation.py`, `apple_receipt.py`, `play_integrity.py`, `revocation.py`, `provider.py`). The Android hardware key store (`kotlin/client-android/src/main/java/org/hyperledger/iroha/sdk/crypto/keystore/KagemushaAndroidHardwareAppKeyStoreV1.kt`). Swift App Attest evidence and enrollment (`KagemushaAppAttestEvidenceV1.swift`, `KagemushaAppAttestEnrollmentVerifierV1.swift`) | Ordinary enrollment (row 4) and the attested-app suite (row 1) | Kept and changed as §10.3 lists: the app attestation key on Android, the output E, and a second copy of the verification in the node for the validators. The `ordinary_*` workers go with row 4 | Not replaced | Not a candidate |
| 13 | V1 wire types and SDK wallet classes. Swift (59 files, 21,172 lines outside row 1, including the Swift files of rows 4, 6 and 8); Kotlin `core-jvm` (33 files, 12,991 lines), `client-android` (36 files, 6,266 lines), `kagemusha-wallet-android` (9 files, 786 lines); Java duplicates in `java/iroha_android` (24 files, 3,484 lines with tests); JavaScript (13 files, 4,745 lines with tests); C# (16 files, 6,598 lines with tests); Python (5 files, 5,055 lines with tests) | Apps built on the SDKs. The cross-SDK fixture tests (`scripts/tests/kagemusha_hard_cut_test.py`) | Replaced by thin Swift and Kotlin shells over the wallet core (§9). There is one payment format, so the V1 Request, Payment and Acknowledgement types are withdrawn. What the JavaScript, C#, Python and Java surfaces keep is not designed here | The shells and the objects of §5.1 | When the shells exist and no consumer of profile 1 remains. The Java duplicates follow `specs/jvm_consolidation_inventory.md` |
| 14 | Release, key-artifact and qualification tooling. Specs `specs/kagemusha_v1_compact_keys.md`, `specs/kagemusha_v1_native_profile_binding.md`, `specs/kagemusha_v1_provider_policy_binding.md`, `specs/kagemusha_v1_physical_evidence.md`, `specs/kagemusha_v1_release_runner_validation.md`. Code: the KAGEMUSHA scripts and their tests under `scripts/` and `pytests/scripts/` (21 Python files, 17,851 lines); `crates/iroha_kagami/src/kagemusha.rs` and its module (3 files, 3,727 lines); the governance release schemas in Swift, JavaScript, C# and Python | The release process of the recursive stack. No release keys exist (§2.2) | Kept. The key codec and layout authentication serve the keys generated for the new relation. Provider policy and physical evidence describe an OEM hardware provider; they are re-scoped or withdrawn in Q0. The record format of physical evidence is the pattern for the gate's records (10.4.7) | For a device support claim, the evidence gate (§10.4) | Not a candidate while row 2 is kept |
| 15 | Formal model. `formal/kagemusha_v1/` (5 files, 1,763 lines, TLA+) | `scripts/tests/kagemusha_formal_proof_gates_test.py` | Kept while row 6 exists. It models one hardware-enforced successor per state (`formal/kagemusha_v1/README.md:8-13, 21-26`), not this design | The model of the commit, marker, checkpoint and restore rules (§11) | With row 6 |

**The existing specifications.** Fourteen KAGEMUSHA specifications exist under
`specs/` besides this proposal and its evidence appendix. "Superseded" means
that the text must be rewritten or withdrawn if this proposal is accepted.
"Kept" means that it stays normative for a component that remains.
"Unaffected" means that nothing here changes it.

| Specification | Lines | Status if this proposal is accepted |
|---|---|---|
| `kagemusha_v1.md` | 346 | Superseded as the definition of the first-release protocol. Its recursive operations (`:53-65`) and reserve rules (`:253-286`) are the starting point for the relation that Q0 specifies |
| `kagemusha_guard_bundle_v1.md` | 241 | Superseded. The Guard is specified again against this design's objects |
| `kagemusha_receiver_admission_v1.md` | 110 | Superseded by the Receive and Outcome steps of §5.2 |
| `kagemusha_app_owned_hardware_v1.md` | 88 | Superseded in part. Its identity enrollment is kept as input to §7.1. Its approval message and its capability boundary (`:19`) are superseded |
| `kagemusha_v1_phone_algorithm.md` | 928 | Superseded in its goal statement (`:10-24`). Its device measurements (`:501-538`) are kept as evidence. Its Claim split (`:735-844`) is kept as input to Q0 |
| `kagemusha_v1_production_readiness.md` | 1,027 | Superseded in its current-profile record (`:8-25`), if the owner says so, and in its device gates (`:119-122`), which the targets of Q0 replace. The rest is a dated record of work on the recursive stack and is kept as such |
| `kagemusha_v1_provider_policy_binding.md` | 147 | Decided in Q0. The governed policy table that E names (§5.1) takes over admission policy for stock phones |
| `kagemusha_v1_physical_evidence.md` | 241 | Superseded as the contract for a device support claim; the evidence gate takes that role for stock phones. Kept while row 6 exists |
| `kagemusha_device_bridge_v1.md` | 651 | Kept unchanged while row 6 exists. Not used by this design. Its peer-protocol section (`:13-43`) is superseded by §5.2 |
| `kagemusha_device_sender_v1.md` | 86 | Kept unchanged while row 6 exists. Not used by this design |
| `kagemusha_pixel6_ese_service_contract_v1.md` | 89 | Kept unchanged while row 6 exists. Its statement that a Pixel 6 monetary profile needs an internal secure-element service (`:3-6`) is superseded |
| `kagemusha_v1_native_profile_binding.md` | 62 | Kept. Its layout table changes when the relation is fixed |
| `kagemusha_v1_compact_keys.md` | 98 | Unaffected |
| `kagemusha_v1_release_runner_validation.md` | 79 | Unaffected. It is a dated validation record of the release scripts |

**Normative statements this proposal would supersede.** Accepting this
design, with stock-phone keys holding offline authority, contradicts each
statement below. Each has to be rewritten or withdrawn in the same change
that makes this design's specification normative. That change comes after the
gate's ruling (§11).

In `specs/kagemusha_v1.md`:

- `:3-6`. Recursive V1 is the sole first-release protocol, with one decoder
  and no protocol selector. Change: this design becomes the first-release
  protocol; while both exist, the profile code is a selector (§11).
- `:26-45`. One hardware-controlled private state per lane, with the balance
  private and qualified hardware holding the authoritative root. Change: the
  state is kept by the app, with the current state in a marker in the phone's
  key store (§5.10), and the receiver sees the payer's totals (§5.7).
- `:64-65` and `:317-319`. Rotate moves the balance to the next hardware
  epoch with no online step. Change: Migrate needs the issuer (§7.2).
- `:171-174`. Any number of payments against one Request are all accepted.
  Change: a Request is single-use (§5.2).
- `:176-179`. Request expiry is judged on the sender's trusted hardware
  commit time. Change: no phone gives a trusted time; §5.4 applies.
- `:183-194`. Offline authority requires an attested non-forking provider
  that meets nine listed requirements, the last being no software fallback.
  Change: withdrawn. §4 states the assumptions the design rests on, and §4.1
  states that no target phone is shown to enforce one successor per state.
- `:196-203`. A host-side signature or certificate alone grants no monetary
  authority, and stock KeyMint, StrongBox, Secure Enclave and App Attest stay
  online-only. Change: withdrawn. The authority is a device signature under a
  certificate, a receipt and an enrollment statement, together with the
  proof.
- `:230-233`. A committed amount is bound to the receiver and an exposed
  credit cannot be cancelled. Change: under §5.2 the receiver may answer
  `Refused` before it credits, and the payer then refunds.
- `:248-251`. Stable credentials and counters are not visible to a peer.
  Change: §5.7 lists what each side learns on the last hop.
- `:257-259` and `:273-278`. The reserve equals top-ups less redemptions, a
  redemption pays the requested account, and a redemption above the reserve
  is rejected. Change: the recorded claims of §8.2 and the mechanics of §8.4,
  and an unload pays only the account bound to the device id (§7.1).
- `:280-281`. The pool has no claims and no buckets. As read here, that
  excludes the recorded unload claims of §8.2. Change: reworded.
- `:288-296`. Four online routes with V1-only schemas. Change: routes for the
  instructions of §6.
- `:90-92`, `:300-303` and `:344-346`. A payment carries one constant-size
  proof of at most 6,528 bytes, and host-only signatures do not establish
  offline monetary authority. Change: the first half stays, with the size as
  a target that Q0 fixes; the second half is reworded as at `:196-203`.

In the other specifications:

- `specs/kagemusha_guard_bundle_v1.md:3-7`. The GuardBundle is the only
  offline hardware-authority path, and a platform signature or attestation
  chain is no substitute. `:29-46`. The capability set is indivisible, and a
  profile missing any capability is online-only. Change: both withdrawn.
- `specs/kagemusha_device_bridge_v1.md:3-6` and `:635-644`. The only ABI is
  to a qualified non-forking hardware service, with no software fallback.
  Change: re-scoped to the secure-element path of row 6.
- `specs/kagemusha_device_sender_v1.md:5-7`. Stock dispatch returns
  unavailable until a qualified provider is installed. Change: re-scoped to
  row 6.
- `specs/kagemusha_receiver_admission_v1.md:3-6` and `:26-28`. A payment is
  accepted into a rollback-resistant hardware inbox, there is no cancellation
  protocol, and a host signature without Guard verification grants no
  authority. Change: superseded by §5.2.
- `specs/kagemusha_pixel6_ese_service_contract_v1.md:3-10`. A Pixel 6
  monetary profile requires a provisioned internal secure-element service,
  and the stock Pixel 6 is not qualified. Change: a Pixel 6 tuple is
  supported, or not, by its gate finding (§10.1).
- `specs/kagemusha_v1_phone_algorithm.md:10-24`. An ordinary app may transfer
  value offline only when a hardware primitive authorizes at most one
  successor. `:508-511`. An attested app and a StrongBox key alone cannot be
  substituted for that. Change: withdrawn; §4.1 says no target phone has the
  primitive and what follows.
- `specs/kagemusha_app_owned_hardware_v1.md:19`. App enrollment does not
  grant offline spending, and a reusable P-256 signature is not a one-use
  authorization. Change: the second half stays true and §4.1 says so; the
  first half is withdrawn.
- `specs/kagemusha_v1_production_readiness.md:8-25`. The owner requirement of
  2026-10-01: the ordinary app profile with BPNG, BOI and CBSI adapters, and
  Play Integrity required at enrollment and refresh. Change: replaced by this
  proposal if the owner says so (§12). `:351-354`. A Pixel 6 needs a
  provisioned hardware counter or another proven no-fork primitive for
  production offline money. Change: withdrawn. `:119-122`. Device gates of
  10 s proving, 1 s verification and 30 s handoff. Change: the targets that
  Q0 fixes.
- `specs/peer_transport_v1.md:3-15`. Exactly three message kinds, the third
  an Acknowledgement sent after staging in a rollback-resistant inbox, with
  no cancellation kind. `:65-71`. The size table. `:78-79`. A smaller limit
  cannot be advertised as an offline-capable profile. Change: the messages of
  §5.2 and the sizes of §5.6.
- `specs/qr_stream.md:13-17`. The payload is one of the three V1 values.
  Change: decided when the normative specification names one framing (row
  9).

Outside `specs/`:

- `roadmap.md:94-96` (outcomes S7, S8 and S9) and `roadmap.md:151-154`, and
  `status.md:24` and `status.md:218-221`. The first production app profile
  requires genuine monetary proofs and current-owner authority, and durable
  money requires an exact-next successor and trusted time with no software
  fallback. Change: rewritten to this design's outcomes. "Genuine monetary
  proofs" stays.
- `AGENTS.md:26-27`. Parallel implementations are prohibited in the first
  release. Change: none to the text; the coexistence of §11 is an exception
  to it until the removals. `AGENTS.md:30-32`. Hardware-dependent guarantees
  must not be relabelled as software guarantees, and a KAGEMUSHA offline
  monetary-authority policy is said to follow "below". No such policy text is
  in that file. Change: the policy is written there, stating the assumptions
  of §4 and what §2.1 says a proof shows, and no more.
- `formal/kagemusha_v1/README.md:8-13, 21-26`. The model assumes one
  hardware-enforced successor per state. Change: it stays the model of row 6
  only.
- `docs/bpng-retail-daily-limit-admission.md:47-50`. KAGEMUSHA top-up and
  redemption stay closed for a retail-governed asset until the owner approves
  a typed treatment. Not contradicted. It gates Load and Unload for such an
  asset (§12).
- `scripts/tests/kagemusha_hard_cut_test.py:35-40`. The names `KagemushaV2`,
  `KagemushaV4`, `KagemushaV5` and their lower-case forms are retired and
  guarded. Not contradicted. This design's objects cannot take those names.

**Smart cards.** The owner said: "right now smart cards are future
optionality to explore but there are no plans right now to use." Nothing is
built for them: the repository has no applet, this proposal defines no card
enrollment class, and no object of §5.1 has a card field. If the option is
taken up, a card applet would reuse four parts:

- the one-successor rule of the secure-element contract: one reserved
  predecessor, one stored outcome, the same bytes on a retry, and a conflict
  for any second successor
  (`specs/kagemusha_pixel6_ese_service_contract_v1.md:30-46`);
- the command framing: the command and response frames of
  `specs/kagemusha_device_bridge_v1.md` and the APDU transport of
  `specs/kagemusha_pixel6_ese_service_contract_v1.md:20-28`;
- the NFC carrier over ISO 7816 commands (§5.6);
- the Guard for hardware that enforces one successor
  (`specs/kagemusha_guard_bundle_v1.md:29-41`), which is the case in which
  §10.4 could find "supported".

A card would also need an enrollment class that is not phone attestation
(§12). This document's reading is that such a class would enter as a new
platform class with its own enrollment statement (§5.1, §8.1). That has not
been designed. The removal decision for row 6 says whether the first two
parts are kept.

## 12. Decisions needed from the owner

The decisions on which the rest depends are the twelve choices of §3.2. §0
names six decisions: choices 1, 2 and 3 (as one), 4, 5 and 12 of §3.2, and
the one-design decision of §11.1. The lists below add the decisions each part
of the design puts to the owner. This document takes none of them. Some
overlap, because the same question arises in more than one section. The same
decision is asked in items 1 and 34; 3 and 79; 5, 18, 67 and 81; 6, 33 and
82; 7, 48 and 68; 9 and 60; 10 and 45; 11, 22, 23, 51, 52, 66 and 85; 12, 40
and 62; 13, 19, 32 and 91; 15 and 94; 25 and 92; 37 and 77; 38 and 50; 39 and
46; 49 and 65; 53 and 66; 73 and 89. One answer settles each group.


**Criterion, assumptions and residual risk (§1, §3, §4)**

1. Confirm or correct each reading of the criterion in section 1: 'completed'
   means the receiving wallet reported complete; 'durably' means as durable as
   the phone and not beyond the destruction of the key; P4 covers reduce,
   refuse, hold and delay; what 'require connectivity' covers.
2. Whether P1, P3, P5 and PC may rest on assumptions at all. The criterion puts
   'under the stated security assumptions' on P2 only; the design claims the
   others under T1-T5 on the holder's own phone and the issuer side, and P1
   also under T6 (section 3.1 group C items 1 and 2).
3. Whether P2 must hold against a compromised operating system. If yes, only
   the gate finding 'supported' meets it, no tuple is expected to reach it, and
   the options are hardware that runs wallet logic or no production release on
   stock phones. If no, 'supported with a stated assumption' meets P2 as
   written (section 3.2 choice 1). This also settles how 'we cannot allow
   compromised OS' is to be taken, given that the proof shows boot-time facts
   only.
4. Which phones may hold value: every phone that enrolls, only tuples the gate
   supports, or no stock phone above a small amount (section 3.2 choice 2).
5. Which platform classes a pool admits when it claims the operating-system
   constraint: iPhones, phones with factory-provisioned attestation keys,
   HarmonyOS NEXT. A pool's claim is that of its weakest class (section 3.2
   choice 3).
6. Whether iPhones hold value while P1 there holds only as long as a passcode
   stays set, and while P2 there against an ordinary user is not shown; or wait
   for the gate's durability test and the second anchor's tests (section 3.1 A3
   and B1).
7. Which regulatory controls are on by default (section 3.2 choice 4), and
   whether the six additions count as explicitly enabled regulatory controls:
   receive freshness, send_blocked, the per-counterparty cap, require_anchor, a
   tier-row notice that switches a control on for certificates already issued,
   a block at the holder's own request (section 1).
8. Who supplies a difference between redemptions and loads after an assumption
   has failed: the operator, the asset's issuer by new issuance, a fee-funded
   reserve, a third party, or nobody (which fails P1 or P4 in substance). None
   is chosen (section 3.2 choice 5).
9. The release rate on unloads above a row's own loads: no limit, or a limit;
   and with it whether P1 and P3 govern redemption (section 3.2 choice 6;
   section 3.1 C3).
10. Certificates that never expire, after an issuer key is stolen: P5 holds
    whatever happens to issuer keys (no forced sync, no time bound), or P5
    holds under T4 (a period to sync is allowed) (section 3.2 choice 7).
11. What a renewal under R8 may check: whether an app-build floor is part of
    R8; the patch floor's value; whether a phone past its vendor's update
    commitment (Pixel 6, October 2026) stays under it (section 3.2 choices 8
    and 9).
12. What a held phone's holder gets back and who may order it, where the two
    signatures came from a tuple fault, someone else's malware, or a phone the
    holder had migrated from; and whether to pursue recovery from the bound
    account (section 3.2 choices 10 and 11).
13. What follows if the proof cannot be made in an acceptable time on the
    supported phones, and the end points and percentile of the 1-2 s target, to
    be fixed before the first measurement (section 3.2 choice 12; section 1).
14. What a root-key compromise voids (section 3.2 table, root-key row).
15. Whether R1-R9 and the criterion override the June 2025 document where they
    disagree (section 1).

**The proof (§2)**

16. Confirm the reading in §2: every payment carries a proof, the relation
    covers every hop, and it includes an enrollment statement E for every
    paying phone.
17. Accept or reject the true reading of "the OS is real" as what the proof
    carries (§2.1, 'The two readings'): boot state, patch level and OS-reported
    app identity as of enrollment or last renewal; not run-time integrity,
    which no platform gives evidence of; on iPhone only 'a genuine Apple Secure
    Enclave acting for this App ID'.
18. Which platform classes a pool that claims the OS constraint admits:
    iPhones, Android phones with a factory-provisioned chain, HarmonyOS NEXT,
    and Android phones on which the app attestation key cannot be created
    (§2.1, 'Platform classes and pools'). The pool's claim is that of the
    weakest class admitted.
19. The time target for a proof-carrying payment: the figure, the percentile,
    the end points (to the receiver's credit, or also to the payer's
    'complete'), and whether it is the same figure as for the exchange without
    a proof (§2.3, target table).
20. Every other target in the §2.3 table, the budget and date of each stage,
    the named stage owners, and the number of reconsiderations of the relation
    or proving system (one is the value to confirm or change).
21. Whether the validator quorum alone is an acceptable basis for E if the
    server-made enrollment proof cannot be built within the host budget (§2.1
    'Who establishes E'; §2.3 Q1 outcome).
22. The patch floor: its value per platform, and whether it applies to a phone
    whose vendor has stopped updates (Pixel 6, October 2026). It bites only
    where R8 is on (§2.1 'Policy inputs').
23. Whether a policy entry may hold a minimum app version code, applied at
    enrollment and renewal (clause EA7); this is the app-build floor question.
24. Vendor attestation-key revocation: keep the design rule (an already sealed
    E stands until the lease ends, without limit where R8 is off), or also let
    the block authority enter such a device under R6 so that receivers holding
    the list refuse it at once, which stops honest phones of a leaked factory
    batch (§2.1 'Policy inputs').
25. Whether Play Integrity or Apple's receipt and fraud metric are required
    fields of E; each rests on the key of the party that made the vendor call
    (§2.1).
26. Whether the mint stays sealed by the validator quorum inside the relation;
    if not, a stolen voucher key mints value every proof accepts (§2.3 'Mint
    authorization').
27. Whether Android device keys are generated with a caller-supplied digest
    authorized from the first enrollment; choosing it later re-enrolls every
    Android phone (§2.3 'Algebraic signed digest').
28. Whether "no trusted setup" binds here, and whether the 10 KB bound or the
    phone list may be relaxed for one more cycle (§2.3, the ruling and
    candidate shape 4).
29. If a bounded native tail is to be evaluated: whether a wait on the phone
    itself, offline, before a later spend leaves P1 intact. Without that
    reading the shape fails PC (§2.3, candidate shape 1).
30. Whether the optional per-hop evidence is wanted: a fresh Android leaf per
    payment or per stated number of transitions, or an App Attest assertion per
    hop on iPhone, with the costs §2.1 states. Neither shows a run-time
    compromise.
31. Whether the relation of §2.1 (one platform equation per hop, the issuer's
    part moved to enrollment) meets the repository's record of the owner
    requirement of 2026-10-01 (§2 'Position').
32. The ruling if no shape meets PC in a time the owner accepts, among the
    options listed in §2.3; the document chooses none.

**Exchange, evidence, marker (§5.2, §5.3, §5.10, §9)**

33. iPhone passcode: accept 'the passcode is neither removed nor reset' (T6) as
    a condition under which P1 is claimed, knowing that removal or reset
    deletes the marker and puts the balance out of reach; or fund the tests of
    a second anchor (App Attest counter) whose four required properties are not
    established. The text keeps the condition and says the design does not
    remove this loss.
34. Whether 'durably' in P1 may rest on a condition on the holder at all:
    destroying the device key (lost phone, erase, Android uninstall without
    keeping data, clear storage) destroys the balance on any single-phone
    design.
35. iPhone key-store durability: if the first iPhone gate test fails and no
    barrier works, choose among accepting the exposure (pay, force restart,
    delete app, reinstall, pay again), having an iPhone wallet stop when its
    files are absent (an honest holder then loses the balance after an app
    deletion or a one-app restore), or treating the tuple as unsupported.
36. If a wait before release is the barrier that works on iPhone: whether its
    length is acceptable inside the payment time.
37. The checkpoint caps: 4 Requests listed, 2 complete and 6 short unresolved
    SendSplits, 4 remembered decisions. Larger caps protect counterparties'
    refunds after a file loss and cost a larger key-store write per signed
    object.
38. Whether the block list is also kept in the key store, so that a resume with
    R6 on never waits for the list (about 0.1 KB per segment and 40 B per
    entry).
39. Whether the per-payer tally restart after a resume is acceptable, or the
    wallet should refuse limited payers until the window turns.
40. What the holder of a held row gets back, and who may order it, where the
    two signatures came from a platform fault, from malware, or from an old
    phone whose key signed after a Migrate. Until this is decided a hold has no
    end.
41. Whether the issuer relays a Payment or Outcome that never crossed when both
    sides later sync (an undelivered payment can otherwise cost an honest payer
    the amount, and after the receiver loses its files the loss is permanent).
42. The Android manifest choices: a no-op backup agent with the backup
    opt-outs, hasFragileUserData, manageSpaceActivity, not direct-boot aware.
43. Whether the proof-carrying design may go ahead before Q0 has fixed a
    relation with a bounded witness: until then a wallet that lost its files
    keeps its balance and cannot pay it offline under the proof, nor unload
    it while a RedeemSplit needs its proof (sections 5.2, 8.2).
44. Whether a tuple may use the certificate-only Android marker (no key
    generation on the payment path) where the gate shows it survives
    screen-lock removal, or one form is required on every Android tuple.

**Objects, time, block list, fees, authentication, versions (§5.1, §5.4, §5.5,
§5.8, §5.9, §5.11)**

45. `Never` certificates after a theft of the certificate key and the witness
    quorum: do they stand without limit of time, or for a root-signed period
    after which the holder must sync once? 5.1 carries the answer as the notice
    field `never_stand`; the choice itself is in section 3.2.
46. Receiver tally after a resume (5.4): restart from the credits the files
    still hold (one more limit per resume against a compromised payer), or keep
    two amounts in the marker so no payer's limit can be passed, at the cost of
    an honest receiver refusing smaller-limit payers until the window turns?
47. May a tier combine a `Never` certificate with day or month limits, given
    that a lost phone's share then never returns to the account (5.4)?
48. Confirm as explicitly enabled regulatory controls the additions beyond the
    owner's words: `require_anchor`, receive freshness (`receive_not_after`),
    the per-counterparty cap (5.4), the `send_blocked` flag (5.5).
49. Holder-requested blocking (5.5): may a scheme switch it on, and does it
    then count as an explicitly enabled regulatory control? Where on, whoever
    holds the account key can stop a working phone paying among holders of the
    list.
50. After a resume with R6 on (5.5): accept that the wallet neither pays nor
    requests until it holds its block-list version again, or keep the list in
    the key store (about 0.1 KB plus 40 B per entry per segment) so that
    nothing stops?
51. Is an app-build floor part of what R8 checks at renewal (5.11)? If yes it
    is a reason to refuse a renewal and old builds die out within one lease; if
    no, no renewal is refused for a build or version and a defective build is
    retired only by its users updating.
52. The patch floor (5.11): its value per platform class, and whether it
    applies to phones past their vendor's update commitment (Pixel 6: October
    2026). Such a phone cannot send after its lease and its holder must Migrate
    or unload.
53. Does a policy entry that stops admitting a platform class also end renewals
    for phones of that class already enrolled (5.11)?
54. Does a tier-row notice that brings in a lease for certificates issued
    without one count as the scheme explicitly enabling R8 (5.11)? If not, a
    new lease reaches a certificate only at a sync its holder chooses.
55. Renewal now needs k of n witnesses as well as the issuer and one final
    ledger write, because every certificate has its own receipt (5.1, 5.11).
    Accept that dependency under R8, in exchange for a stolen certificate key
    alone being useless offline?
56. Fees (5.8): who pays (payer on top, or receiver out of the amount); one
    beneficiary per scheme or per issuer; taxes; settlement per payment
    on-chain or in aggregate on the issuer's word; the value of `fee_limit`,
    including no limit.
57. User authentication (5.9): does the platform prompt before signing, with no
    requirement on the key, meet "generally it should be related to the secure
    hardware"? Also the default mode (a short window is recommended) and
    whether a tier may refuse to pay on a phone with no screen lock.
58. Closure (5.11): may the root bring in a lease for every tier and may the
    issuer stop renewing after a date so that value comes home; does a legal
    dormancy rule count as a regulatory control; who pays unloads if the chain
    itself is retired.
59. Settings carried over (5.4): `window_future_tolerance` of 2 hours
    (proposed, not measured); the limit day as the fixed UTC day; `wall_clock`
    as the default reboot policy.

**Roles, flows, ledger (§6, §7, §8)**

60. unload_limit (section 8.2): a per-row release rate above a row's own loads,
    or no limit. With a limit an honest merchant who loaded nothing is paid one
    limit per window while every assumption holds. With no limit a row whose
    key signs one oversized RedeemSplit is due everything at once and can empty
    the pool in one instruction where an unload needs no proof. The text states
    both and chooses neither; a per-tier value is part of the same choice.
61. Whether an unload must carry a proof under the proof-carrying design
    (section 8.2, fixed with the relation in Q0). Required: claims are bounded
    by a proven balance, and a phone that cannot prove cannot unload. Not
    required: the chain checks a signature only, and a phone that cannot
    prove can unload only if its wallet may also commit a RedeemSplit without
    a proof, which section 5.2 does not allow.
62. What ends a hold (sections 8.2, 6): what the holder gets back, on which
    device id, and who may order it. Until decided a held balance stays held,
    and a hold can fall on an honest holder through malware or through an old
    phone that was passed on before its key was deleted.
63. Load while the pool is short (section 8.4): closed (no new holder's cash
    pays earlier claims, and no cash arrives from loads) or open (each new load
    pays the queue and the new holder's value has less cash behind it). Also
    whether a stated amount of added cash must be in the pool before Load
    opens; who would pay it belongs to section 3.2.
64. Witness model (section 8.1): (A) independent verification, recommended with
    one witness outside the issuer operator's control, or (B) notary with a
    seasoning delay on every first receipt; k and n. Also whether a receipt is
    wanted at all once a proof starts from the validators' seal, and whether
    the ledger should again pay a row above its own loads only while a receipt
    is recorded on-chain (dropped in this text, with the reason).
65. Holder-requested blocking (sections 7.2, 6): off, so that whoever holds the
    account key cannot stop a live phone and a lost unlocked phone can be spent
    like cash; or on as part of R6, so that the account can block its own
    device among holders of the list. And whether it then counts as an
    explicitly enabled regulatory control.
66. The content of R8 at renewal (section 7.1): the patch floor and whether it
    applies to phones past their vendor's update commitment (such a phone can
    never send again after its lease, only unload or Migrate); whether an
    app-build floor is checked; whether a list of OS major versions is checked;
    whether a policy entry that stops admitting a platform class also ends
    renewals for enrolled phones of that class.
67. Which platform classes a pool admits (section 8.1): iPhones, phones with
    factory-provisioned attestation keys, HarmonyOS NEXT. The pool's claim is
    that of the weakest class admitted.
68. Which of the document's additions count as enabled regulatory controls
    (section 7.1): receive freshness, send_blocked, the per-counterparty cap,
    require_anchor, a tier-row notice that switches a control on for
    certificates already issued, a block at the holder's request.
69. A load left with neither a voucher nor a void statement (section 7.1):
    accept that no rule ends the wait and that the wallet cannot load again
    meanwhile, or anchor every voucher on-chain (one more write per load) so
    the chain can refund after a time limit.
70. Migrate (section 7.2): accept that the old wallet's own SendSplits with no
    stored Outcome are abandoned (the app lists them first), or have the
    Migrate carry them so the successor can refund on a late Refused Outcome
    (not designed).
71. Limits after a lost phone whose certificate never expires (section 7.2):
    never return the lost device's share, or return it after a set period and
    accept that the account can then exceed its limit through the old phone if
    it still works.
72. Recovery insurance (section 7.3): whether a scheme may enable it, the cap
    per account per period, whether a delay before payout is wanted, and who
    pays into the recovery account. The text says an ordinary user can claim
    and keep spending, so the insurer should expect every unit paid in to be
    drawn.
73. Privacy of the ledger (section 6): every registration puts raw vendor
    evidence on a public ledger (boot state, patch levels, app identity,
    whatever the vendor's chain carries), and every renewal and sync shows when
    a device went online. Keep, or anchor heads as one digest per period, or
    record serials ahead where R8 is off.
74. A lost account key (section 8.2): accept that the wallet can pay offline
    but cannot unload or Migrate, or define a governed rebinding, which is a
    theft path for whoever controls it.
75. Whether an unload payout to an account that is blocked on the ledger is
    withheld or paid (section 8.2); the claim stays recorded either way. With
    it: who the block authority is, or which existing ledger fact stands
    behind the R6 list (sections 6, 8.2).
76. Who may add serials to the revoked set between policy entries (sections 6,
    8.1): the registry authority, as drafted, which lets a thief of that key
    make honest renewals fail until a new policy entry is installed; or only
    root-signed policy entries, which makes daily revocation updates a root-key
    operation.
77. How many open Requests, decisions and unresolved SendSplits the marker
    keeps (section 7.2, set in 5.10): larger numbers protect other parties'
    refunds after a loss of files and cost a larger key-store write per signed
    object.

**Platforms, evidence gate, order of work (§5.6, §5.7, §10, §11, §14)**

78. Fix the list of tuples for the first cut (model, OS major version, vendor
    build family, key-store security level), including whether 'Huawei' means
    EMUI/HarmonyOS up to 4 or HarmonyOS NEXT and whether Meizu stays (§10.1,
    §10.2). The text proposes starting with iPhone and Pixel tuples and adding
    others only with a captured chain and a gate finding.
79. Must P2 hold against a compromised phone? If yes, only the finding
    'supported' meets it and no tuple is expected to reach it; if T1 to T3 are
    accepted as the stated assumptions, 'supported with a stated assumption'
    meets P2 as worded and the P4 consequence goes to the residual-risk section
    (10.4.6).
80. Factory-provisioned attestation roots: refuse them (excludes leaked factory
    keys, and also the recorded Pixel 6 StrongBox chain and older
    Xiaomi/OPPO/vivo generations) or admit them as a separate platform class
    (those phones enroll; an unlisted leaked key then enrolls software keys)
    (§10.3).
81. Which platform classes a pool admits: iPhone (no OS statement),
    factory-chain Android, HarmonyOS NEXT (no OS claim). The pool's claim is
    that of the weakest class admitted (§10.1, 10.4.6).
82. iPhone passcode: may P1 rest on the holder keeping a passcode set and not
    resetting it (T6)? The text states this as not met by design and does not
    present it as satisfying P1 (10.4.6).
83. For each gate result 'destroyed on a path T6 does not name', and for an
    Android release on which no marker form survives screen-lock removal: is
    the tuple unsupported, or is the holder's action added to T6 for that tuple
    in writing (10.4.6)?
84. Gate inputs before any test: the time target for the exchange without a
    proof, its percentile, its end points (to the receiving wallet showing
    complete, or also to the paying wallet) and whether it is a gate;
    the number of trials for durability tests (100 bounds a rate below 3%,
    1,000 below 0.3%); whether a true power cut on an opened phone is within
    what an ordinary user can do; the patch floor h1 applies (10.4.4).
85. Patch floor at renewal: its value, and whether a phone past its vendor's
    update commitment (Pixel 6, October 2026) stays under it; whether a gate
    finding for such a tuple expires; the re-run policy per OS major version
    and tool version (§10.3, 10.4.6).
86. Is a tuple without a working app attestation key admitted, given that its
    renewal can only show the app that holds the device key and not the same
    secure hardware (§10.3)?
87. Unit of R9: 10,000 bytes of canonical binary, or of the text form a code
    shows? The proof-carrying Payment is about 7.6 KB binary and about 10.2 KB
    in the repo's text form (§5.6).
88. Carriers a wallet must support; whether to build and measure a Bluetooth LE
    carrier; whether to encrypt Payment and Outcome against bystanders, about
    100 bytes (§5.6).
89. Privacy of enrollment evidence: raw vendor attestation on the chain so that
    validators verify it themselves, or only a digest so that it is hidden and
    validators cannot (§5.7).
90. One design: may this design replace the attested-app suite in place, and
    does any app outside the repository use it; does this proposal replace the
    owner record of 2026-10-01; is the coexistence of four tracks accepted as
    an exception to the no-parallel-implementations rule until the removals
    (§11, §11.1)?
91. If Q0 to Q3 end without a proof that meets PC in a time the owner accepts:
    choose among relaxing one named constraint for one more cycle, a narrower
    device list, a candidate shape after design and check, the signature-only
    form with what it lacks, hardware that runs wallet logic, or no production
    release (10.4.6).
92. Play Integrity: required at enrollment and renewal (excludes phones without
    Google Play) or an optional issuer signal; and whether iPhone tiers carry
    lower limits while test h7 is open (§10.3).
93. Give the evidence gate a budget, a date and a named owner; approve spare
    units (rooted or userdebug unit per Android model, opened units for power
    cuts, a jailbroken legacy Apple device) and an application for Apple's
    Security Research Device (10.4.5 groups e, f; 10.4.7).
94. Confirm that the owner's later statements override the June 2025 document
    wherever the two disagree, and decide the controls that document lists and
    the owner did not name: balance ceiling, cap on number of payments,
    automatic freeze; and whether a minimum app build is part of a renewal
    under R8 (§14). §14 also names points that have no other item here: one
    issuer or several, privacy tiers, other device classes and the design
    outage. §11.1 points here for two more: an enrollment class for cards,
    and the typed treatment a retail-governed asset needs before Load and
    Unload open for it.

## 13. Evidence

[`kagemusha_single_design_evidence.md`](kagemusha_single_design_evidence.md)
holds the sources. Its section 0 is a claim map made for revision 5; sections
1 to 7 are older research; section 8 lists the sources for what revision 6
added: the study of what evidence exists that a phone runs a genuine operating
system, and the acceptance-criterion design. The marker that carries a
checkpoint has no separate source list; its platform statements are cited
where §5.10 makes them. The test procedures of the evidence gate are in
[`kagemusha_evidence_gate.md`](kagemusha_evidence_gate.md), with the sources
for its platform statements in its part 10.4.10.

## 14. The June 2025 TC3 document

"TC3: Offline Capabilities" is a response to a central-bank consultation. Its
header says it was written with input and technology from the owner's
company. It is not in the repository. The owner supplied it on 2026-10-02
with these words: "for reference, a lot of the ideas/reqs we have for offline
can be found here, but this is an old doc and not fully up to date". The date
is the owner's; the text read here carries none. An automated comparison
produced about ninety findings. This section keeps those that change the
design or need a decision. Where the 2025 document and a later statement of
the owner disagree, this document follows the later statement and says so at
that place.

**Its design.** A hardware key; an attestation certificate from an offline
certificate authority; a signed transaction shown as a QR code; an optional
countersignature by the receiver; upload of every transfer at the next sync.
That is signatures only, with no proof. A payment that has passed through
several hands is read here as carrying the signed record of each. The
document says only that each hop adds a signed record and that the first sync
carries the full history; its payment message holds the signed transaction
and the certificate.

**Why a proof takes the place of the signed history.** The owner said: "to do
device to device transfers, we really cannot have payment data exceed around
10k or so, which is why we were looking at using cryptographic proofs to
proof in a concise way correctness". One earlier hop costs about 0.85 KB as
signed records (certificate, receipt, signed transition; an estimate). A
payment passes 10 KB after about eleven earlier hops, or at once when the
payer's balance merges about eleven received payments, because each one
brings its own history. R4 and R9 together exclude that. The proof of §2.1
checks every earlier hop at a constant size. No such proof has been produced
yet (§2.2). A payment in the signature-only form does not check earlier hops
at all (§2).

**Wallet logic in secure hardware.** The 2025 document has the balance, the
limits and the one-time-spend counters kept and enforced inside the secure
hardware. The owner has since said: "we don't assume running in secure
hardware as we don't have oem access and instead plan to use hardware backed
keys and some unique counter/commitment to prevent reset and double spend".
This design follows the later statement. Apple's and Android's secure
hardware perform key operations for whatever the operating system asks and
run no app code. What the design does with each part of the owner's sentence:

- Hardware-backed keys. The device key is held in the TEE, StrongBox or
  Secure Enclave (T1). On Android an app attestation key attests it (§10.3).
- A counter or commitment against reset. The marker with its checkpoint
  (§5.10). The app keeps it in the phone's key store; the secure hardware
  does not enforce it. Under T1 to T3 it stops an ordinary user from bringing
  back an earlier state. The evidence gate tests that per tuple (§10.4).
- Against double spend by a compromised phone. No counter or commitment that
  the secure hardware enforces against its own operating system was found on
  any target phone (§4.1). Groups e and f of §10.4 are the tests that could
  show one. Until one does, P2 rests on T1 to T3 on the paying phone.

**Attestation, the valid app and the operating system.** The 2025 document
says that attestation proves the wallet's keys are in secure hardware and
that cloned devices are blocked by attestation checks. The owner's statement
of the security model is: "our security model requires us to have a hardware
backed key that is used to attest that our signing key/state is valid and the
tx is from a valid app." The owner later added: "because we cannot allow
compromised OS, we should include as part of our proof constraint matrix that
the OS is real and prove it somehow". What stock phones give for each part
(§2.1, §4):

- The signing key. On Android the attestation covers the device key. On
  iPhone Apple attests the App Attest key only, and the payment key is a
  second key that nothing attests.
- The state. No platform attests wallet state. The proof shows that every
  transition behind a balance was valid and was signed by an enrolled key. It
  does not show that a state had one successor.
- A valid app. The operating system names the app when a key is attested: at
  enrollment, and at each renewal where R8 is on. No payment carries platform
  evidence of the app that produced it.
- The operating system. On Android the enrollment statement says how the
  phone booted and at which patch levels, as of enrollment or the last
  renewal. On iPhone and on HarmonyOS NEXT it says nothing about the
  operating system. On no platform does anything show that the running
  operating system is uncompromised when a payment is signed. A compromised
  phone that copies its wallet state and pays twice passes every attestation
  check.

**Where it disagrees with itself.** On two points the document says two
things. The acceptance criterion settles both.

- Finality. It says the payment is complete once the receiver has verified
  and recorded it, and that the receiver can spend the value again offline.
  It also says that at sync the server honours the first valid spend and
  rejects the rest, and that value from a wallet reported stolen could be
  refused. P3 and P4 exclude the second pair. The owner said: "offline
  payments must be secure and final. once you transfer from one phone to
  another, it must be a final transfer of value there, with no need to ever
  go online again unless there are regulatory controls".
- Forced sync. Its summary recommends limits on transactions only, and
  recommends against holding limits and synchronization requirements. Its
  policy section then lists a balance ceiling, a forced resync after a number
  of payments, a value or a time, and an automatic freeze. P5 allows a rule
  to require connectivity only if it is an explicitly enabled regulatory
  control. The owner named three: "a blacklist of accounts that users that
  have the blacklist won't send to", "some notion of daily or monthly limits
  that can be optionally set", and "some optional expiry for attestation so
  users will have to sync online before they can send offline again,
  optionally". A resync after a time is the third, where a scheme switches it
  on. A balance ceiling, a cap on the number of payments and an automatic
  freeze are not among them (§12).

**Carried over.** An attested hardware key and a certificate shown in every
payment. A request, then a signed payment. The payer debited at signing. A
payment that is complete once the receiver has verified and recorded it; PC
says exactly when (§5.2). The receiver able to spend again offline, for any
number of hops. Synchronization that is optional. A block list refreshed at
sync. Optional limits and expiry, which stop sending and leave receiving
alone; here only the block-list freshness control of §5.5 stops a wallet
requesting. Higher limits by tier for named users. Unload only to the bound
account. A lost phone is a lost balance (T6). PIN or biometric before paying:
the owner said "pin/biometric is a ux functionality but generally it should
be related to the secure hardware on a phone", and §5.9 has the platform
check it. Optional fees: the owner said "optional fees sound good, but can
only be received to an online account when someone syncs" (§5.8).

**What the acceptance criterion rules out.**

- Every offline transfer replayed on the ledger between per-wallet offline
  accounts. P3: a completed payment does not depend on later reconciliation
  or settlement. Here the ledger holds a pooled reserve. It never executes an
  individual offline payment and never reverses one. What it sees is listed
  in §5.7.
- "Honors the first valid spend, rejecting the rest." P4 and P3: that takes
  value from a receiver after the transfer, through someone else's act. Here
  a received payment is never reduced or reversed. If an assumption fails and
  two receivers hold value from one state, both keep it; §3.2 says what
  follows and which decisions are the owner's.
- Refusing value that came out of a wallet later reported stolen, "at the
  cost of the innocent payee". P4. Here a block entry against a payer stops
  later payments among holders of the list (§5.5). It does nothing to value
  already received.
- A forced resync or a freeze that no enabled regulatory control requires.
  P5.
- An override code obtained by telephone for one large payment. R1 allows no
  call to authorize a payment.

**Two statements this design does not keep, for other reasons.**

- "The Backend system thus always knows the total amount of Digital Shekels
  that are in circulation offline." While T1 to T5 hold on every phone and
  key, the ledger's figure, loads less what was paid out, is the offline
  total, lost balances included. When an assumption fails on some phone it is
  not, and the ledger cannot see the difference (§3.2).
- An expired certificate that still pays when there is no connectivity.
  Where the scheme has switched R8 on, sending stops after `expiry_grace`.
  Where R8 is off, a certificate does not expire. The owner's wording of R8,
  quoted above, decides this.

**Statements that do not hold on stock phones.**

- Balance, limits and one-time-spend counters kept and enforced inside the
  secure hardware. See above.
- Cloned devices blocked by attestation checks. Attestation is made at
  enrollment, and at a renewal where R8 is on. It does not see a compromised
  phone copy its wallet state and pay twice (§4.1).
- SafetyNet was shut down in January 2025. Play Integrity gives a verdict
  about the app and the device through Google's servers; it does not attest
  a key and cannot be used in an offline payment. Key attestation and App
  Attest attest keys (§4, §10.3).
- Payment by Bluetooth advertising. An iPhone app can advertise 28 bytes. A
  payment needs a connection. The platforms allow one between store apps;
  the repo does not implement one (§5.6).
- Tap between any two phones (§5.6).
- Anonymous wallets that can also be traced to an owner through provisioning
  records. Attestation open to a store app gives no unique device identifier,
  so an un-identified wallet traces only to the account that funded it.

**Not in R1–R9; each needs a decision** (§12). The owner has answered three:
timing ("1-2 s should be good ux"), fees and user authentication. The others:
the meaning of the size bound; carriers; privacy tiers and bystander
confidentiality; the default on forced sync; holding, count and since-sync
caps; one issuer or several; a lost-or-stolen report; Play Integrity; the
recovery default; relay of undelivered payments; other device classes; the
design outage; the limit day. §12 also asks the owner to confirm that the
later statements override the 2025 document wherever the two disagree, which
this document assumes. Two points of the old document have no decision item
here: clearing history from the phone after a sync, and location stamps on
records. A third is its version checks that force app updates and push
emergency rules. In this design no version floor acts offline: every release
accepts every rules version ever allowed (§5.11). Whether a minimum app build
is part of what a renewal under R8 checks is the owner's decision (§12).

**Set aside by the owner.** Smart cards: "right now smart cards are future
optionality to explore but there are no plans right now to use." The owner
did not mention wearables, which the 2025 document names beside cards. A
passive wearable is a card in another shape and is treated the same way here
(§11.1).
