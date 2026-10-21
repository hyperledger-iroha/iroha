# Disposable native monetary stage contract

This extends the physical enrollment XCTest mailbox. Every command calls the
current Swift SDK on the actor's actual Native owner and real Apple platform.
It is test glue; full-SDK arm64 iOS typechecking and host harness tests do not
replace matching-source Native linking or physical monetary execution.
It supplies no account key, FI issuer, ledger signer, ledger execution, monetary
implementation or qualification verdict. Only exact externally produced originals
can complete those boundaries. Native refusals remain refusals.

A/B/C use distinct canonical accounts, UUID roots and Native custody slots in one
explicitly isolated disposable app. Slots on the same phone share the app and its
actual Keychain access group; this does not simulate independently signed apps.
The selected iOS device is iPhone17 Pro Max (`iPhone18,2`), iOS26.7 build23H24.
Pixel6 is the other selected device; its corresponding driver is maintained in
`kotlin/kagemusha-wallet-android`. Running all three actors on iOS does not qualify
Android or a cross-OS population. Monetary qualification requires a matching
authenticated package cohort and actual execution on each selected device.

The test host binds each admitted wallet to actual UIKit app/scene foreground
state and battery charging/full state. Foreground-inactive authentication remains
foreground; unknown battery state does not count as charging. Observation starts
before the initial sample, and `open` acknowledges publication before reporting
success. Complete state pairs run on a separate serial control queue so a busy
Native command cannot delay inactivity. Battery monitoring is shared within the
test host and restores its previous setting after the last observer detaches.

Activity errors stay visible to subsequent commands and mailbox idle polling,
which emits one bounded `activity-failed.plist` and prevents successful qualification.
Release/stop remain available. Teardown detaches observers, fences admitted updates,
publishes inactive and closes the exact wallet off the main thread. Setup failure
retains its admitted owner for cleanup; failed close retains that retired owner
for another explicit close. These observations enable the existing Native folding
scheduler. `fold-once` may still return Idle while the background worker owns the
slot; judge current Native state. Explicit deferred receipt recovery and guarded Archive turns use Native queue selection; the probe supplies no eligibility verdict.
The added ordering/lifetime tests and physical lifecycle behavior have not yet
been executed against the changed sources.

## Mailbox originals

Use the enrollment interface's private root, pins, independently pinned catalog,
configuration and sequential ready-file protocol. `command.plist` contains exactly
`action` and `inputs`, with each input a `{path,size,sha256}` file pin. Each input
must lie below the current invocation inbox. The harness retains all exact input
bytes privately before Native execution and never replaces an existing original.
No input named `success`, `verified`, `balance` or `proof-override` is accepted.
The full wire frames are Norito originals. Plists below are harness metadata only.

IDs are exactly32 nonzero bytes; distinct intents need distinct IDs. `amount` is
exactly16 bytes, unsigned little-endian low64 then high64, nonzero. `epoch` is
exactly8 unsigned little-endian bytes. These scalar inputs are forwarded as typed
SDK values; the harness does not encode ledger or wallet protocol frames. All
fees, account identity, scale and proof authority come from Native verification.

Use `begin-exchange` before Bootstrap or a fresh monetary protocol, including Request
preparation, review, issuer-time exchange and either carrier. The mailbox records the Native
reservation and accepts each task before enqueueing it on the actor. Keep that exchange
across `nearby-prepare` and its continuation, review confirmation, and all carrier teardown.
`end-exchange` fences future admission before its actor hop and joins off MainActor.
`discard-review` and `cancel-time` remain cleanup-only after that fence; a new exchange
cannot consume an old review or time challenge. Release/stop fences ingress before closing
the exact wallet. A failed close retains the same owner for cleanup.

A typed Native65 result keeps the exact approved live review and acknowledged on-disk
fence. `retry-reviewed` explicitly retries that same review on the same exchange;
`discard-review` remains cleanup. QR/Nearby keep the actual carrier attempt alive and
show a separate “Retry approved Send” button. This button performs no new confirmation
or authentication and never republishes the fence. Cancellation/deadline/background
loss drains and discards the capability. No generic error clears a fence or restores
approval, and these retries provide no latency sample.

The closed commands and exact sets of input names are:

| Action | Inputs | Meaning |
|---|---|---|
| `open` | `result` | Exact canonical E6; Native account admission through a live callback |
| `release-wallet` | none | Close/join this owner, keep all custody and originals |
| `bootstrap` | none | Native restart-safe Bootstrap; retain actual completion status |
| `activation` | none | Export Native Activate original and ledger instruction |
| `retain-activation` | `signed-wire` | Retain actual outer signed transaction **before** external dispatch |
| `activation-status` | `signed-wire` | Observe that exact retained attempt |
| `activation-finality` | `signed-wire`, `finality` | Ingest one next actual finalized block |
| `confirm-activation` | `signed-wire` | Native successful inclusion confirmation |
| `prepare-load` | `request-id`, `amount` | Persist/export the exact Native Load instruction |
| `prepared-load` | `request-id` | Observe the retained plan, without selecting another ordinal |
| `load` | `request-id`, `receipt`, `finality` | Admit actual matching ordinary-ledger receipt and finality |
| `ledger-finality` | `finality` | Advance Native's verified Global-chain prefix |
| `ledger-status` | none | Observe retained prefix, including absence |
| `epoch-boundary` | `epoch`, `boundary` | Verify the exact incumbent epoch boundary |
| `epoch-status` | none | Observe Native epoch authority |
| `request` | `setup-id`, `amount`; optionally both `fee-schedule`, `fee-certificate` | Export the receiver's Native Request and envelope |
| `decode-request`, `decode-payment`, `decode-credited` | `envelope` | Native extract of expected peer type; no monetary admission |
| `review-send` | `request-id`, `request`, `destination` | Retain live native review and exact projection |
| `review-unload` | `request-id`, `amount`; optionally all `quote`, `certificates`, `beneficiary` | Review actual native Unload terms |
| `continue-review-send`, `continue-review-unload` | Same exact inputs as the corresponding original review | Conditional review using retained full original projection and companions; same request ID |
| `retry-reviewed` | none | Explicit retry of an actual approved-live Native65 review on its original exchange |
| `decide-review` | `decision` | Explicit exact-review confirm/cancel; confirmation also invokes iOS authentication |
| `receive` | `request-id`, `payment`, `payer-companion` | `receiveWithCompanion` using exact Payment and companion originals |
| `credit` | `request-id` | Native credit projection and original Credited evidence |
| `observe-credited` | `request-id`, `credited` | Native retains the exact receipt before verification; no Archive dispatch |
| `credited-status` | `request-id`, `credited` | Exact original payer Archive status without dispatch |
| `resume-deferred-receipt`, `drain-deferred-archive` | none | One bounded Native-selected intake recovery or guarded maintenance turn |
| `begin-exchange`, `end-exchange` | none | Synchronous requested reservation before actor enqueue; explicit fence and off-main join |
| `discard-review` | none | Cleanup of the exact retained review, including after end was requested |
| `fold-once`, `fold-status`, `snapshot` | none | Native local fold or exact current-head observations |
| `request-status`, `released-request` | `request-id` | Recover/observe actual native request mapping |
| `retry`, `released-operation` | `operation-id` | Retry/observe the exact Native operation |
| `resume` | none | Reconcile/finish the selected Native operation |
| `unload-claim` | `request-id`; optionally `beneficiary` | Export actual retained claim and ledger instruction |
| `unload-finality` | `transaction-hash`, `claim`, `finality` | Verify next block for the exact submitted claim |
| `unload-status`, `confirm-unload` | `transaction-hash`, `claim` | Observe history or confirm exact successful settlement |
| `begin-time`, `cancel-time` | none | Actual owner-local direct issuer time exchange |
| `finish-time` | `anchor`, `certificate` | Consume native exchange with actual issuer response |
| `refresh-time` | `request-id`, `anchor`, `certificates` | Native TimeAnchor refresh after direct exchange retention |

After collection of a completed Receive's retained output, `request-status`
may return `CREDIT_STATUS` (10). Run `credit` for the same Receive request ID:
fresh native `UNFOLDED` or `CREDITED` evidence satisfies the invocation's
`received` stage. Raw status 10, burned evidence and exports alone do not.
This records recovered credit without reconstructing the old receipt or its
completion time.

`observe-credited` retains `verified-credited-original.norito` before calling Native
and records the typed evidence, Archive state, pending-core flag, credit/payment
identities and amount. Burned evidence remains an explicit observation; it is not
unburned credit or a successful timing sample. A not-yet-archived receipt or a
pending core descriptor does not invalidate verified unburned delivery. The command
grants no new actor stage. Native errors, unavailable custody or lost outcomes
remain failures; an ordinary unknown status cannot replace receipt verification.
For later idle Archive work, finish the logical exchange with `end-exchange`, then
invoke `drain-deferred-archive`. Native selects the retained entry and rechecks eligibility;
`resume-deferred-receipt` recovers retained unverified intake. `credited-status` observes
the exact original Archive without dispatch. The receipt observation command
does not dispatch ArchiveSent, erase Payment, or authorize regeneration.

Released Send exports include `payer-companion.norito`, recovered by Native from
the exact retained Payment, including after payer credential renewal. Carry those
bytes beside the Payment using `IrohaPeerWalletPaymentV1`; the receiving mailbox
requires them as `payer-companion`. A companion supplies credential DATA, not
monetary authority, and changing either original under a retained Receive ID fails.

Receipt/anchor/certificate bounds are512 bytes; fee schedule/quote1024;
payer companion2048;
canonical account/beneficiary4096; peer originals/envelopes/certificate bundle10000;
signed transaction/claim65536; epoch boundary256KiB. Finality intake is capped at
36MiB overall; `load` itself has the SDK's stricter256KiB finality bound. E6 uses the
SDK's exact `RESULT_MAX_BYTES`. Native retains every canonicality and protocol
bound check. An external file's hash or a signed transaction's existence proves
neither admission nor ledger execution.

For initial activation, the network controller first prepares one exact versioned
signed transaction without submitting it. After the real `retain-activation` call
returns Native's not-started result (no confirmation, rejection, verified height
or block hash), the driver exports `<SEQ>-activation-retained.json`. It has exactly
these string fields: `schema` (`iroha.kagemusha.wallet-activation-retained.v1`),
`action` (`retain-activation`), `signedWireSHA256`, `signedWireBytes` (decimal),
`nativeResult` (`not_started`), `nativeReturnedTicks` (decimal), `invocation` and
`sequence` (decimal). Progress, rejection or confirmation cannot produce this
initial-retention receipt. The controller pins its exact bytes and joins the
retained command, started event and successful finished event before acknowledging
retention and permitting submission of the unchanged transaction. This JSON is
test-controller DATA, not a cryptographic attestation or confirmation of finality.

## Open and review callbacks

After successful enrollment, stage the exact exported E6 as `open.result`. The
actor transfers `loadInstalledRuntime()` from its live enrollment owner. After a
restart use `install` then `open` with the same E6 and account; no expired FI
session is necessary (`session` is an empty dictionary in this case).

`open` exports `<SEQ>-open-account-challenge.bin`. While that command awaits the
external account signer, provide:

```text
inbox/<INVOCATION>/<SEQ>-open-signature.plist
  challengeSHA256 = SHA256(exact exported challenge)
  signature = {path, size:64, sha256}
inbox/<INVOCATION>/<SEQ>-open-signature.ready
  exactly64 ASCII bytes: SHA256(the plist), no newline
```

Stage the exact64-byte externally generated account signature first and publish
the ready marker last. This callback waits at most120 seconds; Native deadlines
remain authoritative. The harness checks cancellation and its continuous-clock
bound before and after selecting readiness and again after retaining the signature,
before returning it to Native. Command intake also checks its bound immediately
before dispatch; readiness or suspension during publication cannot bypass expiry.
An expired started record needs explicit recovery, and no harness deadline cancels
an already dispatched Native operation. An ordinary timeout retains the SDK's exact pending owner.
Retry `open` in a new command in the same process; the SDK may already retain a
signature and then does not invoke the callback again. Actual Task cancellation
has the SDK's custody-retirement semantics and is not manufactured as a timeout.
A new process gets a new invocation and live challenge; never transplant a stale
callback or permit. No private account key enters this harness.

`review-send`/`review-unload` export `<SEQ>-native-review.bin` plus native amounts,
fees, debit, destination digest, current head and source commitment. The live
`KagemushaWalletReviewV1` remains in the same actor. A decision plist has exactly:

```text
decision = "confirm" or "cancel"
invocation = exact current invocation UUID
reviewSequence = exact sequence that produced the retained review
reviewSHA256 = SHA256(exact native-review.bin)
```

A cancelled review is passed to Native `discardReview`. A confirmed review first
invokes real `LAContext.deviceOwnerAuthentication` with reuse disabled and a prompt
identifying its review hash and native gross atomic amount. The app Info.plist
contains the required Face ID usage description. Authentication failure is
retained as a failed attempt; no debit call occurs. This diagnostic UI combines an
external display of the full native projection with actual iOS authentication;
it makes no claim of a hardware-authenticated display or NFC tap confirmation.
Native `executeReviewed` still consumes the real owner-local token and rechecks
its complete source. An exported projection can never recreate that capability.

## Physical QR commands

The iOS probe uses the system CoreImage renderer and AVFoundation rear camera with
existing IRQR/IPM1 codecs. `NSCameraUsageDescription` belongs to the host app.
These closed actions select local identities; none accepts imported QR text,
images, envelopes, Payment, Credited or a caller success verdict:

| Action | Exact inputs |
|---|---|
| `qr-send` | `setup-id`, `request-id` |
| `qr-receive` | `setup-id`, `request-id`, `amount`; optionally both `fee-schedule`, `fee-certificate` |
| `qr-recover-send` | the original `setup-id`, `request-id` |
| `qr-recover-receive` | the original `setup-id`, `request-id` |

Start `qr-receive` on the recipient and `qr-send` on the payer. The recipient
shows its Native Request for the selected amount, together with its mandatory
account original. Point the payer's rear camera at that screen. The payer reviews
the Native amount and destination, authenticates, and displays the released Payment
with its exact payer companion. The receiver scans it and displays Credited only
after Native observes durable credit. This is the receiver-first three-code
Request → Payment → Credited exchange.
Only the payer's exact Native Request review plus fresh system authentication
permits dispatch. Native authenticates the Request and its account companion.
Request/Payment correspondence is checked against the exact displayed/scanned
Request before Receive or Payment display. Native retains all proof, signature,
custody, conservation and Send-bound acknowledgement checks.

Each attempt retains originals, complete envelopes, IPM1 messages, generated QR
frames and actual camera observations separately. The renderer round-trips each
symbol through the system decoder and checks its actual version (at most 17) and
M error correction. An oversized static symbol uses the existing canonical
animated mode. Animated frames change every 150 ms. Display and camera callbacks
share a maximum of 4,096 retained observations and an attempt deadline of 300
seconds. Every cancellation, permission refusal, background interruption, expiry
and slow attempt remains evidence; neither limit silently resamples the run.
These limits belong to this test actor, not the wire protocol.

The receiver's **Close display (delivery unconfirmed)** button records only that
its already proven durable credit's Credited screen was closed locally. It does
not prove payer receipt. The payer's diagnostic succeeds only after camera
reconstruction of Credited and Native-backed `observeCredited` returns unburned payer
evidence for the selected Send. Payer timing retains camera arrival and Native
verification separately, measured from
that same phone's fresh authentication callback; operator handling and retention
costs remain included. Archive completion is outside this measurement. Both QR and
Nearby retain the exact Credited original before verification for a later explicit
Native-selected `drain-deferred-archive` turn after `end-exchange`. Neither carrier starts Archive work.
Burned evidence is recorded before the positive-payment diagnostic fails, with no
successful latency sample. Native Archive state and pending-core observations are
recorded independently of the evidence verdict. No clock from another phone is
subtracted. Recovery has no new end-to-end latency sample for the original operation.

A recovered setup retains its original Request, account companion, Payment and
payer companion, amount/fee inputs and IDs. Rescan the exact retained Request or
Payment carrier as prompted; its paired originals must match.
If a dispatch fence already exists, recovery uses Native status and retained
release/current credit projection; it never re-confirms or creates a replacement
Send/Receive. An incomplete Native result remains a failed QR attempt. Resolve
that exact pending operation through the existing `request-status`, `resume` or
`retry` commands before another QR recovery attempt. A full current review fence with
all original companions can instead use `qr-continue-send` (the same setup/request IDs)
or `nearby-continue-send` (the same request ID and Request, with a fresh radio session).
These ask Native for a conditional new review and require fresh owner confirmation.
A historical Native66 result is observed without dispatch. Never erase the original marker. Foreground loss, cancellation and scene disconnection drain the
camera and Native worker before releasing the shared carrier owner.

The three-code migration has source review and updated gate/schema tests. Those
tests, full hosted compilation/linking against a matching authenticated SDK/native
candidate and execution on the specified physical phones remain open. Earlier
parser, adapter and portable-test results do not validate these changed sources.
Android has no corresponding optical QR actor yet; existing text codec tests do
not establish cross-platform QR delivery.

## Recovery and timing

Before every Load/Receive/reviewed operation, the harness freezes the action and
exact input hashes under the stable request ID. Setup and Load-plan identities
are similarly frozen. New inbox paths with identical bytes are valid retries;
changed intent under the same ID is rejected. The harness cannot replace Native's
journal or infer success after uncertain delivery.

Before dispatching a confirmed review, the exact full Native projection is published
as an exclusive immutable execution fence. Exact action originals and companions were
retained before review. After interruption, `request-status`, `resume`, `retry` or
`released-*` observe the same custody and IDs. `continue-review-send` or
`continue-review-unload` may request conditional review only with those retained originals
and the initial full projection. Native independently checks current source and definite
absence before issuing a new capability. Each confirmed continuation publishes a separate
exclusive attempt fence; the initial fence and original request ID never change.
Old 64-byte hash-only fences, malformed projections, missing/changed originals and
restored/rollback-uncertain custody remain refused. No compatibility decoder recreates
their lost evidence. Native67 is tokenless waiting; Native66 is an already bound request,
never a new review or dispatch. The local awaiting-confirmation and approved-live65 states
are distinct. Reviews, approval and direct-time tokens cannot survive process reopening;
Native monetary outcomes do. The stock iOS double-anchor-loss residual remains open. `release-wallet`, `install`,
`open` provide a non-destructive same-process reopen sequence. `stop` closes all
owners. Neither command abandons enrollment, deletes keys, replaces custody,
erases apps, restores arbitrary developer files or claims power-loss testing.

Every command preserves actual status, detail, sequence and original output.
Pending, preparing, not-performed, archived and delivery-loss statuses remain
visible; they are not labelled completed. Completion exports use the SDK's actual
`releasedOutput` projection; peer frames use the native `envelope` operation.
Mailbox publications synchronize private sibling staging before an atomic rename
that refuses an existing destination. Interrupted staging cannot occupy a final
name. An identical final output is synchronized again on retry; different bytes
are always refused. This helper never repairs or deletes Native journal state.
A fresh invocation can inspect Native custody and publish into a new output root.
Every failed command remains in the results and causes the XCTest summary to fail
even if a subsequent explicit retry succeeds.

`mach_continuous_time` timestamps and the invocation's timebase are retained for:
command start/end, Native returned completion, observed retained completion,
actual iOS confirmation, current-head fold observation and the next successful
native Send review. Values are comparable only within the same invocation/phone;
never subtract A's clock from B's clock. The first complete Receive observation
is not retroactively relabelled as its historical commit time. Fold-ready means
only exact-current-head Ω observed; the next successful Send review separately
establishes that specific operation's readiness under current Native checks.
Monetary completion and fold completion remain separate. Confirmation-to-complete
includes this test driver's own retention overhead and is not an optimized UI
latency measurement. Hardware/OS interrupts and all unsuccessful attempts remain
in the evidence population.

Nearby Receive recovery accepts the native `CREDIT_STATUS` result after old
Receive output collection. It reads fresh covering evidence through
`creditProjection(receiveRequestId:)`, without looking up the collected release.
An ordinary `COMPLETE` result still records its retained release separately before
reading fresh credit evidence. Missing, burned or nonreceiver evidence fails the
diagnostic; only the exact native Credited original enters the acknowledgement.
Sender Payment recovery remains `COMPLETE`-only; receipt verification uses the
typed payer projection independently of Archive progress. Recovery does not
reconstruct the original completion time.

For A→B→C, select the actual carrier and preserve independent physical
ingress/egress evidence. The QR actor performs its own camera/display exchange;
generic mailbox export/decode commands require external carrier execution. The mailbox's file copy is not carrier success.
Real cross-phone P1a timing needs an externally observed common boundary, or the
specified one-device request/ack round trip; these local clocks alone cannot
produce a cross-device duration. After B's fold and successful onward review,
execute the reviewed onward payment to C. Source balances and destination net
amounts come from native projections; do not assume all-value send is fee-free.
Each Unload uses real signed ledger submission and exact finalized history before
native confirmation. HTTP status, proof generation, claim export and any single
component result are insufficient to declare settlement or release readiness.
