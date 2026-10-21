# Disposable Android KAGEMUSHA device probe

This test-only driver invokes the current Kotlin SDK/JNI installation, genuine enrollment,
original-E6 wallet open, monetary review/execute/recovery, and explicit abandonment APIs
on a selected disposable app.
Only actual retained results establish the corresponding stage. Native is the sole source
of installation, enrollment and wallet admission. The controller supplies independently signed issuer/ledger originals; the driver never
signs or submits ledger transactions. Physical carrier delivery is not observed by the
mailbox. Explicit abandonment exports the exact Native terminal outcome and canonical
ledger instruction without submitting it.

## Build and identity inputs

Use the current canonical Kotlin and authenticated JNI packaging corridor. Never
package host JNI or substitute a fake driver. The ordinary signed application-release
trust selection, source custody and complete native artifact inventory still apply.
This test does not set any runtime authority or verifier option. JDK 8 API compatibility
and the existing JDK 21 compiler are unchanged.

The library instrumentation build accepts an explicit developer-only Gradle property
`-PkagemushaEnrollmentProbeApplicationId=org.hyperledger.iroha.kagemusha.enrollmentprobe.campaign_20261013.a`.
It changes only the test APK application ID. Select a real disposable namespace and
signing certificate intentionally, then have the disposable signed AppPolicy admit
that exact package, certificate SHA-256 and versionCode. A default debug signing key
or installed SDK APK does not supply policy approval. Do not install over any existing
production or enrolled application. The target and instrumentation contexts must refer
to monolithic APKs; this increment refuses split APKs. Independently pin both actual
installed APK SHA-256 digests and the full build/native provenance before execution.
The driver compares those public APK digests before entry and after stop; this is an
observer check, not an attestation of compiler custody.

Use exactly one actor and one campaign per disposable application UID. The driver uses
`KagemushaWalletAndroidPlatformV1.create(targetContext)` unchanged, so AndroidKeyStore,
credential-encrypted no-backup custody, lock state and backup rules are the real
platform implementation. A/B/C require separately signed/admitted application
namespaces on this phone, or separate phones. Never redirect the platform environment
or copy wallet custody to manufacture actor isolation. One phone cannot establish a
three-physical-device campaign. Keep the same application installed through retries;
uninstall, clear-data, restore and arbitrary key deletion are not recovery operations.

Select the physical phone by actual device/model/fingerprint and record the selected
ADB serial separately in the controller. This driver contains no ADB command or discovery
fallback. A matching Build string alone is no hardware verdict: real KeyMint evidence
and the issuer's admission remain required. Physical execution also requires an unlocked,
credential-encrypted app context, governed Android attestation roots, current revocation
inputs and the signed policy's actual hardware/boot/app requirements. There is no Apple
App Attest root or entitlement on the Android session path.

## Owner-only staging

The trusted controller must stage `noBackupFilesDir/kagemusha-enrollment-probe` and all
subdirectories as owner-only 0700; every staged regular file must be mode0600, owned by
the application UID, with one hard link and no symlinks. Do not stage via shared/external
storage or upload private session files. The root must already exist. Every absolute
ancestor must remain stable while the harness is running; the path-based mailbox is not
a sandbox against a concurrent same-UID writer replacing directory ancestors. Native
custody independently performs its own admission and storage checks.

The harness publishes its own records through reserved sibling staging files under a
private process-shared publication lock. It synchronizes complete staging bytes before
atomically renaming the stage to the final name without replacement, then synchronizes
the parent directory. The selected inode always has one hard link. After interruption it
may discard a partial unpublished harness staging file; it never
replaces a published original or repairs Native custody. An identical existing record
retries both file and directory synchronization before success. Keep `.publication.lock`
and names beginning `.kagemusha-publication-` reserved for this protocol, and keep them
out of staged command/catalog inputs. Busy publication ownership fails explicitly.

`src/androidTest/cpp/enrollment_probe_rename.c` owns this test-only atomic operation.
The instrumentation APK packages `libkagemusha_enrollment_probe_files.so` for arm64-v8a,
armeabi-v7a and x86_64, compiled with the same pinned NDK 28.0.12674087-beta2 at API26.
The helper retains the selected private parent descriptor, validates bounded leaf names
and a single-link mode0600 source, and calls Linux `renameat2(RENAME_NOREPLACE)`.
A destination symlink or any existing name returns EEXIST. Unsupported syscall, policy
refusal or uncertain post-rename checks fail closed; there is no hard-link, copy or
replacing-rename fallback. No wallet Native ABI, artifact selector or AAR is changed.
The caller synchronizes the selected stage before publication and the file/directory
before success; this helper does not grant Native custody or monetary admission.

Run the pure interruption model with `:kagemusha-wallet-android:testDebugUnitTest`.
On Linux, `python3 kotlin/kagemusha-wallet-android/src/androidTest/cpp/test_enrollment_probe_rename.py`
compiles and tests the exact C syscall core without JNI; these cases do not qualify
Android SELinux. The physical contract cases additionally race the actual JNI syscall
without the helper lock and test source/destination symlinks, private modes and leaf
refusals. Physical publication success does not establish survival of device power loss.

Stage `catalog/` as a complete exact public-original inventory, with no unlisted files.
Required entries are app-manifest.bin, envelope.bin, wallet-runtime.bin, verifier-pack.bin,
producer-inventory.bin, signed-genesis.bin, registration-source.bin, app-policy.bin,
enrollment-policy.bin and the selected canonical account original. `catalog/originals/`
holds the whole native financial catalog and `catalog/registration/` holds the real
finalized registration transport originals. Native authenticates the catalog and same
signed genesis. Generic disposable releases must supply registration; this harness
provides no fixed-deployment shortcut. Only SDK relocation changes the registration
locator, after exact bytes have been independently pinned.

The two policy copies for `AndroidAttestationE1PolicyV1.fromPolicyOriginals` must come
from the same signed runtime and registered asset: the generic runtime carries an
AppPolicy and EnrollmentPolicyTemplate, while Native derives the full EnrollmentPolicy
for the finalized asset. The controller must use the canonical Rust template.for_asset
operation and compare those outputs to the retained signed runtime/registration source.
Kotlin currently exposes no authenticated installed-policy projection and no template
constructor for that derivation; this harness does not introduce one. Its pinned policy
copies only guide the client precheck. Client `Eligible` remains non-authoritative;
Native and the issuer verify the actual selected policy and originals. Never count
passing a caller-pinned precheck as issuer admission.

Each fresh lower-case UUID invocation has `inbox/<invocation>/configuration.json`.
Configuration JSON has exactly these keys: version (string "1"), actor (lower-case UUID),
role (A/B/C), requiredStage (installed/dispatch/e5/e6/opened/abandoned/activated/loaded/
sent/received/folded/unloaded), account (pin), catalog
(array of every pin). A pin has exactly path (relative beneath catalog or the command's
inbox), size (canonical nonnegative decimal string), sha256 (64 lower-case hex digits).
The account pin must also occur identically in the catalog. SDK strict Norito JSON
rejects duplicate members and invalid UTF-8. These descriptors are test DATA, never
alternative wire encoders. The driver freezes run/actor/account/catalog/application
identity outside the invocation namespace; changing it requires an explicitly separate
application, never deleting the identity file to reuse custody.

The instrumentation arguments contain only public selection DATA:
`kagemushaProbe=enrollment-v1`, `kagemushaRun`, `kagemushaInvocation`,
`kagemushaConfigurationSHA256`, `kagemushaGenesisSHA256`, `kagemushaDevice`,
`kagemushaModel`, `kagemushaFingerprint`, `kagemushaTargetAPK_SHA256`,
`kagemushaTestAPK_SHA256`, and `kagemushaTimeoutSeconds` (1–1800).
No argument carries token, DPoP, account key, issuer key or signing secret.
Select only `KagemushaEnrollmentPhysicalDeviceTest#retainedNativeEnrollmentMailbox`
for the campaign. An absent opt-in skips this case in ordinary instrumentation; an
explicit campaign with missing or invalid input fails. A skip is never device evidence.
The separate thirteen device-mailbox contract cases exercise only descriptors and isolated mailbox files,
including interrupted publication, exclusive attempt markers, the actual merged confirmation Activity and concurrent helper instances. Host fault-
injection tests exercise the same publication state machine; they do not qualify Android
filesystem behavior or Native custody.

## Commands and retained originals

Publish command inputs first as immutable private files. Then create `<NNNN>.json`
with exactly action and inputs; inputs maps the closed names below to pins. Then
write `<NNNN>.sha256` containing exactly the 64 ASCII lower-case SHA-256 of the command
JSON, with no newline. Close and read back every transferred file before readiness;
synchronize inputs, digest and their parent wherever the host transport supports it.
Finally create an empty mode0700 `<NNNN>.ready` directory with one exclusive `mkdir`.
The directory's atomic creation is the sole commit marker; a duplicate fails, and a
regular file, symlink, nonempty marker or replaced marker is refused. No former marker
layout is accepted. Before returning a committed command, the driver retains the
marker descriptor and synchronizes the command, digest, marker and parent directory.
Inputs are then retained through the ordinary private publisher before Native dispatch.
These barriers do not constitute physical power-loss qualification.

Sequences run 0000–9999. Wait for the actual finished record before
publishing the next command; command execution is sequential on the instrumentation
worker. Cancellation and the monotonic deadline are checked before and after each
committed selection and again immediately before dispatch. The open-signature callback
applies its own 120-second bound through signature retention and return. Expiry does
not cancel a Native operation that has already started; an incomplete started record
requires explicit recovery. Read-only observation commands are explicit, not polling
that manufactures a new operation. Use fresh command numbers for retries; never rewrite a command or input.

- install: no inputs. It authenticates originals once, retains the actual installation
  attempt and retries register on that same object. Refusal does not reload artifacts.
- start-enrollment / renew-session: access-token and dpop-proof private originals.
  Generic sessions require nonempty DPoP. Android's SDK supplies the empty root field.
  Renew preserves original native dates and evidence; begin with the same retained ID
  is required next. Session originals never enter the public directory.
- begin: no inputs. Exports exact native dispatch and canonical PreKey request.
- permit: response. Decodes only the canonical PreKey Permit response and passes its
  original to Native; exports the actual existing-account challenge.
- authorize: signature. Requires exactly 64 bytes from the independent account signer.
  Native owns key creation. Pending, abandoned and bootstrap-selected remain distinct.
- collect-evidence: no inputs. Returns retained E5 when present. Otherwise uses the exact
  native-selected key target with the real AndroidKeyStore evidence adapter and selected
  policy originals. Exact leaf-first chain copies remain private. Client refusal code
  and holder class are retained, the chain is not submitted, and further collection is
  fenced by an immutable refusal marker. An explicit `abandon` command terminalizes that same unused enrollment through Native;
  never erase this marker or silently select another key. Client success is not issuer success.
- retain-request: signature. Native retains E5 before exporting its exact bytes plus
  canonical Evidence, Issue and Deliver request DATA. Exporting Issue/Deliver does not
  send them or authorize a controller to skip fresh eligibility checks.
- accept-result: response. Decodes the canonical Deliver Credential, then Native verifies
  and retains E6; returned bytes must equal the received original. No activation follows.
- inspect: no inputs. Exports actual native retained E5/E6. Begin must first establish
  the current dispatch when canonical service request envelopes are required.
- open: result. Exact E6 and account are immutably bound. Native ownership transfers
  from enrollment to installed runtime once. The SDK's async admission exports the
  exact fresh account challenge. Reply in `<NNNN>-open-signature.json` with exactly
  challengeSHA256 and signature (private pin). Retain its exact 64-byte digest in
  `<NNNN>-open-signature.sha256`, close/read back all files, then exclusively create the
  empty mode0700 `<NNNN>-open-signature.ready` directory. This uses the same committed
  intake checks, and the signature pin must be exactly 64 bytes and bind the actual
  32-byte Native challenge.
  Only exactly 64 signature bytes are accepted. Callback waiting is bounded to 120 seconds.
- begin-exchange: no inputs. Reserve one Native logical exchange before monetary work.
  Keep it across Request, review, confirmation, issuer and carrier waits. Each command
  admits a child synchronously on the mailbox worker before any SDK worker or UI enqueue.
- end-exchange: no inputs. Fence new work, join accepted children off the UI thread and
  end that exact Native exchange. A failed end retains its owner for explicit retry.
  The probe discards its live financial review through that exact origin after fencing;
  its approved retry state cannot survive end. Time challenge cleanup remains bound to
  its original exchange even after a new exchange is opened.
- release-wallet: no inputs. End the exact logical exchange first. Retires activity callbacks, detaches observations and drains
  the final inactive publication attempt before closing the actual Native wallet. A failed
  publication stays visible but does not prevent Native cleanup after the control fence.
  An exchange-end failure stays visible after the local task fence but cannot prevent
  exact-wallet close/retry. Failed observer or Native cleanup retains that exact owner for retry; Native cleanup
  retries use `retryCleanup`, without enabling the retired wallet again.
  install + open then reopens exact E6 on the same custody without an expired session.
- abandon: no inputs. Explicitly call the canonical Native abandonment API on the same
  unused enrollment, including a definitive E1 refusal. Native alone
  decides whether this is LocalAbandoned (no ledger bytes) or the exact retained signed
  Abandonment frame. Export the signed frame before requesting its canonical ledger
  instruction; preserve partial failures and retry this same action for exact bytes. No
  time override, fresh key, credential issuance or automatic reset occurs. The original
  E1 refusal marker remains. Native refuses after Bootstrap selection. `abandoned` is a
  terminal component stage, not successful enrollment or authenticated ledger inclusion.
  Earlier command failures still fail the campaign summary even after abandonment.
- abandon-expired: proof. Calls `abandonExpired` with the exact independently issued expiry
  proof (1–4096 bytes) for an Accepted/no-Slot attempt. Native authenticates expiry and
  durably retains its immutable Expired marker. The harness retains the same proof on
  every retry, exports only the actual local terminal outcome, and never reads the wall
  clock to infer expiry. Selected slots and uncertain/native errors are not success.
  No ledger instruction, new challenge/key, or replacement credential is produced.
- stop: no inputs. Closes owners in dependency order; never abandons enrollment, deletes
  keys, clears storage or turns a failed close into success.

Ordinary open-signature failure is returned to the SDK future without cancelling it;
the same installed owner retains its challenge/originals for an explicit retry. An
interrupted instrumentation thread explicitly cancels the open future and lets the
SDK fence and retire admission. Do not infer success from missing output. There is no
hard watchdog around native work or vendor operations: the outer timeout bounds
mailbox waiting only. A process restart requires a fresh invocation and re-admission
of the same native custody. Do not replay an old live permit or reconstruct a handle.

Every command retains exact input hashes, start/end `elapsedRealtimeNanos`, closed errors
and actual outcome. Clocks are compared only within that invocation. All failed commands
remain in the population and fail the final summary even if a later retry succeeds.
Successful stop requires the exact requiredStage to have been observed. Public originals
are still restricted campaign evidence, not permission to publish personal metadata.
No error messages, Throwable causes, tokens, DPoP or private paths enter public records.

After collection of a completed Receive's retained output, `request-status`
may return `CREDIT_STATUS` (10). Run `credit` for the same Receive request ID:
fresh native `UNFOLDED` or `CREDITED` evidence satisfies the invocation's
`received` stage. Raw status 10, burned evidence and exports alone do not.
This records recovered credit without reconstructing the old receipt or its
completion time.

## Remaining execution dependencies

Use the current-candidate disposable local four-validator campaign and its reviewed
issuer/controller plan. Historical output directories are not runtime authorities.
The controller still owns genuine signed release/catalog production, register-only
finality, full Android-policy derivation, locally configured fresh eligibility/session
signing, issuer journal and worker setup, Torii request authorization, external existing-
account signatures, app signing and explicit physical target selection. No cloud bank
session, production policy approval, Apple provisioning or existing namespace is inferred.

Before a physical campaign, typecheck the selected candidate, run its contract cases,
produce authenticated same-candidate APK/JNI originals, and perform an explicit device
campaign retaining all failures. Source typechecking and host contract tests do not establish actual hardware prompt, Native
proof latency, carrier evidence, finality, collection or settlement. The maintained commands
below make monetary execution reviewable; each physical gate still needs its own retained
same-candidate result. Failed and inconclusive attempts remain in the campaign population.


## Monetary controller protocol

Commands retain their exact bounded input files before calling the actual SDK. IDs are
nonzero 32-byte words; amount is a nonzero unsigned 128-bit little-endian integer (16 bytes),
and epoch is an unsigned 64-bit little-endian integer (8 bytes). JSON numeric observations
are decimal strings; each 128-bit value is emitted as unsigned `Low` and `High` fields.
No floating-point amount or caller success field is accepted. `KagemushaMonetaryProbeManifest`
is the closed input schema and bound list. Optional fee/Unload charge bundles must be
complete. Signed receipts, policies, time anchors, proofs and finality are admitted only
by Native; a pinned mailbox digest establishes input identity, not issuer authority.

| Command | Input pins | Actual action/output |
| --- | --- | --- |
| bootstrap | none | Native bootstrap status; no activation claim |
| activation | none | Exact activation and ledger instruction originals |
| retain-activation / activation-status / confirm-activation | signed-wire | Native retained attempt/status or verified successful inclusion |
| activation-finality | signed-wire, finality | Authenticate one finality original; only actual confirmation marks activated |
| prepare-load | request-id, amount | Retain exact load plan and export ledger instruction |
| prepared-load | request-id | Recover exact retained plan or report absent |
| load | request-id, receipt, finality | Native Load; completion also reads its released original |
| ledger-finality / ledger-status | finality / none | Native verified prefix only |
| epoch-boundary / epoch-status | epoch, boundary / none | Native authenticated Load epoch progress |
| begin-time | none | Native single-use exchange nonce |
| finish-time | anchor, certificate | Consume exchange once and retain verified issuer response |
| cancel-time | none | Cancel exact live exchange |
| refresh-time | request-id, anchor, certificates | Native TimeAnchor refresh; no caller clock |
| request | setup-id, amount; optionally fee-schedule, fee-certificate | Receiver's exact signed open Request and Native peer envelope |
| decode-request / decode-payment / decode-credited | envelope | Extract expected Native original without granting monetary admission |
| review-send | request-id, request, destination | Actual Native review capability and complete projection |
| review-unload | request-id, amount; optionally quote, certificates, beneficiary | Actual Native review capability and charge-bound projection |
| continue-review-send / continue-review-unload | Same exact inputs as review-send / review-unload | Compare the retained full original projection through Native; a new Ready requires a new explicit decision and authentication |
| decide-review | decision | Cancel an awaiting review, or fresh foreground device authentication then execute exactly that review |
| retry-reviewed | none | Retry only the same approved live typed65 review through its still-open original exchange and acknowledged fence |
| payment-companion | payment | Export payer-companion.norito for this wallet's exact retained Payment, including its historical credential and issuer certificates |
| receive | request-id, payment, companion | Native authenticates the exact Payment and payer companion against the receiver's durably issued Request |
| credit | request-id | Actual Receive credit evidence, optional exact Credited original/envelope |
| observe-credited | request-id, credited | Native durably retains exact Credited before its Send-bound delivery projection; no Archive dispatch |
| resume-deferred-receipt | none | Native verifies at most one retained intake; no Archive dispatch |
| drain-deferred-archive | none | One Native-selected guarded turn; reports NoWork, typed wait, reconciliation or actual execution outcome |
| credited-status | request-id, credited | Exact existing Archive status; never dispatches |
| discard-review | none | Discard the exact retained review through its original exchange, including after end |
| fold-once / fold-status / snapshot | none | Native work/status or current head; only actual folded head marks folded |
| request-status / released-request | request-id | Resolve actual request or read its exact released output |
| retry / released-operation | operation-id | Native retry or exact released output |
| resume | none | Native reconciliation; never a new debit |
| unload-claim | request-id; optionally beneficiary | Exact retained claim and ledger instruction, not settlement |
| unload-status / confirm-unload | transaction-hash, claim | Native history or verified successful exact inclusion |
| unload-finality | transaction-hash, claim, finality | Authenticate/retain prefix; no settlement claim by itself |

The receiver creates Request from `amount`; the payer completes it after reviewing the
Request and the receiver's canonical `destination` account original. After durable Send,
`payment-companion` consumes that exact Payment original. Pass its exact exported bytes
as the `companion` pin to `receive` (maximum 2,048 bytes). The companion supplies the
retained Send's credential and issuer certificate originals, including after renewal;
it grants no monetary authority. Retain Payment and companion together for exact retries.

`observe-credited` first publishes `verified-credited-original.norito`, then exports the
Native projection's evidence, Archive state, `corePending`, credit ID, Payment digest,
amount and return clock. `UNFOLDED` and `CREDITED` are positive delivery evidence;
`BURNED` is authenticated burned evidence, never a positive payment sample. `NOT_ARCHIVED`
and either `corePending` value are valid payer observations. The outcome
`native-payer-delivery-evidence-<evidence>` alone marks no actor stage. Retain the exact
Credited original in Native's canonical queue. `resume-deferred-receipt` retries one
retained intake; `drain-deferred-archive` requests one Native-selected turn after the
logical exchange ends. Native alone rechecks selected recovery, exchange admission,
fold backlog, charging/count/age and exact source. No caller receipt selects fresh
Archive execution. A blocked/no-work result or generic projection grants no actor stage;
only the actual inner completion describes the maintenance outcome. The mailbox does
not start an automatic polling task.

`decision` is UTF-8 JSON with exactly four string fields: `decision` (`confirm` or
`cancel`), `invocation` (this invocation UUID), `reviewSequence` (canonical decimal
command sequence that produced the review), and `reviewSHA256` (exact Native review
bytes). A host confirmation is followed by a new platform `BiometricPrompt` accepting
strong biometrics or the device credential, with confirmation required. This test-only
flow requires Android API 30 or later and a foreground Activity. It never changes lock,
biometric enrollment, KeyMint key settings or platform authentication policy. It has no
cached verdict, test override or software-authentication branch. A real callback is
bound to one Activity ticket and one live review. Failed scans remain with Android's
prompt; cancellation/error/timeout never dispatches. Ticket retirement rejects late,
duplicate or stale callbacks. Activity destruction fails that attempt; an unconsumed
Native review remains owned until explicit cancel, another fresh prompt, or wallet close.
This diagnostic UI plus external exact review is not a trusted-display attestation.

Review issuance binds the nonzero request ID and exact originals in Native. The SDK
returns only Ready (actual capability), Existing (Preparing or an exact bound request
outcome), or Waiting (selected Pending, Archive fold or busy). Existing and Waiting
cannot execute or provide a new latency sample. Execution accepts the actual review
alone; callers cannot supply a replacement request ID.

Immediately before dispatch, both monetary and Nearby drivers publish the full exact
`KWORV1` projection into the durable exclusive `executed-<id>` root under
`private/intents`. Its locked no-replace operation refuses even identical existing bytes.
Failed publication after naming never grants a live retry. Recovery keeps the same
Native request/operation ID; never delete this marker or choose another ID to work around
an uncertain outcome. Ordinary exact Native recovery remains available even when an
older hash-only or JSON-only marker lacks continuation evidence.

`continue-review-send`, `continue-review-unload` and `nearby-continue-send` require the
existing exact operation binding and full root projection. Native compares those DATA
against its authenticated source and original request identity; neither a local file nor
Unknown grants permission. A new Ready receives a fresh explicit decision/authentication,
an immutable invocation/command attempt binding to the root, and a distinct exclusive
attempt fence containing the entire new projection. Old hash/JSON bodies are refused;
there is no fallback decoder. The root is never overwritten. Continuation is excluded
from the original attempt's latency sample.

Typed65 retains the same actual review, original exchange and acknowledged fence in the
live Driver. Only `retry-reviewed` can retry that approval: it verifies the exact fence,
admits another child of the same still-open exchange, and neither prompts nor republishes
a fence. A repeated `decide-review` is refused. Nearby closes its radio session on65 while
keeping this logical exchange open. After exact monetary completion, use
`nearby-recover-send` with a fresh radio session to transport the retained Payment;
that recovery does not reconstruct an original timing anchor. Ending, closing or losing
the process destroys the approved-live retry state; subsequent conditional continuation
requires new Native Ready and new approval. Explicit discard remains cleanup-only.
Stable content bindings refuse changed inputs for a reused ID across commands/invocations.
`stop` or `release-wallet` ends the logical exchange before actual wallet close; keys,
originals and ordinary Native recovery custody remain retained.

Every admitted wallet now owns an activity binding before `opened` is reported. The existing
AndroidX instrumentation lifecycle monitor observes the three actual probe Activities;
STARTED, RESUMED and PAUSED count as visible, while an empty visible set is inactive. The
binding subscribes before sampling, including Activities already running. It observes the
real sticky battery status and subsequent changes: CHARGING or FULL enables the charging
axis; missing, unknown, discharging and not-charging do not. These observations use the
instrumentation host, not an assumed target-application lifecycle. Complete foreground/
charging pairs reach the exact wallet on a separate serial control executor, including
while a monetary call is blocked. Initial publication is acknowledged before commands;
callback failures remain visible during commands and mailbox waiting.

The mailbox loop itself is not a visible Activity. When its temporary prompt Activities
finish, folding can correctly become inactive unless the device is actually charging.
An unplugged foreground fold requires a genuinely visible probe Activity. `fold-once` can
also return idle while the Native background worker owns folding; inspect actual current
fold state. This binding does not implement the separate §5.3 deferred Archive queue.
Qualification still requires matching-artifact phone runs covering initial visibility,
prompt pause/stop/recreation, charging/full/disconnect/unknown observations, transitions
during a blocked command, and cleanup/reopen; source and unit controls are not that evidence.

For A → B → C → Unload, use three admitted actor namespaces and their original accounts:

1. Enroll/open, bootstrap and export each actor's activation. The independent controller
   signs/submits each exact ordinary ledger instruction and feeds retained signed wire
   plus real finality back; require Native activation confirmation. Refresh real issuer
   time and ingest required policy/epoch/finality originals before further operations.
2. A prepares Load; the controller obtains the actual finalized receipt and proof. A
   calls Load with the same request ID, resolves pending results through recovery, and
   observes current folded state before Send review. No exported plan counts as loaded.
3. B creates Request for the selected amount. A decodes B's exact Request envelope and
   reviews Send using B's canonical account original. Confirm the exact projection and
   complete A's fresh device prompt. Recover the exact Payment original/envelope from
   actual durable Send output, then export its exact payer companion on A.
4. B decodes that Payment envelope and calls Receive with the Payment original and the
   exact companion exported by A. Retain durable Receive, current head/fold observations
   and exact Credited evidence for A's durable `observe-credited`. C creates its own Request;
   B reviews it and repeats the fresh prompt/Payment plus companion/Receive/Credited
   exchange with C. Native refuses onward Send review until its actual source
   prerequisites are satisfied.
5. C reviews Unload, confirms the exact projection through a fresh prompt and resolves
   the Native operation. Export its exact Unload claim/instruction. The independent
   controller signs/submits it, then supplies the actual transaction hash, exact claim
   and ordered real finality. Require Native Unload confirmation for `unloaded`.

The physical Nearby driver uses the same receiver-first Request → Payment with payer
companion → Credited exchange. `KagemushaNearbyProbeManifest` owns these closed commands:

| Command | Input pins | Actual action/output |
| --- | --- | --- |
| nearby-identity | none | Export this live driver's engineering transport identity |
| nearby-permissions | none | Request actual Android Nearby permissions |
| nearby-prepare | setup-id, amount; optionally fee-schedule, fee-certificate | Retain receiver Request with its canonical account companion; export nearby-request-ipm1.bin and nearby-request-hash.bin |
| nearby-send / nearby-continue-send / nearby-recover-send | request-id, session-id, request-hash, peer-certificate | Discover the selected receiver, authenticate, freshly review, conditionally review, or recover exact Send, then verify its returned Credited without ArchiveSent |
| nearby-receive / nearby-recover-receive | request-id, session-id, request-hash, peer-certificate, setup-id | Advertise, authenticate, send the retained Request, receive Payment with its carried companion and return fresh Native Credited |

Stage each peer's exact `nearby-certificate.bin` and the receiver's exported Request hash.
`session-id` is a nonzero 16-byte word; `request-hash` and `peer-certificate` are nonzero
32-byte words. Each failed or restarted radio attempt requires a fresh session ID while
retaining the same monetary request ID and immutable intent inputs for recovery. The
receiver's setup ID selects its retained Request. After mutual authentication, Request
is the first encrypted record; the payer carries the exact native Payment and companion
together in `IrohaPeerWalletPaymentV1`. The receiver passes both to `receiveWithCompanion`.
No separate mailbox `payment-companion` command is needed during that physical exchange.
Prepared metadata and the engineering transport identity do not establish carrier
delivery, hardware attestation or shipping qualification.

The payer persists `nearby-credited-original.norito` before `observeCreditedForSend` and records
the typed projection before admitting a positive diagnostic. Authenticated burned evidence
is retained but fails that positive diagnostic; it is not a Native verification error.
An unarchived or pending-fold payer projection remains eligible. The positive outcome is
`native-exact-send-delivery-verified-over-nearby`; its local elapsed interval ends at the
Native delivery-verification return, before any separate ArchiveSent. Receipt arrival
remains a separate timing observation. Keep the exact retained Credited for later archive.

Nearby Receive recovery accepts the native `CREDIT_STATUS` result after old
Receive output collection. It reads fresh covering evidence through
`receivedCredit(requestId)`, without looking up the collected release. An
ordinary `COMPLETE` result still records its retained release separately before
reading fresh credit evidence. Missing, burned or nonreceiver evidence fails the
diagnostic; only the exact native Credited original enters the acknowledgement.
Sender recovery still requires `COMPLETE` for its retained Send, followed by a positive
Native delivery projection for the returned Credited. Recovery does not reconstruct the
original completion time.

Mailbox copying is not physical QR/NFC/Nearby delivery evidence. Run the maintained
carrier separately when qualifying a carrier. Each command records invocation-local
`elapsedRealtimeNanos`; prompt start/success, reviewed dispatch, durable Native return,
review-ready and current-head-fold observations are separate events. A recovered result
is timed as a present observation, never backdated to an unknown historical commit.
Do not subtract timestamps across invocations/devices or count host fakes as p95 trials.
An external campaign controller must retain its own bounded timing/source identity and
all failed, timed-out and incomplete trials to calculate the two-second p95 gate.

The first external Activate POST additionally requires the network continuation's prepared
signed-wire handoff and a portable `activation-retained.json` original. The driver emits
that closed JSON receipt only after the actual `retainActivationAttempt` call returns
NotStarted without confirmation, rejection or verified history. Its eight string fields
bind the exact signed-wire SHA256/size, invocation/sequence and Native-return clock.
The controller must independently join the exact queued command, its started digest and
successful finished record before explicitly authorizing the host POST. A prepared file,
activation export, progress or family-confirmed status cannot substitute for this initial
retention handoff. Recovery retains the original signed transaction and never re-signs it.
