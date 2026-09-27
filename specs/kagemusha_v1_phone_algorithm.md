# KAGEMUSHA V1 phone algorithm and release contract

Status: design draft for the first release, 2026-09-23. This document states
the required algorithm and the current implementation boundary. It is not a
claim that any phone profile is qualified or that production monetary
operations are enabled.

## Goal and threat boundary

An ordinary installed phone app may transfer value offline only when an
enrolled phone hardware primitive irrevocably authorizes **at most one
proof-valid exact-next successor** of the current monetary state. The app and
native Core can prepare and prove a transition; neither may select a second
successor by restoring host files, replaying a command, reinstalling the app,
cloning a snapshot, changing the clock, or restarting during a partial commit.
This document specifies one proof obligation and two possible hardware
realizations: a durable-checkpoint service, or an attested strict-next counter
or one-use-key ratchet. The ratchet needs no monetary-state parser in the
hardware and is the ordinary-app phone candidate. Both remain unavailable for
production until their actual enforcement, original-result loss and recovery
are qualified on the target devices. The checkpoint contract remains the
existing monetary corridor; the app ratchet now has staged state commitments,
attestation candidates and distinct governed profiles, but no admitting
recursive hardware-assertion fold.

Online top-ups are the issuance authority: each finalized debit of online
funds backs one MintFold credit in the pooled reserve. The aggregate balance
is inductively backed by the set of finalized MintFolds; recursive proof
relations conserve value through subsequent transitions. Bootstrap and Rotate
do not create new value. The device counter is anti-fork enforcement, not a
source of value. Two branches from one predecessor can each have internally
valid lineage proofs; disconnected recipients cannot detect that conflict
from those proofs alone.

The accepted proof package must establish both properties: the aggregate's
finalized backing and conserved recursive lineage, and an authenticated
hardware selection for each value-changing successor. The terminal proof
binds the real hardware authorization evidence, predecessor, successor and
counter; the verifier checks that evidence against the enrolled device
credential and signed release. Separately, the issuer must sign an enrollment
record binding independently verified app attestation and the platform-enforced
app/key access policy to that credential and release, and acceptance must
verify that record. The raw platform evidence and verifier result must be
retained and auditable. A claimed app version or release hash is not an
attested binary measurement unless its distribution evidence establishes it.
Current proof fields bind the hardware credential and release but do not
carry an app-attestation transcript; production must add or authenticate the
enrollment binding before claiming the complete app-plus-hardware proof. A
host capability bit, arithmetic lineage alone, or app attestation alone
cannot stand in for a hardware-selected transition.

The design has two independent admission gates:

1. The issuer verifies the authentic app instance and its signing identity,
   plus a fresh challenge bound to the FI, authentication namespace, account,
   network, ledger dataspace, asset incarnation and scale, release, policy,
   device lane, and device credential. Platform app attestation is evidence
   for this gate. It does not itself select an offline state successor.
2. A phone-resident hardware primitive holds a non-exportable enrolled key and
   rollback-resistant one-use authority accessible only to the approved app
   identity. The checkpoint profile also holds the current checkpoint/counter
   and performs atomic compare-and-select, returning an authenticated,
   retained terminal certificate. The counter/key-ratchet profile instead
   proves that only one signed successor can carry the exact-next use index
   and must specify fail-closed lost-result recovery.

## Inductive proof obligation

Acceptance verifies a single inductive predicate, `Valid(head)`. Its base is a
hardware-selected zero Bootstrap under an enrolled credential. Every successor
must consume the verified proof(s) for its exact predecessor state and, for a
received credit, the sender's exact proof and unique credit identity. It must
verify the operation relation: MintFold adds only an externally finalized
reserve-backed credit; SendSplit subtracts the exact amount it creates as one
receiver-bound credit; ReceiveFold adds that credit once; RedeemSplit removes
the redeemed amount once; Rotate conserves balance and replay state. The
successor also requires its own authentic hardware terminal evidence over
the exact predecessor, successor, release, Guard digest, one-use operation and
exact-next counter. Its governed device credential must be bound to the
issuer-verified authentic-app enrollment. The recursion carries authenticated
ancestor credential/terminal-history commitments, so a later valid-looking
head cannot erase an unbacked earlier step.

The release requires constant-size proof and verification work after at least
1,024 real handoffs. Therefore the device's raw signature/assertion and its
canonical subject must be verified inside a recursively folded proof relation
or an equivalently succinct, independently verifiable relation. Checking a
signature only on the sender's phone cannot be trusted by an offline receiver;
shipping every ancestor's raw signature as a growing sidecar fails the
constant-size release contract. The proving witness may contain raw evidence,
while the accepted compact proof must bind its verified result to the
governed credential and exact public state transition.

One Core-derived canonical selection subject `S` must be shared by every phone
adapter and proof verifier.

`S` is exactly 460 bytes: the 49-byte NUL-terminated domain
`iroha:kagemusha:v1:hardware-transition-selection\0`, the body length 403 as
u64-LE, then fixed-width fields in this order: version u16-LE; release ID,
provider-policy root, app-policy digest, credential ID, raw 32-byte `NetworkId`,
lane commitment and hardware-profile ID (32 bytes each); policy epoch u64-LE;
hardware-epoch ID (32 bytes); hardware-epoch generation u64-LE; operation tag
u8; transition-statement digest, candidate-envelope digest and terminal-body
commitment (32 bytes each); secure indices before and after as u128-LE. The
provider-policy root is the receipt-authenticated provider credential Merkle
root from the admitted release. Bootstrap establishes index zero without a
signed selection; the first later operation signs 0→1. Norito remains the
storage and transport format, but its header and checksum are not part of `S`.
The model-owned byte ranges in `hardware_selection.rs` define the exact proof
binding offsets.

For Apple App Attest, `clientDataHash = SHA256(S)`,
`nonce = SHA256(authenticatorData || clientDataHash)`, and ECDSA-P256-SHA256
verifies the nonce (so the ECDSA prehash is `SHA256(nonce)`). Its authenticated
counter is inside the authenticator data, not a free field in the subject.
For an Apple release profile, the governed app signing identity is the SHA-256
hash of the exact App ID and must equal the assertion's RP ID. The complete
authenticator data is signed. Where Apple supplies signed validation-category
and bundle-version extensions, the release verifier must check them against
the app policy. The physically tested iPhone 17 Pro Max returned 37-byte
assertion authenticator data with flag `0x40`, counter values 1 then 2, and
no extension suffix. That form authenticates the App ID, attested key and
counter, but it **does not attest the app version in each assertion**. The
issuer must bind release policy at enrollment and cannot label an assertion's
release ID as an independent binary measurement. Apple's iOS 27 extension
format adds release signals; it is a distinct current platform format to
qualify on a physical device. The parser must not reject the observed `0x40`
flag or require absent extensions.
On 2026-09-24, a development-signed ordinary app on a physical iPhone 17 Pro
Max (iOS 26.7) enrolled a dedicated key, produced two exact-`S` assertions
for 0→1→2, and passed a separate consumed-but-lost-result journal test.
The signed app had no HCE entitlement; App Attest operated through the
ordinary development app path.
A separate physical test signed two distinct selections that both claimed
the same 0→1 predecessor on one enrolled key. Their independently verified
assertions carried counters 1 and 2, so only the first met the exact-next
counter relation. This tests competing selections in one process; it does not
establish rollback resistance after device restore or qualify monetary admission.
Independent verification of exported raw evidence passed the pinned App
Attestation certificate chain, the Apple Root CA G3 fraud receipt, the exact
challenge and both signatures/counters. The physical nonce certificate
extension used DER `SEQUENCE { [1] EXPLICIT OCTET STRING(32) }`; an untagged
octet string is not this observed format. Development category 3 and the
signed app's CFBundleVersion `1` were governed app-policy expectations; Apple
did not sign that version in the observed assertion. The iOS native archive
and app build used an explicitly unsealed, one-slice diagnostic package after
source changed during compilation. These tests establish device API behavior,
not a source-sealed release, monetary checkpoint or hardware no-fork proof.
For that profile, `app_release_digest` is SHA-256 of the domain
`iroha:kagemusha:v1:app-attest-release\0`, the validation category as u32-LE,
the UTF-8 bundle-version byte length as u16-LE, and those exact version bytes.
A direct hardware ECDSA service may instead sign the subject under its exact
governed hash profile. These are different signature equations over the same
selected transition and require exact cross-language vectors. The Swift
App Attest candidate checks the fixed V1 selection shape and signed
predecessor index against Core's trusted counter before reserving an assertion,
then hashes the exact Core canonical selection frame;
Core also checks its assertion signature, signed release extensions, and
exact-next counter against the persisted candidate before proof construction.
The Core verifier now requires the original two-field CBOR assertion rather
than trusting extracted authenticator and signature fields, and accepts the
observed 37-byte assertion header while verifying release extensions when
present. Governed profile
classes separately bind the OEM checkpoint guarantee set, Apple App Attest
guarantees, and Android KeyMint one-use-key guarantees; the app profiles do not
inherit checkpoint-service claims. Cross-language byte vectors and recursive
signature verification remain required before this candidate can carry
monetary authority. The pinned generic
Halo2 ECC helper assumes a curve coefficient of zero and is unsuitable for
P-256. A dedicated `a = -3` point and bounded ECDSA relation is present, but
its full-width proof and recursive assertion binding remain unqualified.
The staged Apple original-assertion relation now composes exact two-key CBOR,
canonical DER, the signed `S` and authenticator bytes, the SHA-derived P-256
equation, and copy constraints from both signed secure indices and the Apple
counter to passed state cells. The five-case original-assertion selection
passes in both Pasta fields, including signed-index and DER mutations; the
dedicated P-256 carry geometry now uses three 87-bit limbs. The live Terminal
now binds its derived body hash to the original signed `S` and binds the
release, profile, candidate, credential ID and app-policy digest to the same
source cells. Its focused two-parity tests cover those links. The staged
original assertion and issuer-enrollment equations are still absent from the
live monetary fold. They cannot simply be inserted into its fixed k16,
32-job geometry: even a reduced-window staged P-256 test uses k18, while a
real assertion needs the full 256-bit equation and additional SHA work. An
independently verifiable, release-keyed assertion relation must bind the
original evidence, credential and exact Terminal source and be recursively
folded in both parities before ordinary-app admission can open.
A separate staged composition now feeds the same assigned signing subject and
authenticator bytes through the available State, Guard, governed Apple policy,
compact credential-ID SHA opening and original assertion/P-256 relations. The
credential issuance and expiry must fit the SHA-bound governed profile
window in both Pasta fields; a focused Eq/Ep mutation test covers this bound.
The composition is compile-checked as a helper,
not yet proof-tested as one combined circuit; a coherent non-Bootstrap Apple
State/Guard fixture and the missing issuer/terminal bindings are still needed.
The source-staged paired outgoing terminal relation now derives journal and
recovery digests through six appended SHA jobs, then derives the terminal-body
hash from those results and assigned State, Guard and certificate cells. It
requires exact outgoing bytes and the release-pinned manifest source. These
jobs join the same typed recursive claim after State and Guard verification;
the 32-job proof and fixed geometry still require compilation and real-proof
qualification in both Pasta parities before production use.
The recursive State proof now carries six public limbs for the outgoing
preparation ID and two sealed-stream digests, and the native preparation ID
uses a fixed, domain-separated pre-proof transcript. These are candidate
commitments. An isolated terminal helper opens the exact 475-byte preparation
transcript and both bounded sealed streams for SendSplit and RedeemSplit against
recursively verified State carriers, with both-parity mutation tests. Core can
export the six exact SHA preimages from a retained committed send or redemption
candidate. A native committed-redemption test checks those six exporter messages
against Core's retained digests. A source-staged genuine redemption diagnostic
now carries a funded Core candidate through State, Guard, the 32-job claim and
Terminal; its expensive paired proof has not yet compiled or run. The live
terminal fold has a release-authenticated
redemption-manifest source in its 83-cell terminal public column and includes
the six appended jobs. Its compiled 32-job geometry, paired proof, release keys
and physical backing remain unqualified. The ID cannot authorize a production
terminal outcome until those checks pass.
This is an unsatisfied proof obligation even for an OEM checkpoint profile;
testnet execution must not be cited as proof of production hardware backing.

`Valid` is the conjunction of the recursive monetary relation, authenticated
online issuance/finality anchors, complete hardware/credential history and
the current device's non-forking one-use successor selection. A proof field
containing an asserted counter, app identity or certificate-shaped bytes does
not satisfy that predicate. Cryptography authenticates what the device
signed; the claim that its signed counter was enforced in tamper-resistant
hardware depends on
the governed service implementation, provisioned access rule, trusted
attestation and physical qualification. No amount of proof recursion alone
can establish that physical enforcement.

An app-hosted NFC/card-emulation process or a host-maintained counter cannot
stand in for gate 2. An attested hardware assertion counter **might** enforce
one-use no-fork selection if a dedicated key signs the exact predecessor and
successor and the recursive verifier requires exact-next (`after = before + 1`);
a hardware-enforced single-use key committed by the prior state is the
analogous key ratchet. App identity, hardware enforcement, inability to clone
or reset the key, one-use uniqueness, all-ancestor proof binding, exact
release scope and crash semantics must be verified independently. If an
assertion is consumed but its original response is lost, accepting a skipped
counter could admit an unseen spend. That lane must freeze until a sound
reconciliation procedure exists. Neither counter candidate is currently an
enabled production profile. No external card, cloud serialization, debug
service, software monetary fallback, or compatibility path is part of the
qualified production profile.

Apple [documents](https://developer.apple.com/videos/play/wwdc2026/201/) that an
App Attest key is invalidated when its app is reinstalled or the device is
restored. A newly attested key is a new credential; it cannot
continue an old offline head or reset the old counter. Such a lane stays frozen
unless an authenticated online recovery procedure can reconcile all exposed
value and authorize a new hardware lineage.

## Authoritative state: durable-checkpoint profile

The secure service's compact current selection binds the immutable owner and
lane; authenticated release, hardware profile, policy and credential; hardware
epoch generation/ID; exact per-epoch logical sequence, journal revision and
one-use hardware authorization counter; current
aggregate/Guard commitment and nonce commitment; the roots and revisions of
the accepted-credit inbox, sender outbox, response history and recovery
checkpoint; and a monotonic metadata generation that does not roll back on
epoch rotation. Complete proofs, encrypted credits and append-only journals
may live in protected host storage only when their exact bytes and prefixes
are committed by the selected hardware checkpoint. A locally cached
checkpoint is never freshness authority.

The Core transition statement includes the consumed and produced aggregate
commitments, predecessor and successor epoch, sequence, journal revision,
device-policy binding, fresh state nonce, effect digest and normalized
GuardBundle digest. A normal transition must advance sequence and journal
revision exactly once. Terminal authorization separately constrains the
hardware authorization counter to advance exactly once and binds its
one-use digest to the predecessor state, nonce, lane, epoch, device key,
logical sequence and journal revision. A proof witness or host integer for
that counter is insufficient: the qualified service must attest and
atomically enforce the counter together with its selected state head.
A governed epoch rotation may reset the per-epoch
sequence and journal revision only while advancing the hardware epoch to its
unique next generation; it carries the complete balance, replay and recovery
state and cannot reset the global metadata floor or reauthorize an already
consumed one-use authorization.

The hardware transaction for a new **aggregate-head transition** has the following logical
contract. The physical service may use a compact anchor rather than store
every host byte, but it must enforce the same comparison atomically:

```text
Commit(operation_id, canonical_command, prepared_candidate,
       authenticated_core_authorization, exact_predecessor):
    if operation_id is committed with identical canonical bytes:
        return the original retained terminal certificate and response
    if operation_id is known with different bytes:
        reject conflict
    authenticate app access, active credential, release and policy
    authenticate the Core authorization for this candidate and one-use nonce
    compare (epoch, logical sequence, journal revision,
             hardware authorization counter, predecessor commitment,
             intent digest)
        with the one current hardware checkpoint and sealed intent
    check the candidate's exact-next epoch/sequence/journal/root bindings
    check trusted-time or monotonic-lease, amount and operation obligations
    atomically consume predecessor and authorization nonce,
        select one successor checkpoint/counter,
        retain the original response and certificate
    return that retained response
```

No signature over a proposed successor grants acceptance unless this compare
and select occurred. A second proposal consuming the same predecessor fails
even if it comes from the same authentic app and the same attested key.
Storage exhaustion fails before publication; it never discards the selected
head or an acknowledged terminal response. Proof and APDU framing remain
canonical, bounded, and version 1 only.

Inbound staging is a separate atomic insert against the hardware-owned inbox
root and revision; it need not consume the aggregate head. For a credit ID,
the service compares the exact request/payment bytes, rejects a conflicting
reuse, and returns the original durable receipt for an identical retry. A
new receipt is published only with a durable inbox-root successor anchored
to the current hardware checkpoint. ReceiveFold later consumes that credit
under the aggregate-head transition. This separation permits concurrent
arrival without permitting duplicate value.

For the strict-next counter/key-ratchet profile, the proof head and its exact
hardware use index replace the selected monetary checkpoint. The hardware
must authenticate at most one canonical successor at each exact next index;
it need not parse the aggregate or store the host journal. Core and the
recipient verify the raw assertion/signature, enrolled key, app-bound
credential, exact predecessor and successor, and index at every recursive
link. Host files are not freshness authority. A snapshot restored to an older
head cannot obtain another valid exact-next authorization for that head.
This profile cannot acknowledge a merely staged inbound credit without a
hardware-backed inbox primitive. It performs and proves ReceiveFold before
acknowledging the credit. If an authorization is consumed but its original
result is unavailable after a crash, the lane freezes offline; skipping an
index is invalid because an unseen payment at that index may exist.

## Enrollment and release admission

1. Pin a signed release manifest, proof artifacts, native contract vector,
   exact FI, authentication namespace, account, network, ledger dataspace,
   asset incarnation and scale, release scope and hardware policy.
   Bind an app-attestation challenge and a secure-service key-possession
   challenge to the same enrollment transcript. Persist the native nonce and
   selected release, profile and lane before any external call. Use the
   six-field attestation transcript
   `domain || client_nonce || server_nonce || release_id || profile_id ||
   attested_key_id || lane_id`, where `domain` is
   `iroha:kagemusha:v1:app-device-attestation-challenge\0` and each field is
   exactly 32 bytes. The issuer signs a 273-byte preparation containing those
   six fields and its issue/expiry times before platform attestation.
   On Apple, generate and retain the App Attest key first; its key ID is
   SHA-256 of the attested SEC1 public point. On Android KeyMint, obtain the
   signed preparation first and use the all-zero key-ID sentinel only under
   a governed Android profile, since KeyMint fixes its challenge during key
   generation. Generate the key with that challenge and derive its actual
   point/reference from verified attestation. No key may be qualified from
   a device-provided claim alone.
   The Android SDK has a JNI preflight for this exact 273-byte signed
   preparation. It requires an independently pinned canonical issuer policy,
   its SHA-256 digest and trusted service time, compares the signature to the
   supplied process-owned selection, and returns only the verified server
   nonce for KeyMint's challenge. The native enrollment owner still rechecks
   the complete authority. Without those independently supplied inputs, app
   enrollment remains unavailable. A checked-in cross-SDK vector now fixes
   the independent issuer's canonical policy, signature and 273-byte Android
   preparation exactly. The Core issuer generator and Rust JNI verifier both
   pass it; an unsealed Pixel 6 Android instrumentation run passes the same
   positive vector and rejects changed release, signature, service time and
   policy pin. The Swift issuer-preparation verifier accepts the same Android
   vector only under the governed zero-key-ID binding; Apple selection requires
   the generated nonzero App Attest key ID. This qualifies only the preparation
   preflight, not a device counter or monetary enrollment. Before a new
   attestation, verify the signed fields against the pinned issuer policy and
   current trusted issuer time. Exact recovery of an already-requested,
   durably retained certificate may reauthenticate those signed fields after
   expiry, but may not start another attestation or native qualification.
2. Verify the app attestation using its platform trust chain and approved app
   identity. The independent verifier signs both the actual SHA-256 key ID and
   the derived device-key reference from the same raw attested point, together
   with the exact raw-evidence digest. Apple's signed preparation key ID must
   equal that certificate key ID. Android's signed preparation contains the
   governed zero sentinel, while its certificate carries the actual attested
   point's key ID. The verifier durably issues at most one certificate for an
   attested key. Obtain the governed operation-1 qualification afterward;
   the issuer checks its exact public point, profile, lane and app-policy
   binding against the signed preparation and verifier certificate. The native
   pending selection retains its original nonce and deadline across this
   certificate-before-qualification phase; resuming never creates a second
   key, nonce or qualification after an ambiguous result. Separately
   verify the secure-service credential, non-exportable key, epoch, access
   rule, policy and qualification report against the signed release. A
   device-feature bit or an app-provided Boolean cannot create either
   capability.
3. Select a new lane only through authenticated provisioning of its one-use
   hardware authority. A checkpoint lane restores from the challenged
   current selection plus exact retained journal prefixes. A ratchet lane
   restores only with an exact proof head and hardware use index; a gap
   freezes it. Missing or corrupted history never becomes a new lane.
4. Keep each release/profile unavailable until the actual hardware supplies
   its complete authenticated contract. The checkpoint service's sixteen
   required capabilities and closed set of twenty-two operations apply to
   that profile. The ratchet profile instead requires attested unique
   exact-next use, non-resettable enforcement and fail-closed loss behavior.
   A capability frame is a claim to authenticate and test, not authority.

The native initial-enrollment adapter now journals one selection, challenge,
proof and issuer completion through six bounded phases. Its accepted challenge,
prepared proof and completed admission are consuming Rust types tied to the
original revocable selection; a restart cannot recreate them from app frames.
The app-side phase owner correlates those six frames to one selected account and
signed release; a lost cancellation reply permits only an exact retry of the
same revocation ticket. No qualified issuer/app-evidence delegate or installed
monetary backend is present, so these phase mechanics do not yet admit a wallet.

## Payment and recovery algorithm

The only peer messages are a signed PaymentRequest, a proof-backed Payment,
and a signed Acknowledgement. The request binds network, release, asset
incarnation, amount, receiver identity/key, hardware credential, unique
request ID and validity window; it does not reveal the receiver's balance.

For a sender, persist a unique operation ID, exact request bytes, intent and
outbox allowance before device dispatch. Core authenticates the release and
artifact set, constructs and locally verifies the private SendSplit
precommit candidate relation, checks the exact hardware/Guard statement and
obtains a release-pinned authorization. The selected device profile then
authorizes the exact predecessor-to-successor transition. A checkpoint
service atomically selects the head and retains the original terminal
certificate for exact-response recovery. A strict-next ratchet signs the
canonical transition at the required next index and cannot sign another
accepted successor at that index. After the authenticated hardware result,
Core derives and verifies the terminal recursive proof that binds the actual
evidence, then durably installs the exact terminal envelope and outbox before
exposing the Payment. A lost checkpoint reply uses exact-operation recovery;
a lost ratchet assertion freezes the offline lane. Neither starts a fresh
value-changing request from the prior head.

For a receiver, verify the signed request/payment bindings, genuine recursive
proof, terminal evidence, credential and release. Under the checkpoint
profile, stage the *exact* request/payment bytes with a unique credit ID into
the hardware's rollback-resistant inbox before publishing the
Acknowledgement. Exact duplicates return the original receipt; conflicting
bytes or reused IDs fail. ReceiveFold later consumes that staged credit once
under the next hardware checkpoint. Under the ratchet profile, verify credit
nonmembership, execute and prove ReceiveFold under the next one-use hardware
authorization, and durably install the resulting head before publishing the
Acknowledgement. A duplicate returns a durably retained original response;
if that response is lost, the lane freezes. Proof ancestry, aggregate
conservation, unique credit consumption and exact-next anti-fork selection
must hold together.

For mint/top-up, atomically debit online funds and credit the pooled reserve
under one idempotent mint-credit identity, then verify an externally
authenticated ledger finality anchor before staging or folding offline
value. A node response does not authenticate its own finality. Redemption
likewise needs the exact verified finality and reserve/nullifier obligations
before a hardware release authorization. An outbox release or redeemed
liability is recorded only after the corresponding authenticated terminal
result and durable head installation; immutable operation IDs, evidence and
replay anchors remain retained.

At every crash boundary, retain the original intent/command before hardware
dispatch. In the checkpoint profile, restart challenges the hardware for its
current selection, authenticates the original reply and journal prefixes,
and resumes only the recorded phase. If hardware advanced but the host
snapshot did not publish, finish the retained candidate against the original
certificate. In the ratchet profile, persist the original assertion and
result before any peer exposure; if the counter advanced but the result is
missing or unprovable, lock offline monetary operations pending an
independently sound settlement procedure. Never reset, reissue a one-use
authorization, skip an index, or choose a host snapshot as the current head.

## Phone platform mapping

The platform adapter owns transport/session and the platform hardware API.
Native Core owns proof verification, durable intent and exact-response
admission. The selected hardware primitive owns the non-exportable key and
non-forking use authority. The UI never converts transport
success, an app-attestation pass, or a decoded APDU into a monetary success.
Authentication of the original hardware result under the accepted device
credential precedes host publication or admission of the returned outcome.
A checkpoint service may already have advanced before its response arrives,
so an uncertain result is recovered from that service. A ratchet result that
cannot be recovered freezes the lane.

On Android, a built-in checkpoint-service profile must select an authorized
embedded secure-element reader and the exact provisioned applet/access rule.
OMAPI discovery, StrongBox/KeyMint, a UICC reader, or an SD reader alone do not
qualify this checkpoint profile. The ordinary-app candidate may use an attested
hardware-enforced single-use KeyMint key ratchet with each state's exact next
public key commitment, but Android allows software enforcement of use limits
on devices without the relevant hardware feature. The verifier must check the
hardware-enforced one-use and rollback-resistance authorizations, app scope
and key continuity. Lost-signature recovery remains unresolved. Vendor and
firmware tuples require separate evidence.
The mandatory Pixel 6 profile cannot use this KeyMint ratchet as presently
specified. On a physical Pixel 6 running Android 16, a forced StrongBox
`setMaxUsageCount(1)` diagnostic produced StrongBox attestation but placed the
use limit in `softwareEnforced` (tag 405), with neither hardware use-limit nor
rollback-resistance (tag 303). The first signature verified and the second
failed in the framework; that failure does not prove hardware one-use. Its
connected embedded SE has no observed access rule for the current applet.
Pixel 6 needs a separately provisioned, app-accessible hardware counter or
checkpoint service with signed original-result recovery, or a new physically
verified primitive satisfying the same no-fork relation. An attested app and
StrongBox key alone cannot be substituted for that relation.
After the connected Pixel 6 moved to an Android 17 user build, it still
advertised neither hardware single-use nor limited-use Keystore support, and
did not advertise the hardware Identity Credential feature.
Its embedded `eSE1` reader was present, but no access rule for the current
applet AID was observed. A fresh test-only Android 17 instrumentation probe
reached the reader but `OPEN_LOGICAL_CHANNEL` returned
`ACCESS_CONTROL_ENFORCER_DENIED`; it observed a StrongBox key but no hardware
no-fork selection. This recheck does not establish that an applet is installed
or that this app can select it. The concrete internal eSE service
and physical acceptance contract is in
[`kagemusha_pixel6_ese_service_contract_v1.md`](kagemusha_pixel6_ese_service_contract_v1.md).

On iPhone, an HCE entitlement permits app-hosted card emulation but does not
grant the app access to implement the **checkpoint-service profile** in the
built-in Secure Element. App Attest protects a Secure Enclave key and exposes
an authenticated assertion-use counter. A dedicated App Attest key could be
evaluated for the strict-next candidate above; Apple's server guidance checks
only that the counter increased, and it does not document retained original
responses or a monetary rollback-resistant checkpoint. Device tests and a
safe consumed-but-lost-assertion recovery protocol are required. Thus the
currently specified checkpoint-service monetary path remains unavailable with
only HCE access. A built-in-SE checkpoint profile requires the platform's
Secure Element Credential grant, an approved applet/service implementing the
exact checkpoint contract, a provisioned configuration, owner access for
maintenance, user-authorized wired transactions for value-moving use and
physical qualification.
Secure Element Credential grants applet/session/APDU access; the custom
service and physical evidence must separately establish the exact-next
counter and journal. HCE is optional for KAGEMUSHA. NFC, QR and nearby links
are transports for the same canonical peer bytes; none authorizes value.
An HCE entitlement is required only when that transport uses HCE, and its
scope and user experience must be qualified separately.

## Experimental testnet execution

The testnet corridor must admit monetary top-up, offline peer-flow and
redemption experiments while hardware qualification proceeds. A testnet-only
release may use an explicitly labeled experimental device adapter or
structural certificate
to exercise end-to-end wallet, proof, transport and ledger behavior. Such
evidence proves only the relations it actually checks; it must not set a
hardware-qualified claim or be accepted by the production verifier. Bind the
experimental release, keys, asset and reserve to the exact test network so
its transcripts cannot be replayed under a production release. This is one
V1 wire layout, not a compatibility decoder or an alternative production
security contract.

The signed V1 release purpose distinguishes `Production` from
`TestnetExperiment` and commits the exact experimental asset identity,
incarnation, decimal scale, and reserve pool into the release ID and authority
attestation. The production Torii runtime and wallet state context reject an
experimental release; the testnet proof observer accepts only its exact signed
scope. A process-local, non-spendable mint trial accepts only the opaque result
of an Applied top-up paired with a verified MintFold and rejects changed
retries or duplicate credits. The existing release receipt still requires
production qualification. Kagami can explicitly prepare, threshold-sign,
assemble and authenticate the experimental release and its distinct
structural receipt against exact native artifact and evidence hashes. A trusted
producer now converts an independently pinned verifier projection into a typed
receipt and content-addressed artifact/evidence handoff that Kagami rehashes.
Genuine structural evidence, real proof artifacts and independent signing
authority remain required before a candidate can be issued from real devices.

The structural device certificate and real-proof corridor still require
genuine release evidence and artifact generation. An explicit Experimental
node top-up path is source-staged, but it has not yet been validated with a
real signed release and four-validator Applied finality evidence. Redemption
and offline peer value movement are not qualified by that top-up path. Node
admission requires the explicit
`settlement.kagemusha.allow_testnet_experimental_release = true` opt-in;
its default is false. This source-stage switch is not proof of live Applied
finality or genuine device evidence. Live testnet top-up and redemption
require the configured command runtime and
authenticated proof release; submitted operations must reach Applied finality
before counting as funded or redeemed. A testnet experimental device profile
therefore needs explicit release/network-scoped proof and runtime installation,
not a global hardware-admission bypass.
The native testnet State observer requires operator-pinned network, asset
identity, asset incarnation, scale, reserve liability pool, and authenticated
release; it verifies the actual paired State proof and returns only an
unqualified observation. A Rust-only owner can retain one process-local lane
or create a private, exclusive, append-only testnet journal. The durable path
fsyncs the exact confidential mint reservation before online submission and
replays every retained State proof and Applied mint against the concrete signed
release and independently resupplied finality anchors after restart. An exact
operation anchor must first be pinned through the Rust-only trusted finality
path; coordinates supplied by JNI cannot pin themselves. The native testnet
finality-chain helper verifies every consecutive signed Sumeragi bundle from
an independently authenticated first height-context ID before pinning the last
context. The native installer now requires an opaque verified bootstrap token.
Its bounded canonical V1 package carries threshold signatures over the network,
asset/incarnation/scale/reserve, release and attestation identities, first
height-context ID, validity interval and sequence. The native verifier checks
an independently provisioned authority policy and deployment selection; the
package cannot choose its own trusted keys. It rejects expired, future,
regressed and same-sequence changed checkpoints, while allowing an exact
still-valid retry. The canonical checkpoint and threshold verifier live in the
shared data model. Kagami prepares checkpoints from an authenticated Experimental
release, signs partial approvals under independently supplied deployment and
lifetime pins, and assembles distinct approvals into the exact native package.
Each output is a new private file; issuing a package does not provision a phone
or establish current time. Native trusted time and a retained sequence/digest
pin are inputs; ordinary app storage does not establish rollback resistance. The
verified token has a suspend-inclusive installation lease capped at 120 seconds
and the checkpoint's remaining validity; loading, journal replay and publication
must finish within it or require fresh verification. Native verification starts
that lease before calling the freshness reader, so a suspension between reading
trusted UTC and verifying signatures cannot renew stale time. The
installer derives every release/finality pin from that token and authenticates
the matching release and artifacts before installing the owner. Swift and
Android can activate a Rust-provisioned native startup context by transporting
only that bounded signed package. Native provisioning fixes the authority,
release archives, proof layout, private paths and create/recover mode once;
native freshness ownership supplies current trusted time and replay state.
The native online freshness owner creates an unpredictable nonce and retains its
continuous start before transport. It consumes a threshold-signed response bound
to that nonce, the exact checkpoint digest and sequence, and independent deployment
pins. The authority must durably retain the checkpoint before signing and supply
a trusted UTC interval that includes its uncertainty. Verification requires the
signed lower bound to follow issuance and the signed upper bound plus all native
elapsed time to precede expiry. Every provider read advances that upper bound;
device sleep, slow replies and signature work consume time, and no handset wall
clock is consulted. The online attempt lasts at most 120 seconds; a subsequent
installation uses the separate bounded lease described above. This source does
not supply the deployed authority service, its operational keys, or app provisioning,
and its retained bootstrap pin is not an offline wallet anti-rollback counter.
Activation retains the verified checkpoint before installing the actual host,
and an exact retry rechecks freshness and authentication. A changed checkpoint
cannot replace a live host. An uncertain durable mutation or partial installation
requires process restart and authenticated recovery. There is no reset or
caller-supplied verification callback across C/JNI. Publication, C/JNI dispatch
and public Rust mutation calls share one native ownership gate; partially
installed global owners cannot be called before final publication. Rust host
operations require scoped access with a guard-borrowed permit that cannot move
across threads. An operation panic permanently revokes dispatch for that process;
even a callback that swallows the error cannot return a successful result. Inherited
process identities are rejected before acquiring native ownership or clock
locks. Approved deployment
signatures and a concrete independently provisioned native context remain
required; neither a status hint nor the bundle response can choose the
checkpoint. JNI finality
coordinates are comparison evidence only and cannot install a pin. The
observer's MintFold entry
requires that original reservation, an Applied chain top-up,
both release-authenticated mint proofs, exact linkage to the paired State proof
and a unique credit ID; exact mint and non-mint retries return their original
unqualified observations, while changed transcripts are rejected. The mobile
JNI carries the original Torii status JSON, independent
finality coordinates and paired proof bytes, never the private credit opening.
The durable owner can project a positive, proof-verified testnet value record
only for a retained Applied top-up with the exact reservation, signed release
scope, unique credit ID and independently pinned finality anchor. Its native
C and Android JNI entrypoints accept only the operation ID; the returned
Norito archive is copyable inspection evidence, not a spend credential.
A distinct Rust-only, append-only testnet mint-credit registry consumes the
opaque native admission, fsyncs each unique positive credit, and rederives
every retained credit from the durable proof owner on recovery. Exact retries
are idempotent; changed operation evidence, duplicate credit IDs, and scope
changes fail. A Rust-only native host API opens this ledger under the installed
durable owner and credits a finalized top-up by operation ID while holding that
owner's lock. A process-local Rust host composes release installation, private
pre-submission reservation, signed-finality pinning and observed crediting with
typed ordering tokens. It requires matching native-trusted create/recover modes
for the two journals. Mixed modes fail closed because path absence cannot
distinguish interrupted installation from rollback; interrupted installation
requires a trusted external checkpoint and explicit repair before restart.
The signed finality chain is verified once per pin attempt, and the returned
anchor comes from the same verified token used by the native owner.
Its C mobile endpoint accepts only that 32-byte operation ID and
returns a bounded canonical Norito archive with the counted credit and ledger
total. The Swift and Android inspection adapters require the exact canonical
archive layout, the requested operation ID, positive value and finality, and
`hardware_qualified = false`; Android also checks the native 16-byte alignment
for both admission and credit frames. Callers cannot pass a copied archive as
authority. Concrete deployment provisioning, native private mint preparation,
wallet account crediting, and testnet spending remain required; the registry does
not enter production monetary state or attest hardware.
The terminal hardware fold and production admission
remain separate. File permissions protect the journal's
confidential opening from other ordinary processes; this is not secure-element
storage. Both private WALs detect malformed or partially written frames but
have no trusted hardware head, so an attacker able to replace either with an
earlier complete prefix can roll its testnet observation or counted-credit
history back. Neither may serve as the phone's anti-rollback monetary counter.
The signed release manifest includes the exact genesis-derived network ID.
Node startup, native proof verification, enrollment and testnet observation
reject a release signed for another network before admitting its artifacts or
proofs. This prevents cross-network release reuse; it does not qualify device
hardware or enable the experimental monetary route by itself.
Core now has a read-only projection of the original paired outgoing State
proof and canonical public inputs from a retained, authenticated candidate.
It re-verifies the pair and rejects released or stale operations. Method 14 of
the native coordinator ABI now exports this projection by Core operation ID to
Swift and Kotlin adapters, but stock builds have no installed qualified backend.
The projection therefore does not make the phone observer or a testnet payment
path operational by itself.
On Pixel 6, a source-staged experimental collector binds a StrongBox signature,
attestation challenge and app-private intent journal to the canonical selection
frame and exact network, release, lane and counter inputs. Its focused Android
collector suite passes 15 JVM tests; the connected Android 17 Pixel 6 confirms
StrongBox signing while placing the one-use limit only in software-enforced
attestation. A source-matched JNI/app package and device run remain pending. This
collector cannot yet authorize peer value exchange or redemption; a separate
exact-network experimental coordinator and release are required. The software
use limit leaves no-fork unproven. Its raw observation carries an experimental
profile and cannot be relabeled as hardware one-use evidence.
The testnet phone probe can obtain a 460-byte frame directly from the Rust
data model without pasted hex. For this raw observation only, eight synthetic
identifiers use a distinct Pixel 6 diagnostic domain, the current app-owner
scope and exact network; the operation is Rotate with index 0→1. These bytes
are model-canonical but are not an issuer release, enrolled credential, real
transition statement or monetary lineage. The diagnostic JNI export is separate
from the coordinator and cannot satisfy production release admission.

Testnet admission should still enforce finalized issuance, conservation,
unique credit use, exact network/release binding, durable idempotency and
unambiguous operation recovery. A missing physical hardware profile alone
must not close this experimental corridor. Each test result records whether
the app and hardware evidence were real, simulated or absent; only verified
real evidence may advance production qualification. The public testnet
ingress and configured authority must actually be live before a remote
write is counted as a completed test.

## Claim split needed for the phone memory gate

The current structured V1 Claim is not a 128 MiB phone prover. Its configured
minimum has 96 advice columns at `k = 16`: one Base, fourteen carrier-RLC,
two 37-column dense-MSM lanes, and seven native-Poseidon columns. One resident
advice evaluation bank is therefore 192 MiB. The prover also retains thirteen
fixed and nine permutation evaluation columns at that point, a 236 MiB lower
bound before coefficient banks, proof scratch, and the app. Compact key storage
does not change that live memory. Keep the 128 MiB whole-process gate closed.

A candidate replacement must partition the **authenticated source inventory**,
not merely trim the two 4,090-cell carrier columns. The existing complete
Claim permits 1,008 sources per parity, with four canonical `u128` cells per
source and 58 bound cells in each carrier. Its deferred-batch Poseidon challenge
commits the complete ordered source namespace, verifier-input binding, equation
tags/selectors, and bound values *before* aggregate coefficients are formed.
Five k15 arithmetic slices are the minimum for one dense-MSM lane per slice:
`k15 usable = 32,768 - 9 = 32,759` rows, each source costs 130 rows and the
job costs three, so `floor((32,759 - 3) / 130) = 251` sources per lane;
four one-lane subclaims cover only 1,004 sources. For a fixed
five-slice candidate, let `S` be the circuit-authenticated source count and
slice `i` cover `[floor(i*S/5), floor((i+1)*S/5))`, for `i = 0..4`.
At `S = 1,008` the ranges are `[0,201)`, `[201,403)`, `[403,604)`,
`[604,806)`, `[806,1008)`. Every slice has at most 202 sources and needs at
most `3 + 130*202 = 26,263` dense rows. A paired slice's two carriers would
each need at most `4*202 + 58 = 866` active cells before constrained padding.
These are row bounds for the source terms. The implemented arithmetic slice
adds a public start point with coefficient one and endpoint with coefficient
minus one to the identity equation, using `3 + 130*204 = 26,523` dense rows.
Its fixed 202 slots expose all original canonical point/scalar limbs, alongside
`S`, slice index, both interval bounds, active count and endpoints: 817 public
field cells. Bounded Euclidean remainders constrain each floor division;
every inactive slot must contain the canonical generator and zero coefficient.
Both parity layouts configure 55 advice, three fixed and 21 permutation columns:
15 Base gate columns, three lookup columns and one 37-column dense lane. A single
advice/fixed/permutation evaluation bank therefore already needs 79 MiB; these
arithmetic layouts exclude global authentication, carrier binding and joins.
The dense witness now computes the forbidden curve offsets once and chooses
the first permitted generator multiple within the complete `2A+1` bound for
`A` active digits. This removes valid-input rejection at the former arbitrary
256-candidate limit without retrying the full trace for each candidate.

Each parity must first authenticate the same original ordinary parent/shard
proofs, folds, exact verifier inputs, ordered source identities and equation
inventory as the unsplit Claim. The new global challenge must be derived once
from that complete authenticated inventory under a domain-separated transcript;
all slices must use that exact challenge and prove their own source points,
coefficients, and partial curve sum. A local challenge per slice, a host-supplied
source digest, a freely chosen partial sum, or a hash of unauthenticated carrier
bytes would change the relation. A binary join must verify its children,
their adjacent index ranges and common global statement, and constrain its
sum to the group addition of their partial sums. Its root requires range
`[0,S)`, exact count `S`, and the identity point as the complete batched
equation result. Duplicate, omitted, reordered, or padded nonzero sources
must fail. The existing two-challenge common-prime carrier binding must cover
each paired Eq/Ep slice and link every slice to the same global inventory;
both fields must validate the other field's canonical point/scalar limbs.

The unsplit Claim now exposes its existing complete-source challenge through a
typed, circuit-owned helper before coefficient aggregation, with mutation
fixtures for both Pasta fields. This is a refactoring of the current verifier
graph, not an independent source-authentication proof. The separate k15 slice
arithmetic is not linked to that complete inventory or to child proofs and a
root join, so neither component qualifies the phone gate.

The separate inventory circuit now reuses the complete scalar verifier graph,
native Poseidon jobs and reciprocal carrier binding, exposing columns of
113, 4,090 and 4,090 cells. Its two extra semantic cells bind both source counts
to the verifier graph's shape; the carriers expose the ordered source and
coefficient limbs, common statement and constrained zero padding. This circuit
is not selected by the release. Its final root must still verify both parity
proofs, authenticate their actual carrier commitments, and bind the five slices
and joins. Full-circuit tests exercise Poseidon and the entire carrier evaluation
schedule using a small genuine deferred equation; that fixture does not contain
the complete recursive parent/shard proof graph or establish phone resource use.

The arithmetic tree has exactly five Eq and five Ep slice proofs plus four
binary joins in each field: eighteen proof instances per transition, before
any separate source-authentication or final composition proofs. These other
proof counts are not established yet. The accepted roots must expose the
unchanged 97-cell external Claim statement, including release and both plan
digests, complete SHA stage/job cursor, both chaining states, message and
terminal roots, both protocol/audit/proof-chain digests, and all 34 history
limbs. The final relation must recompute the typed SHA jobs and bind the
actual terminal body through the existing terminal fold; copying a claimed
terminal root from a child is insufficient. The 14 proof-internal carrier
binding cells and two carrier columns may change shape only as part of a new
reviewed circuit, never by weakening the external verifier's checks.

The current Poseidon batch geometry would need at most
`39 + 202 + 7 + 202 + 14 = 464` mandatory permutations for a 202-source
slice if its proof-input and protocol-point contribution can be partitioned
without adding rows. One k15 lane fits `floor(32,759/66) = 496`
permutations, leaving only 32 for optional native transcript work; the
existing transcript scheduler and whole-inventory source authentication do
not meet this envelope by construction. One dense and one Poseidon lane plus
minimum Base and RLC would require at least 56 advice columns, or 56 MiB for
one k15 advice bank. Actual Base, key, verifier, join, and peak-process
memory have not been measured. The fixed slice arithmetic is implemented as a
test-only prerequisite; authenticated subclaims and joins remain unimplemented.

TODO: Implement a standalone circuit-authenticated global-inventory proof,
five paired bounded slice relations, reciprocal carrier binding and exact
binary joins; prove the complete source-to-root and terminal bindings with
positive and omission, duplication, reorder, coefficient, partial-sum,
cross-parity, and terminal-body mutation tests. Configure and prove both
fields with real keys, measure every source-authentication/slice/join phase
including key reload and handoff on target phones, and keep the 128 MiB
whole-process gate until the maximum observed RSS is below it.

## Acceptance tests and implementation order

The release gate requires the same candidate and signed provenance across
Core, native bridge, SDKs, phone apps and service. Run exact positive and
negative tests for enrollment substitution, wrong app, key, policy, release,
reader, stale predecessor, second successor, counter/journal rollback, nonce
reuse, changed-byte retry, torn commit, response loss, process kill, storage
full, epoch rotation, clock rollback and replayed credit. A checkpoint
device must show the **same original certificate** after post-commit restart.
A ratchet device must show exact-next non-forking and permanent fail-closed
behavior when the original assertion is lost. Both must refuse a second
spend from the old head. Emulator and host mocks do not satisfy this gate.

Implementation proceeds in this order:

1. Compile and test the native Core proof/candidate/recovery owner with
   genuine recursive proofs and the selected hardware authorization profile.
2. Implement the qualified phone anti-fork selection and session binding.
   The checkpoint profile requires an atomic current head and retained
   response; the ratchet profile requires exact-next one-use authority and
   fail-closed loss handling. Both require an attested app/key access rule.
   Keep production monetary admission unavailable without an eligible
   profile; the separate testnet experimental corridor remains available.
3. Integrate the asynchronous iPhone and Android wallet owners with persisted
   intent before transport, authenticated response before admission and
   checkpoint exact-request recovery or ratchet fail-closed freeze. Remove
   first-release aliases and fallback paths.
4. Verify cross-SDK canonical fixtures, four-validator finality and
   1,024 real linked handoffs/1,000 funded devices. Measure full-process
   128 MiB RSS and configured proving, verification, handoff, PK and VK caps.
5. Qualify each exact phone model/OS/firmware/service/release tuple with signed
   physical transcripts, app and service attestations, crash/rollback
   observations and independent security review. Only then enable monetary
   operations for that exact profile.

Current code contains the exact-next statement validation, formal invariants,
native/SDK transport and partial journal/service components. It does not yet
compose a production native owner with a provisioned phone service, real
funded proof corridor and signed device qualification. This distinction is a
release condition, not an alternate V1 algorithm.

The iPhone App Attest intent journal may move a completed assertion to the next
ready counter only after native coordinator method 13 acknowledges the exact
enrolled key, signed selection, original raw assertion, predecessor/next
counter, committed terminal certificate and installed envelope. The native
backend hook is unavailable by default; its future implementation must verify
those fields against the authenticated durable Core journal. The mobile CAS
and frame correlation do not themselves qualify a monetary commit.

The implementation gap is concrete: Core's structural exact-next checks and
recovery metadata exist, but its production hardware-guard hooks default to
rejection; the stock native device bridge is unavailable without a service
backend; the native coordinator backend is still test-only; and mobile
transport/SDK admission cannot create the missing hardware authority. The
real funded recursive corridor, resource caps, signed release provenance and
physical hardware evidence must close independently. A checkpoint lane must
preserve the original certificate across uncertainty and reject a host
journal whose prefix differs from the freshly selected hardware head. A
ratchet lane must reject any head whose next index cannot be authenticated,
including a locally cached earlier head after an uncertain assertion.
The current certificate decoder checks structure; it is not independent
hardware-signature verification. Test-injected proof verifiers likewise do
not qualify the concrete Eq/Ep relation. Production admission must use the
real certificate/Guard verifier and the signed app-to-device enrollment
binding, then demonstrate them in genuine linked proofs.

## Source and platform basis

- `specs/kagemusha_device_bridge_v1.md`: closed operation/capability ABI,
  exact-next and recovery obligations.
- `specs/kagemusha_v1.md`: aggregate-balance, recursion, peer-message and
  durable device state-machine algorithm.
- `specs/peer_transport_v1.md`: canonical peer bytes independent of QR, NFC,
  HCE and nearby transport choice.
- `formal/kagemusha_v1/KagemushaV1.tla`: ExactNextNonForking and crash/rotation
  model; the mutation harness produces a counterexample for a second successor.
- `crates/iroha_core/src/zk/kagemusha_v1_state/mod.rs`: hardware epoch and
  exact-next transition statement.
- [Apple CardSession](https://developer.apple.com/documentation/corenfc/cardsession)
  and [NFC & SE platform](https://developer.apple.com/support/nfc-se-platform/).
- [Apple App Attest validation](https://developer.apple.com/documentation/devicecheck/validating-apps-that-connect-to-your-server).
- [Apple Secure Element Credential access and transactions](https://developer.apple.com/documentation/secureelementcredential/accessing-and-using-secure-element-credentials).
- [Android hardware-enforced single-use keys](https://developer.android.com/reference/android/security/keystore/KeyGenParameterSpec.Builder)
  and [KeyMint attestation authorization lists](https://source.android.com/docs/security/features/keystore/attestation).
