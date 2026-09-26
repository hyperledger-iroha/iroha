# Native retail enrollment phases

This kernel is compiled for qualified Rust backends. The generic C/JNI bridge
exposes the bounded method-12 initial-enrollment phase frames, but no production
backend is installed. These phases alone do not qualify an iPhone or Android
device, an issuer configuration, a native release artifact, or an offline
monetary operation. The method-12 backend hook defaults to `Unavailable`.

The phase-1 journal selection creates one continuous, suspend-inclusive deadline,
nonzero ticket, client nonce and lane before HTTP. Its ticket is an opaque local
selector, not issuer or monetary authority. The qualified backend must retain that
exact live selection across App Attest/KeyMint preparation and the one-use device
qualification. It must never issue a second key, native nonce or qualification after
an ambiguous result. Phase 7 reads the byte-identical original phase-1 response
through the same live native owner after a lost return. It checks the original
account, journal and deadline and never selects a new nonce. A restart or
uncertain journal result freezes the attempt.

The phase-1 response has seven fields: ticket, nonce, release ID, profile ID,
lane ID, native owner scope, and fixed native deadline in continuous milliseconds.
The owner scope is a digest of the original selected account, ticket, lane and
independently pinned policies for app cache isolation; it is not the eventual
retail enrollment ID. The deadline uses the platform's suspend-inclusive boot
clock, never Unix time. Apps may reject an expired response early, while every
native phase still checks the original process-owned deadline itself.

`KagemushaEnrollmentPhaseOneBackendV1` is a Rust-only adapter for this selection
and bounded challenge, proof and certificate dispatch. It requires an independently
qualified coordinator delegate, an authenticated rollback-checked journal store, exact storage path and
governed pins. It consumes the phase-1 attempt before durable I/O. A repeated open
revokes the first handle and its selected journal ticket; close also freezes the
journal. Phases 2–5 require a separate Rust-only
`KagemushaQualifiedEnrollmentDelegateV1`; the ordinary coordinator delegate is
never treated as an enrollment verifier. With that provider, phase 2 checks the
original ticket, policy pins and deadline, persists one challenge intent, passes
the original process-local live selection and its governed pins to the provider,
and retains the kernel's `AcceptedIssuerChallengeV1` only after publishing the
derived response durably. Phase 3 persists one proof intent before consuming that
exact accepted challenge, validates the kernel's `PreparedIssuerProofV1` against
the original challenge ID and publishes its canonical proof before retaining it.
Phase 4 reads only the original durable proof; it never asks the provider to sign
again. Phase 5 consumes that prepared proof against one issuer certificate, publishes
the resulting enrollment ID durably, and exposes the `FreshIssuerAdmissionV1`
exactly once to trusted Rust code for the separate wallet-open ceremony. Phase 6
freezes the selected ticket and drops every remaining typed state; an exact retry
of a successful cancellation returns the same empty response. Exact challenge,
proof and certificate retries return retained bytes without a second provider call. An
uncertain or invalid result leaves its intent frozen. The phase-2 frame does not
carry raw platform evidence or independent policy/release objects: the provider
must retain and authenticate them and consume the enrollment kernel using the
supplied live selection. This adapter does not verify those inputs itself and does
not install itself. The accepted challenge, prepared proof and fresh admission are
process-local consuming types. A restart may recover exact journal response bytes
for diagnosis, but it cannot recreate these types, the original continuous deadline,
or permission to resume enrollment from app bytes. An uncertain publication retains
no new typed state in the adapter.

`KagemushaKernelEnrollmentDelegateV1` implements the consuming phase-2/3 handoff
for a Rust-only `KagemushaEnrollmentContextProviderV1`. That provider must return
the independently governed owner, policies, authenticated release, native key,
original raw platform evidence and trusted service time for the original live
ticket. The delegate accepts the exact signed app certificate from the phase-2
frame only after `begin_selected` binds it to that evidence. No context provider
or global backend installer is supplied by the ordinary bridge.

`PendingIssuerEnrollmentV1::begin_selected` consumes the original live selection
after verifying the issuer-signed 273-byte preparation, the independent verifier's
canonical signed app certificate, the full raw evidence digest and the canonical
qualification. Raw evidence is bounded to 128 KiB to admit the verifier's Android
certificate-chain envelope; it is retained by the trusted Rust context provider,
not carried as a coordinator frame field. The signed certificate's
`attested_key_id` is SHA-256 of the raw platform-attested SEC1 point. For Apple it
must equal the provisional App Attest key ID signed in preparation; for Android the
preparation carries the zero sentinel
and the later certificate signs the actual KeyMint point ID. The governed device
reference must derive from that same point. The qualified backend supplies independently
authenticated issuer/release pins, the selected owner, trusted service time and native
Core authorization key. The qualification body is checked against the authenticated
release's complete enabled profile, governed credential, owner network/lane and native
key reference. It selects the expected device; actual device possession is
established in a later phase.

The consuming lifecycle is:

1. Read `client_nonce` and `canonical_qualification` for the issuer start request.
   Repeated reads return the original values and never reset the deadline.
2. `accept_challenge_with_certificate` authenticates the phase-2 certificate under
   the pinned app policy and trusted service time, then canonically decodes the
   issuer challenge, checks all retained owner/issuer/release/credential fields,
   and derives the account signing message,
   device request identity and sole operation-1 command locally. Both HTTP IDs,
   signing message, command and expiry projections must match those derivations.
   The returned `AcceptedIssuerChallengeV1` is only a checked signing request.
3. `prepare_proof` accepts exactly one raw 64-byte account signature and a bounded
   complete device frame. The delegate first checks that the live journal is the
   same process-local attempt retained by the accepted challenge. It verifies
   the typed account signing purpose, command,
   operation, complete challenge identity, device key, report digest and exact
   returned qualification. It retains the sole canonical possession proof.
4. Read that same proof and challenge ID for finish retries.
   `PreparedIssuerProofV1::complete` accepts only the issuer certificate. The host
   cannot replace the proof at completion. Issuer evidence authenticates all
   three signatures against the original native owner, nonce and release.

Phases 2–5 check the original native deadline and live journal ticket on every
read and transition; phase 6 can revoke an expired ticket. Before certificate
authentication, timestamps are checked only for valid interval relationships;
neither an unsigned challenge time nor a host wall clock becomes trusted UTC.
Only the issuer's signed historical decision instant is used for historical
challenge validity. Admission still grants no hardware commit clock or monetary
authority. Consuming types have no clone/deserialize/restart constructor.

The phase tests cover governed signature substitutions, independent HTTP
projection mismatches, same-client-nonce challenge replay, account signing
purpose, exact retry bytes, certificate commitment and expiry. A separate
phase-adapter test takes the real consuming delegate through challenge, proof,
certificate, one-use admission and cancellation against one journal selection.
These tests use explicit test keys and the real data-model verification functions.
Reuse the warm target
for the maintained focused suite:

```sh
cargo iroha-fast --target-slot kagemusha-v1 --stable-local-metadata --incremental -- test -p connect_norito_bridge --lib kagemusha_core_coordinator_v1::initial_enrollment::tests
```

Production integration must supply a qualified context provider, install the
phase adapter behind the native owner, and hand verified phase-5 admission into
the separate enrolled-wallet open under the same bounded, revocable registry
handle and authenticated journal. The provider must independently retain raw
platform evidence, authenticated issuer configuration, release and policy pins,
and trusted service time. Exact current-source native artifacts are also required.
The generic bridge supplies no C/JNI installer for an unqualified backend. Device
setup, funding/finality, crash recovery and physical hardware qualification
remain separate required outcomes.
