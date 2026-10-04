# KAGEMUSHA — final implementation draft

Status: **canonical implementation target, 2026-10-03**. This replaces revision
6 and its decision menu. Implement and integrate this design for the POC and
production. The [verification checklist](kagemusha_evidence_gate.md) records
results; it does not authorize, prohibit or delay use or deployment.

This is a specification, not a claim that the implementation is complete.
The [source inventory](kagemusha_single_design_evidence.md) records what exists
and what changes. This document takes precedence over older KAGEMUSHA monetary
and phone-provider designs. Existing lower-level specifications describe their
current implementations until their contracts are replaced in the same patches.

## 1. Decisions and requirements

There is one protocol: a proof-carrying offline wallet, hardware-backed signing
keys, and a durable software state provider running in the released app on a
stock, uncompromised phone. The user explicitly selected this trust model for
the POC. Phones must run the vendor OS without rooting, jailbreak or a rootkit;
bootloaders must remain locked where the platform exposes that condition.
The same protocol is the production integration target. A stronger hardware
state provider may be researched later; it is not a prerequisite or a second
payment protocol.

There is no per-payment issuer call, Reserve/Commit service approval, deferred
settlement, signature-only payment alternative, or release sign-off gate.
Signatures, proof validity, replay protection and durable state commits are
ordinary runtime requirements. A failed check rejects that operation; a missing
implementation returns an explicit error rather than reporting a payment complete.

### 1.1 Required behavior

| ID | Requirement | Design rule |
|---|---|---|
| R1 | Offline payments | After enrollment and loading, peers need only each other (§5). |
| R2 | Mainstream phones | Stock Android/vendor equivalents and iPhone, hardware-backed keys, no custom applet (§2). Actual platform evidence and measurements are recorded separately. |
| R3 | Load from the ledger | A finalized reserve debit creates one wallet-bound load voucher (§6). |
| R4 | Final device-to-device value, unbounded hops | Send irreversibly transfers value to the bound receiver; Receive makes it immediately onward-spendable. Only exact Payment replay can finish delivery; no refund or hop ceiling (§§3–5). |
| R5 | Optional return online | Unload is the holder's choice. Remaining offline has no deadline unless an enabled regulatory control supplies one (§§6–7). |
| R6 | Account blacklist | A wallet holding an enabled authenticated list refuses to send to listed accounts (§7). |
| R7 | Optional daily/monthly limits | Disabled by default; enforced through signed quotas and persistent counters when enabled (§7). |
| R8 | Optional attestation expiry | Disabled by default; an enabled lease can require renewal before future sending (§7). |
| R9 | Compact payment | Canonical binary Payment is at most 10,000 bytes, independent of history (§8). |
| R10 | Stock OS in the proof | Every transition verifies the issuer-signed credential recording the platform evidence verified at enrollment or the last renewal (§§2.2, 3.2). Android: hardware key, locked bootloader, verified vendor boot, patch levels and app signing identity, as the attestation records them. iPhone: an App Attest key of this app on genuine Apple hardware, bound to the payment key; no boot, patch or jailbreak field exists. This proves recorded evidence, not live absence of compromise. |

For a payment from A to B, the following define completion:

- **P1:** B durably owns the value and can spend it onward offline.
- **P2:** A cannot spend the same value again under the stated trust assumptions.
- **P3:** B needs no later reconciliation, approval or settlement.
- **P4:** Later discovery of misconduct by A cannot invalidate B's accepted value.
- **P5:** Only explicitly enabled regulatory controls require an otherwise
  functioning offline wallet to reconnect.
- **PC:** All work needed for those properties finishes before completion is shown.

A committed Send permanently removes `amount + fee` from the payer. There is
no refund, cancellation, timeout reversal or retargeting after that commit.
The payer can only present the exact same Payment bytes again to the bound
receiver. Delivery evidence never authorizes restoring the payer's balance.

The receiving wallet reports **complete** only after its own proof, state,
commit receipt and recovery data are durable. At Send commit the payer reports
**sent — irreversible, delivery unconfirmed**; valid `Credited` evidence changes
that to **received**. This distinction reports delivery, not provisional value.
A lost receipt changes neither party's monetary rights.

Defaults, for the POC and production, are: blacklist enforcement off, spending
limits off, attestation
expiry off and transfer fees zero. An operator can enable the defined controls
through signed scheme policy. Integration needs no additional design decision.

### 1.2 Scope of durability

Normal restart, process death, interrupted I/O, power loss and supported app/OS
updates preserve the committed state and its private recovery data. Restoring
older app files must never restore spending authority. Temporary locked or
unavailable storage means **retry**, never wallet deletion or a zero balance.

A phone is the custody device. Destruction of its key or all current private
recovery data is permanent custody loss; a commitment cannot recreate missing
bytes. Factory reset, destructive uninstall/clear-data paths and loss of the
phone are not balance recovery mechanisms. Do not restore an older checkpoint,
automatically mint compensation, or promise survival of deliberate erasure.
Show destructive custody consequences before wallet-controlled reset actions.

## 2. Trust, enrollment and keys

### 2.1 Explicit assumptions

The safety argument assumes: correct released wallet code; a stock vendor OS
that enforces app isolation, key access and the storage contract in §4; a
hardware-backed non-exportable signing key; sound cryptography; correctly
operating issuer keys and ledger consensus; and retention of the custody data
in §1.2. An ordinary user may interrupt operations, restore app backups, change
the wall clock or replay messages. Those actions must not create another spend.
A compromised OS or modified wallet executable is outside this selected model.

A proof establishes conserved, authorized lineage. It cannot establish the
absence of a rootkit, or discover another valid fork by itself. The state
provider prevents forks under the assumptions above. Insurance or later fraud
detection is not part of that argument.

### 2.2 Enrollment

Enrollment is online once per wallet incarnation. Bind a fresh issuer challenge
to the canonical domainless `AccountId`, network, scheme, asset incarnation,
wallet key, app identity and enrollment policy. A new incarnation always begins
at zero; reenrollment never imports an old balance or resets its replay state.

The issuer verifies platform evidence and issues a compact signed credential:

`(scheme, asset_incarnation, wallet_id, account_id, payment_key,
provider_contract, platform_evidence_kind, evidence_digest, app_policy,
regulatory_policy,
enrollment_id)`.

`wallet_id` binds the public key and unique incarnation. `provider_contract`
names the §4 Advance contract and receipt format of the scheme. It is fixed for
the scheme's lifetime, so app and adapter updates never require reenrollment.
The credential conveys exactly the evidence verified, including its time and
platform limitations. Its issuer signature and policy binding are consumed by
the proof relation. Raw attestation chains stay in enrollment records, outside
peer messages.

- **Android and compatible vendor APIs:** validate the attestation chain
  to a pinned Google root or adapter-named vendor root, with no certificate on
  the attestation revocation status list at the time of the check (a leaked key
  passes until Google lists it; the list is not instantaneous), and the
  challenge, hardware security level, generated key properties, expected app
  signing identity, locked device, verified vendor boot and enrollment patch
  policy. Reject software keys, unlocked/custom boot evidence and identity
  mismatches. Keep TEE and StrongBox evidence distinct. These fields describe
  the attested event, not a live runtime examination of every payment.
  See the [AOSP attestation contract](https://source.android.com/docs/security/features/keystore/attestation).
- **iPhone:** create the payment key in the Secure Enclave. Verify App Attest
  and bind its fresh assertion transcript to that payment public key and the
  enrollment challenge. Reject when App Attest is unsupported, the attestation
  does not chain to Apple's App Attestation root, the App ID differs, the
  environment is development for a production scheme, or the nonce, key
  identifier or counter does not match. The binding authenticates the app's
  supplied key; it is not independent Apple attestation of that separate key or
  a measurement proving the running OS has no exploit. Stock iOS and correct
  local key creation remain assumptions. See [Apple's validation
  API](https://developer.apple.com/documentation/devicecheck/validating-apps-that-connect-to-your-server).
- **Other vendor ecosystems:** use an adapter with an explicit equivalent
  evidence contract. A brand name is not an attestation format. Do not invent
  Android fields for a platform that lacks them or silently enroll a software
  key. Such adapter work can proceed alongside the POC.

Play Integrity, where used, is an enrollment-time signal recorded in
`evidence_digest`. Delete the periodic Play Integrity refresh lease and its
Guard slot; periodic renewal exists only as the optional §7 attestation lease.
Local root/jailbreak checks supplement these checks; they are not a proof that
compromise is absent. A positive local check rejects enrollment. After
enrollment, run the checks before issuing each Request and before Send; a
positive result rejects that operation and says why. It never deletes state,
changes the balance or alters committed payments. Preserve custody, delivery
bytes and recovery access; a detected compromise is outside the security
assumption, not authority to reverse a Send.

Renewal, where the attestation lease is enabled, is not a repeat of enrollment:
platforms attest a key only when it is generated. A renewal carries three
things. (1) Possession: the payment key signs the issuer's fresh renewal
challenge. (2) Fresh platform evidence where the platform offers it: on
Android, a newly generated attested key whose attestation challenge is the
renewal challenge, whose public key the payment key also signs, and whose
attestation records the current boot state and patch levels; on iPhone, an App
Attest assertion by the enrolled App Attest key over the challenge (an App
Attest key cannot be attested twice, and nothing about the OS is attested).
(3) The enrollment evidence, identified as historical facts about the payment
key's generation. The issuer applies the scheme's current enrollment policy to
the fresh evidence and records in the renewed credential which facts are fresh
and which are historical. Without expiry enabled, enrollment does not create a
periodic reattestation or app-store-update requirement. Normal vendor OS updates
retain the custody contract; record regressions and repair them, without
silently redefining old completed payments as provisional.

### 2.3 Key roles and user authentication

Separate the scheme root, enrollment signer, finalized-load authorization,
regulatory-policy signer and artifact signer by role and domain. Ledger finality
must authenticate every load; an enrollment or artifact signature cannot mint.
A wallet key signs provider receipts, Request quotes, peer session
and ledger-control authentication, and the §2.2 renewal challenge and
renewal-key binding, each in a separate domain. These signatures confer only
the authority of their named operation. All verifiers enforce the role,
scheme and network.

PIN/biometric authorization is local UX, verified by the phone's secure
hardware: Send and Unload confirmation uses the platform's hardware-backed user
authentication (Android BiometricPrompt with strong biometric or device
credential; iPhone LocalAuthentication backed by the Secure Enclave). Do not
bind the payment key itself to user authentication: on Android removing the
screen lock invalidates such keys, which would destroy the balance. If a
confirmation key bound to user authentication is used, it is a separate key
that can be regenerated without affecting custody. Never use an app-only PIN
screen. Keep payment
identity stable across biometric enrollment changes. Do not select a key policy
that silently destroys money when a fingerprint changes. Where a storage class
requires a device passcode, explain that removing the passcode may destroy its
custody material; test that lifecycle on each adapter. Apple's storage classes
have distinct [backup and passcode behavior](https://support.apple.com/guide/security/keychain-data-protection-secb0694df1a/web).

## 3. One state and one proof relation

The existing Rust state and recursive-proof owners remain canonical (§9).
Swift and Kotlin own platform I/O, not independent balance algorithms.
All amounts are checked `u128` integer asset units; zero-value Send is rejected.
Sequences are checked `u128`, never wrapping or resetting within an incarnation.

Each `wallet_id` is enrolled for one scheme and asset incarnation (§2.2) and
owns one private state:

```text
lifecycle: Active | Retiring
balance
sequence, next_send, next_load, next_redeem
consumed_credit_root
pending_outgoing_root, load/redeem and fee-claim recovery roots
regulatory_policy, quota counters, accepted time/epoch information
state nonce, state commitment
```

Authenticated local maps hold the openings and retained objects behind their
roots. The root is not a backup of the map. The consumed-credit map permanently
binds each received `credit_id` to the digest of its full canonical Payment.
It is never pruned or reset within an incarnation. There is no exclusive
receive slot; multiple incoming and committed outgoing payments are allowed.
An absent recipient must not freeze all other spending. The same key cannot
pay itself; different wallets of the same account can transfer value and
remain subject to their quotas.

Map leaves contain stable credit/operation identifiers and effect descriptors,
not the current proof, receipt or complete output package. Exact output bytes
live in an authenticated auxiliary archive. This keeps the successor state
commitment independent of the proof and receipt that will certify it.

### 3.1 State packages

A public package is `(statement, π, τ)`. The statement binds scheme, relation,
wallet credential, asset, operation, sequence, predecessor and successor state
commitments, and the operation's public effect. Sensitive openings and private
history are witnesses. `π` is the recursive transition proof. `τ` is the
provider's durable commit receipt bound to the exact statement and proof digest.

To make a transition:

1. Verify the complete predecessor package and required input packages.
2. Construct the next state and prove its transition, producing `π_next`.
3. Call `Advance(expected_head, new_head, hash(π_next), operation_id,
   recovery_capsule)` and obtain `τ_next` (§4).
4. Release the new complete package only after durable completion.

The current receipt is verified natively whenever a package is consumed. Its
successor proof verifies that same receipt inside the recursive relation.
Every incoming package is also fully verified, including its receipt. Thus an
ancestor's authorization cannot disappear inside a later proof. A proof alone
is not a transferable credit. This ordering avoids requiring the current proof
to contain a signature over its own digest.

### 3.2 Transition relation

Use the existing paired-Pasta recursive construction and retained P-256 gadgets;
replace obsolete online Guard bindings. Pin all hash domains, curve encodings,
transcript parameters and verifier material in one authenticated artifact set.
Native and in-circuit encodings must have shared vectors. No server receives
wallet private state or acts as its monetary prover; proving runs in the native
wallet core. Enrollment evidence verification is an issuer task.

Every transition enforces:

- Correct credential, scope, relation identity, operation tag and provider
  contract; the new state keeps its identity and advances its sequence once.
- A valid predecessor proof **and receipt**, or the unique zero-state enrollment
  base case. Every recursively consumed incoming package includes its receipt.
- Checked balance arithmetic, exact map membership/nonmembership updates,
  unchanged unrelated fields, and exact operation effects from §§5–7.
- Request/credit identity, one-time ordinals, counterparty and exact amount
  binding; positive payments; distinct payer and receiver wallet keys.
- Applicable signed policy, historical fee terms and quota/time transitions.
  A Send permanently consumes its gross sending quota. Delivery, replay and
  outbox cleanup cannot increase the payer's balance or restore that quota.
- Commit receipt bindings to the statement, proof digest, operation identifier,
  sequence and predecessor/successor. No substituted app approval or arbitrary
  hardware signature satisfies that interface.

The zero-state proof binds its unique enrolled incarnation and zero balance,
empty maps and zero ordinals. The provider installs this base state once, by
Advance from an enrollment marker made durable with the payment key before the
credential request. From then until custody is deleted, the key is always
covered by one durable current marker, or by a terminal marker once the
incarnation's custody is deliberately deleted or its unused enrollment abandoned.
Older generations are retired as §4.2 says; coverage is continuous, not
preservation of every generation. A key or
credential without its uninstalled enrollment marker is a used incarnation and
is never initialized again.
The ledger enables load issuance only after verifying and recording the complete
Bootstrap package for that incarnation. A credential alone cannot receive a
load voucher. This activation is idempotent and occurs during enrollment.
An interrupted enrollment resumes the same installation; it never creates two
initialized heads. Abandonment is allowed only while the enrollment marker is
still selected and Bootstrap has never committed. It commits a terminal marker
and records its receipt in a ledger instruction that atomically rejects prior
activation and permanently disables activation and loads. No voucher or monetary
balance exists to return. Once Bootstrap commits, including an uncertain
activation response, recover that incarnation and use §6.3; do not abandon funded
obligations. Releasing an unused quota allocation requires that terminal evidence
and does not erase historical quota usage.

### 3.3 Stable offline verification

Freeze one relation and verification interface for the lifetime of a scheme.
Existing offline wallets verify future payments in that scheme using the same
material. Implementation optimizations preserve that interface and semantics.
A breaking relation is a different scheme. Moving value to it uses a voluntary
unload and new load; there is no cross-scheme offline decoder or conversion.
Existing wallets can keep their original scheme without reconnecting.

Certificates for planned signing-key rotation are signed directly by the
existing scheme root and travel within authenticated packages or peer policy
data. Delegation depth is fixed, independent of rotation count. Historical
loads remain valid. New issuance can stop after issuer compromise; old completed
value is not retroactively tainted. An emergency online reconnection rule is
not implied by key rotation: it exists only if a previously enabled regulatory
control authorizes it. No unavailable online revocation lookup is hidden in
native or recursive peer verification.

## 4. Durable state provider

### 4.1 Contract

`Advance` is a local operation, serialized per wallet. It either leaves the
old head usable without releasing a new receipt, or durably selects the exact
new head and retains everything needed to finish and return its package.
Only one successor of a given head may be released. Competing precomputed
proofs do not debit value; only the winning commit can become a package.

The signed receipt binds the scheme, wallet, provider-contract identity,
sequence, operation ID, old/new commitments, statement digest, proof digest
and recovery capsule digest. Before first release, persist the assembled canonical output, including
its proof and receipt, in redundant authenticated local completion records.
Retries return those exact bytes; they never create a new Payment, proof or
receipt for a released result. If every copy of released Payment bytes is lost,
report delivery-data loss; neither a regenerated package nor a restored payer
balance is recovery. Reusing an operation ID with changed inputs fails
only after the ID has a selected head; a discarded staging attempt frees it.
A crash after head selection but before signing finishes resumes that same
operation offline.
When a write error leaves the outcome unknown, report the operation pending
until reconciliation. Show a resumed operation's released result, and show an
operation whose staging was discarded as not performed.

Freeze the private recovery capsule body before signing: actual next state
openings, proof, required map nodes, retained input bytes, output descriptors
and operation recovery data. The receipt signs this immutable body's digest.
Append the receipt and assembled output bytes in a separate durable completion
record excluded from that digest. Neither the body nor a monetary map leaf
contains its own final receipt/package. Persist both records before returning;
a hash or a key-store marker alone is insufficient for onward spending.

### 4.2 Stock-phone adapter

Implement the contract with the released native core, a durable journal, a
non-backup platform marker and the credential's hardware-backed `payment_key`,
the wallet key that alone signs receipts. The OS is trusted
to enforce storage and key access. A reusable hardware key does not itself
implement compare-and-swap or make the marker safe against a compromised OS.

The adapter performs this recoverable sequence:

1. On startup and before every operation, reconcile all marker generations and
   journal records. Finish any interrupted commit and retirement of **all**
   older markers before permitting another receipt. Do not infer absence from
   a read error or locked storage.
2. Return the retained result for an operation ID that already has a selected
   head. Otherwise check the expected head and reserve the storage of §5.3.
   Persist the complete staged
   capsule with authenticated links to the predecessor and operation ID. Flush
   its data and directory metadata using the platform's durability contract.
3. Install the new marker, binding that capsule and its predecessor. Make it
   durable before retiring old markers. On restart, a complete successor marker
   selects that successor; a staging record without its marker, complete or
   not, selects nothing and is discarded before any other operation; the head
   bound by the current marker stays selected. Never select an arbitrary older
   valid-looking marker.
   A payment key or journal found with no marker at all is lost custody: never
   install a base state or select a journal head under it. Enrollment writes
   its first marker before requesting the credential, and a new incarnation
   generates a new key.
4. Durably retire older markers, finish the selected operation, sign its receipt
   and retain the exact released package. Only then return or display/send it.

A complete marker with missing or mismatched capsule means unavailable custody
data: recover its authenticated local copy or stop the operation. Never fall
back to the old balance. No backup, restore or
device-transfer path may carry keys or markers. Restored app files are compared
with the surviving current marker. Store redundant local
recovery copies for interrupted writes; do not claim they survive total erasure.
Keep current capsules, completion records and these copies outside backup and
device-transfer sets, where an app-data restore cannot replace them. If a
platform restore still removes them, that is custody loss under §1.2 and the
wallet shows it as such; it never waits silently for missing bytes.

Android and Apple storage adapters must implement these semantics with their
actual APIs and record durability assumptions. This document does not assert
that a successful API return has already demonstrated power-loss durability.
That is implementation and verification work, not an administrative prerequisite
to use or integration. A failed durable commit cannot truthfully return success.

## 5. Offline exchange and recovery

### 5.1 Messages

Every message binds the scheme, asset, wallet identities and canonical format.
`credit_id = H("credit", canonical Request fields)` uses a distinct hash domain.
Request fields bind both wallet IDs, payer send ordinal `s`, receiver credential,
amount, signed fee schedule and exact fee, regulatory policy references, the
receiver's authenticated accepted time (§7) and a fresh 256-bit nonce. The
receiver signs these fields in the Request domain. No free-form field changes
the monetary interpretation.

| Step | Message and state effect |
|---|---|
| Offer | Payer advertises its next `s`, amount and supported scheme. It is an authenticated session hint, with no debit or credit authority. |
| Request | Receiver signs a nonmonetary setup quote containing the exact fields above. It creates no state transition, receiver ordinal or reserved receive slot. Send verifies its signature and credential. |
| Payment | Payer verifies Request and applicable Send policy, checks `s == next_send`, proves Send, permanently subtracts `amount + fee`, increments `next_send` and inserts the credit descriptor in its pending outbox. Commit and retain the full canonical Payment before first release. Payment includes the complete Send package, Request and all verification dependencies. |
| Receive | Receiver verifies the full Payment, its own bound identity and consumed-credit nonmembership. It proves Receive, adds exactly `amount` and inserts `credit_id` with the full canonical Payment digest into the permanent consumed-credit map in the same commit. Show complete only after durable completion. |
| Credited | Optional delivery evidence: the complete Receive package or a read-only `CreditStatus` proof against the receiver's current complete package. The latter proves consumed-credit membership bound to the exact Payment digest, recipient and scope. It advances no state and need not be retained. |
| ArchiveSent | Payer verifies matching Credited evidence and proves removal of the matching pending outgoing descriptor. It may then delete the delivered Payment bytes, except copies still needed for a fee claim (§6.2). Balance, quotas and consumed-credit entries stay unchanged. |

A Request has no cancellation or expiry that can invalidate an already
committed Payment. Its validity for Send is scoped to the payer's still-unused
`s`; consuming that ordinal prevents another Send under the same quote.
Quote creation authenticates setup, not receiver spending authority. The Send
relation verifies the quote's receiver signature and credential directly.

Receive and CreditStatus bind the digest of the **entire canonical Payment**,
including its proof and provider receipt. Conflicting bytes for a consumed
credit are rejected. A proof of absence or a generic signed acknowledgement is
not evidence of credit. CreditStatus verifies the current package and its
receipt as well as the membership opening; its proof cannot invent an accepted
credit. ArchiveSent verifies all that evidence in its transition relation.
No receiver acknowledgement archive or subsequent pruning exchange is needed.
The permanent replay map permits delivery evidence to be regenerated after
unrelated sends, receives and cleanup.

### 5.2 Retry and race rules

- A Send becomes irreversible at its durable commit. Cancellation before that
  commit changes no balance. An uncertain commit is resolved from the selected
  local head and journal; a transport error never implies that Send did not occur.
- Every delivery retry presents the exact committed Payment bytes to the same
  receiver. Framing and carrier may change; the canonical transaction bytes may
  not. No new Send or proof is generated for a delivery retry.
- The first valid Receive credits once. A duplicate with identical bytes returns
  optional Credited evidence without another credit. Permanent consumed-credit
  membership prevents replay after cleanup, restart or unrelated transactions.
- A stale Offer or an unacceptable quote detected before Send can start a new
  setup without debit. After Send, a newer Request, policy, fee schedule or
  retirement state cannot cancel the recipient's existing claim. Verify the
  historical policy and fee terms proved by Send, not a new quote's terms.
- The receiver may be absent or temporarily unable to store or process Payment.
  Declining a screen or disconnecting does not produce a monetary refusal.
  Preserve the payer's outbox for later delivery. No timeout or negative reply
  permits refund, retargeting or any restoration of the debit.
- Receive does not depend on a still-open Request or the receiver's old head.
  Delayed valid Payments remain receivable after other payments. Malformed,
  unauthenticated or wrong-recipient objects are rejected without mutation;
  rejection is not a value-return authorization.
- Lost Credited evidence only leaves delivery unconfirmed at the payer.
  The receiver can spend onward immediately. It can later prove the original
  credit through its permanent replay map, even after spending that value.
- If every retained copy of a not-yet-delivered Payment is destroyed, delivery
  cannot be reconstructed from a digest. The in-flight value is stranded under
  §1.2; it is never returned to the payer. Explicit regulatory controls can
  restrict use only as §7 defines and cannot reverse Send.

### 5.3 Storage and user experience

Keep exact unresolved outgoing Payment bytes until verified Credited evidence
allows ArchiveSent. Keep consumed-credit IDs, Payment digests, private map
openings and recovery data for the lifetime of the wallet incarnation. Do not
prune these by age, peer absence or an arbitrary history limit. Local storage
grows with received credits and unresolved operations; constant-size proofs do
not imply constant total wallet storage.

Before Send or Receive, reserve actual disk capacity for the full
commit/recovery/output path. Include replay-map growth, redundant output copies,
fee claims and eventual outbox cleanup. A Request alone promises no reserved
capacity. Temporary capacity pressure postpones Receive while the immutable
Payment remains deliverable; it does not cancel that credit. Do not strand an
already accepted balance because its ancestry is long. Derive proving workspace
bounds from the fixed relation and publish measured memory and per-operation
storage budgets in artifact metadata. Existing measurements do not yet provide
those budgets for this design.

Show amount, recipient, fee and irreversibility before payer confirmation.
After Send commit show **sent — irreversible, delivery unconfirmed**, then
**received** if valid delivery evidence arrives. Reconnecting the same phones
resumes delivery of the existing Payment; it does not create another payment.

## 6. Ledger boundary, fees and wallet replacement

### 6.1 Load and unload

After completed Bootstrap activation (§3.2), a finalized online transaction
debits the payer's ledger account into the scheme reserve and creates a unique
load voucher bound to `(wallet_id, next_load, asset, amount)`. The ledger assigns
successive ordinals per incarnation. `Load` consumes exactly the next voucher,
verifies its finalized issuance, adds its amount and increments `next_load`.
Out-of-order vouchers wait; duplicates cannot load twice. Retrieving the
original voucher after a connection failure is idempotent. Unabsorbed vouchers
remain reserve liabilities; failed delivery does not refund their issuance.
Retirement closes future loads atomically (§6.3).

`Unload` subtracts a chosen positive amount, increments `next_redeem` and creates
a ledger-directed claim with a domain-separated nullifier derived from scheme,
wallet and redemption ordinal. The ledger verifies the complete committed
package and pays its bound account exactly once at face value. Retry returns
the original result. No timeout restores an uncertain redemption to the wallet.
Retain the claim until finalized payout. The remaining wallet balance continues
to work offline. Unload spends the holder's current balance; it cannot reclaim
an earlier Send.

Reserve accounting conserves the sum of wallet balances, unabsorbed loads,
in-flight recipient credits, earned but unpaid fees and unpaid redemption
claims. Send moves `amount` to an in-flight recipient credit and `fee` to an
earned fee; Receive moves only that amount into the receiver's balance. Each
transition moves value between these categories; it does not create it.
Distinguish proven accounting from a monetary backstop. No first-redeemer
preference, pro-rata haircut or later payer-misconduct finding changes an
honest recipient's claim. Ledger unavailability delays a requested unload but
is not an offline settlement dependency.

### 6.2 Optional fees

Fees are zero by default. A signed immutable schedule fixes the fee calculation,
rounding rule and online beneficiary. Bind the schedule and exact fee into
Request and Send; Receive verifies those historical terms. Send permanently
debits `amount + fee`, and the fee is earned at that commit even if delivery is
delayed or never finishes. Receive gives the recipient `amount`.

The complete committed Payment authorizes one fee payout keyed by `credit_id`,
only to the fixed beneficiary. Anyone may relay it online, including before
Receive; the ledger verifies the Send proof, receipt and fee terms and rejects
duplicate claims. Later delivery evidence and schedule changes cannot alter or
cancel the entitlement. Use checked integer arithmetic and explicit rounding.
For a nonzero fee, the payer retains a separate fee-claim object containing the
Payment until finalized payout. ArchiveSent cannot delete the last copy needed
by that claim. It does not block spending or require the holder to reconnect;
another party may relay it, or the claim remains unpaid in the reserve.

Load/unload fees are also possible under a displayed signed quote: the exact
net offline value and online charge must be separate fields. They never deduct
an undisclosed fee from a previously accepted offline balance.

### 6.3 Replacement and custody loss

Move value to another enrolled phone using an ordinary offline payment,
including full-balance transfer. This uses the same proof and message format.
Keep the old wallet's key, private state, replay map and unresolved Payment and
claim bytes. Zero spendable balance does not mean its custody data is disposable.

Retirement closes new setup and funding, while preserving existing claims:

1. Commit a proven `Retiring` transition. It issues no new Request quotes, but
   continues to Receive valid Payments under previously signed quotes, including
   Sends that commit later. It may Load vouchers already issued, Send or Unload
   remaining value and finish delivery, fee and redemption claims. The lifecycle
   never reverts to Active.
2. Submit a ledger-control instruction carrying the complete Retiring package
   or a later complete package proving that lifecycle and its `next_load`.
   In one transaction, the ledger checks that no voucher at or above that ordinal
   exists and permanently disables further loads. If a voucher exists, the
   instruction fails and the wallet loads it first. A load submitted after
   closure is refused and debits nothing. Repeating closure is idempotent;
   later activation attempts cannot reopen it.
3. Keep the retiring incarnation's receiving custody available for late
   Payments. An empty balance, empty outbox or a delivery receipt cannot prove
   the absence of unseen incoming Payments. An old signed quote cannot be
   revoked to invalidate a Payment. Normal retirement therefore does not delete
   that key, replay map or recovery data, even after known obligations finish.

The wallet can remain Retiring offline indefinitely. Deliberate permanent
custody deletion writes a terminal marker before deleting key material and
warns that late incoming Payments and any retained claims will be lost. It is
a destructive custody action under §1.2, not a lossless monetary drain or a
way to refund the payer. Unused enrollment abandonment has the separate
unactivated/no-voucher condition in §3.2.

There is no seed restore that recreates spent offline value on another phone
or automatic issuer reissue. The issuer cannot reclaim or reissue a committed
Payment on timeout or reported delivery failure. Replacement does not renew
regulatory quotas.

## 7. Explicit regulatory controls

Only the following signed controls may restrict future use. The credential
binds the scheme's permitted controls and their activation semantics. A wallet
can accept a newer authenticated policy via a peer; learning it does not require
an online call unless an enabled rule below explicitly requires freshness.
Policies cannot retroactively undo completed credit or impose an unannounced
new connectivity control on an existing credential.

`RefreshPolicy` is a proven, locally committed transition for authenticated
policy/list updates, time reanchoring and lease renewal. It preserves wallet
identity, balance, ordinals, consumed-credit entries and pending obligations;
advances policy/list epochs
monotonically; and cannot reset consumed quota or backdate accepted time. Any
replacement credential binds the same incarnation. Verify the appropriate
policy/enrollment signer and the existing credential's permitted controls.
Obtaining a renewal online is necessary only for an enabled control that needs
it. Cached or peer-carried authenticated updates use this same transition for
policy and list changes; they never reanchor time (below).

| Control | Behavior when enabled | Connectivity consequence |
|---|---|---|
| Recipient blacklist | Check the recipient's canonical account against the latest authenticated list held by the sender. Persist monotone list versions. A listed recipient is refused before Send. | None from list age alone. A separately explicit maximum-list-age rule can require refresh. |
| Daily/monthly sending quotas | Debit gross `amount + fee` permanently against both configured windows at Send. Delivery and cleanup do not replenish quota. Issuer allocates wallet shares so their sum cannot exceed the account allowance in any window. | No routine sync while the signed quota and trusted time remain usable; loss of the time anchor may require renewal. |
| Attestation lease | Require renewal before a future Send after the signed expiry. Accepted balance and already committed exchanges remain owned and recoverable. | Renewal is the configured online requirement. |

Blacklisting a former payer does not taint downstream value. No ancestor lookup
can become an implicit blacklist. A restriction on the current holder must be
an explicit control on that holder, never a reassignment or confiscation of the
accepted amount by the payment protocol.

Time-dependent controls use an issuer-signed time anchor and OS monotonic
elapsed time, never the editable wall clock. An anchor is created only by a
direct issuer exchange in the current boot: the wallet sends a fresh nonce
generated in this boot, the issuer's signed response covers that nonce and
its own time, and the wallet accepts it only if it arrives within a fixed
response-age bound of the request on the monotonic clock. The anchor records
the boot identity, the issuer's signed time `T`, and the monotonic readings
when the request was sent (`m_req`) and when the response arrived (`m_rcv`).
Real time at a later reading `m` in the same boot lies between
`T + (m - m_rcv)` and `T + (m - m_req)`; the width `m_rcv - m_req` is the
anchor's uncertainty and never exceeds the response-age bound. Checks are
conservative: an expiry, lease end or other deadline counts as passed once the
upper end has passed, and a window start or not-before counts as reached only
once the lower end has. A Send whose interval touches two day or month windows
is counted in every window it touches. The monotonic clock must keep running
while the device sleeps (Android `elapsedRealtime`, Apple
`mach_continuous_time`); a clock that stops in sleep is not used. Elapsed time
runs only within the anchor's boot. A signed time that arrives in any
other way (cached, in a policy update, or carried by a peer) can raise the
accepted time floor but never creates an anchor or restarts elapsed time. Use
one effective accepted time for all applicable windows and checks: at least
the prior accepted time, anchored local time and authenticated Request time. A
reboot that loses the elapsed-time basis needs reanchoring only while such a
control is enabled; until then that control refuses Send, because the accepted
time floor alone can lag real time. With controls off, no clock condition
blocks payment.

Policy renewal, reenrollment and device replacement preserve consumed allowance.
While an old wallet can still spend under its allocation, its share stays
reserved; another device cannot be issued the same share. Quotas bind explicit
windows and expiry, and unused expired shares are not retrospectively spendable.
Additional per-counterparty caps, receive-age rules and arbitrary version-based
sync requirements are outside this design.

## 8. Wire format, carriers and performance

Use one canonical Norito V1 envelope and explicit layout flags. Unknown mandatory
fields, noncanonical encodings, overflow and mismatched scheme/relation IDs are
rejected before mutation. Do not guess codecs or accept a retired format as a
fallback. Bind hashes and signatures to canonical bytes with length-delimited,
role-separated transcripts. Protocol P-256 signatures use fixed 64-byte
big-endian `r || s`, with `1 <= r < n` and `1 <= s <= floor(n/2)` for the
P-256 group order `n`. Normalize signing output before freezing the canonical
object; native and in-circuit verifiers reject high-S or alternate encodings
rather than rewriting received bytes. Raw platform attestation records remain
unchanged in enrollment evidence. The Send statement binds the exact signed
Request and verification dependencies carried in Payment by digest; its receipt
binds that statement and proof. No unauthenticated extension can change the
canonical Payment digest while preserving its authorization.
The exact G1 field layouts, transcripts and bounds are recorded in the
[wallet wire record](kagemusha_wallet_wire_v1.md).

Text transport is `kgm1:` plus unpadded base64url; text/framing expansion is
additional carrier overhead, not hidden in the binary budget.

| Object | Maximum canonical binary bytes |
|---|---:|
| Offer and simple session-control frame | 2,048 |
| Request, Payment or Credited (including a Receive package or CreditStatus proof and its current package), with required dependencies | 10,000 each |

Proofs summarize lineage; do not ship a certificate or transaction chain whose
size grows with hops. Payment includes the signed Request and its credential;
its proof verifies that signature and binds those exact fields. Credited binds
the full Payment digest. ArchiveSent consumes that evidence locally and creates
no additional peer-message round trip.
An unknown scheme can be declined during Offer. Malformed unauthenticated
traffic is dropped. A setup error before Send carries no monetary authority;
a decode, version, policy or capacity error after Send cannot reverse the debit.
Valid committed Payments retain their recipient binding and historical terms.
Each package contains the certificates and authenticated terms its verifier
needs beyond the preinstalled scheme roots and fixed artifact set. A missing
dependency cannot trigger an implicit online fetch during payment.

Keep NFC, supported local radio transports, QR and Petal Stream as carriers of
the same envelope. Carrier negotiation selects transport, never monetary rules.
Petal is an opaque optical carrier; its documented per-payload size must be
handled by framing/reassembly when a whole message is larger. Do not carry over
QR frame-rate arithmetic as a measured result for Swift or Petal. Each carrier
reassembles and validates a complete bounded message before monetary parsing.

The UX target is **2 seconds p95** from payer confirmation after Request to
receiver durable, onward-ready completion. It includes payer proof, transfer,
receiver proof and both durable commits. Measure Offer/Request setup, cold
startup and payer receipt-confirmation latency separately. Record failures and
slow trials as well as successes. The 10,000-byte bound is a format requirement;
2 seconds is an optimization target. Neither is a claim about today's code.
Work on integration and optimization can proceed together; missing a latency
target does not authorize skipping proof or durability work.

## 9. Implementation ownership and retirement

Implement in the existing owners. The detailed capability inventory and deletion
checks are in [the evidence appendix](kagemusha_single_design_evidence.md).

| Owner | Work to retain and change |
|---|---|
| `crates/iroha_core_zk/src/kagemusha_v1_state/` | Single state machine, pending maps, transition validation and recovery projections. Replace old operation ordering with proof then Advance. |
| `crates/iroha_core_zk/src/kagemusha_v1_recursion/` | One recursive relation/artifact set; retain cryptographic gadgets and replace per-operation online and hardware-only authority assumptions. |
| `crates/iroha_crypto/src/kagemusha.rs` | Retain encryption and recovery primitives; update caller contracts and domain bindings. |
| `crates/connect_norito_bridge/src/kagemusha_core_coordinator_v1/` | Opaque state/proof handles, platform dispatch and durable retry coordination. Remove per-payment service phases. |
| Swift; Kotlin `core-jvm`, `client-android`, `kagemusha-wallet-android` | Thin shared-core clients; platform evidence/key/storage adapters and carriers remain in their appropriate modules. Kotlin owns JVM behavior; preserve Java consumer assertions. |
| `iroha_data_model`, `iroha_core`, `iroha_torii`, `iroha_config` | One model and service family for enrollment, load, unload and policy; reserve/finality/replay enforcement; configuration through user → actual → defaults. |
| Formal models, fixtures and package tools | Update the selected trust boundary, messages and crash transitions; preserve useful assertions and regenerate one canonical set of vectors. |

Remove per-payment ordinary Reserve/Commit/FI-control service requirements and
its duplicate wire/API family. Remove monetary Refuse/Refund, cancellable receive
Requests, acknowledgement/pruning chains and their obsolete recovery APIs from
the target state machine, proof relation, model, codecs and SDKs. Move useful attestation, storage and equation
code first. Remove the independent signature-only suite and empty
`iroha_kagemusha_attested` / `iroha_kagemusha_issuer` stubs after migrating
actual consumers. Preserve generic Petal/NFC/QR components. Artifact
authentication is the artifact signer's signature over the frozen artifact set:
remove the review, fuzz, reproducible-build and profile-qualification evidence
of `KagemushaInternalValidationReceiptV1`, threshold release approvals and the
Production/TestnetExperiment split from `KagemushaAuthenticatedReleaseV1`, its
governance install path and the compact-key and native-profile specs. Keep every
runtime binding that receipt carries: move `native_profile_digest`, the Eq and
Ep protocol digests, the relation identities and the artifact inventory digest
into the signed artifact manifest, and keep every verifier comparison against
them. Removing sign-offs must not remove a binding a verifier checks. Retire
unused secure-element wrappers and separate monetary-provider profiles; the
local Advance interface is the one adapter boundary.

Deletion is part of implementation, with no separate owner permission step.
For each replacement, identify all callers, move retained behavior/assertions,
connect the canonical owner, remove obsolete exports/layouts/fixtures and run
the affected checks in the same change. Keep no compatibility shim, second
production engine or decoder fallback. Do not delete by filename prefix: the
ordinary modules contain useful shared capabilities as well as retired protocol.
This draft does not establish that live issued state is absent; the review
found no evidence of a funded deployment outside testnets. Old-format value
under a TestnetExperiment release is testnet value; a testnet reset is its
cutover. Before an old release is removed from a network, establish whether
funded old-format value exists there. If it does, that release's verifier
stays until its holders have unloaded or paid it into the new scheme by their
own choice; nobody is required to go online for the cutover. Keep no decoder
fallback beyond that.

## 10. Goals and execution order

These are concrete implementation goals, not permission gates. Start the POC
integration now; work streams may overlap. Mark a goal complete when its stated
result exists and record observations in the checklist. Failed experiments
produce fixes or accurately stated limitations, not a release approval ceremony.

| ID | Goal and owner | Completion result |
|---|---|---|
| G0 | Consolidate design — specs | One target, explicit stock-OS assumption, informational checklist and current migration inventory; superseded authority clearly marked. **Completed by this draft.** |
| G1 | Define canonical objects — model/core | Norito state/message/credential/receipt fields, bounded envelopes, domain-separated transcripts and cross-language vectors encode §§3–8 exactly. |
| G2 | Implement Advance — native/platform adapters | Journal, marker cleanup and retained capsule implement §4; retry, backup restore and interruption cases exercise the real platform boundaries. |
| G3 | Complete proofs — core ZK | Real Bootstrap/Load/Send/Receive/ArchiveSent/Unload/RefreshPolicy/Retiring packages and read-only CreditStatus proofs verify every ancestor receipt, signed Request, irreversible debits, exact arithmetic, policies and permanent replay membership. |
| G4 | Integrate the phone exchange — bridge/Swift/Kotlin | A → B → C remains offline, including restart, interrupted delivery and exact-byte replay; Send never reverses and receiver completion already has its spendable proof. Record current size, latency and resource results. |
| G5 | Connect ledger and controls — node/Torii/config | Finalized load and exact-once unload/fee claims preserve reserve liabilities; optional controls default off and operate as §7 specifies. |
| G6 | Delete superseded implementations — component owners | Old payment authority, profiles, APIs, duplicate engines, stub crates and obsolete vectors removed with their consumers migrated; one packaged implementation remains. |
| G7 | Verify and maintain — component owners | Checklist records genuine proofs, device results, formal assumptions and known deviations on the candidate; repairs update code and vectors together. This work continues during use. |

Suggested first vertical slice: one asset, Android → Android, controls/fees off,
real enrollment and load, shared Rust transition/proof path, durable offline
receive and onward payment. Integrate iPhone storage/keys and alternate carriers
in parallel. Stand-in payloads are useful for carrier work but cannot satisfy a
completed-payment claim. Neither the POC nor production integration introduces
a flag that bypasses monetary validation.

## 11. Verification and present implementation truth

The checklist covers conservation, competing successors, receipt substitution,
permanent replay protection, irreversible Send, exact-byte redelivery, lost
delivery evidence, crashes, storage errors, clock rollback, stale policies,
genuine multi-hop proofs and physical carriers.
Update the finite-state model to match this exchange; a model's provider
assumption is not proof that a phone API implements it. Each result identifies
its code, artifacts, devices and trust assumptions.

At this rewrite's inspected working-tree baseline, substantial state, crypto,
attestation, recursive gadget, ledger and carrier code exists. The ordinary
outgoing path still depends on Reserve/Commit and FI-control responses; complete
production State proving still has rejection paths. A Receive consumer now
exists and must be reused where applicable. There is no demonstrated complete
phone implementation of this consolidated design, or measured end-to-end
2-second proof exchange. The source inventory identifies those exact boundaries.

No build, device experiment, live payment, deployment or implementation-code
deletion was performed for this documentation rewrite. These facts distinguish
the target from its implementation; they add no approval gate.
