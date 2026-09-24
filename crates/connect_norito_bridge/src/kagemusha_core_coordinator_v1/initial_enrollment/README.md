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
an ambiguous result. A restart or uncertain journal result freezes the attempt.

`PendingIssuerEnrollmentV1::begin_selected` consumes the original live selection
after verifying the issuer-signed 273-byte preparation, the independent verifier's
canonical signed app certificate, the full raw evidence digest and the canonical
qualification. The signed certificate's `attested_key_id` is SHA-256 of the raw
platform-attested SEC1 point. For Apple it must equal the provisional App Attest
key ID signed in preparation; for Android the preparation carries the zero sentinel
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
2. `accept_challenge` canonically decodes the issuer challenge, checks all retained
   owner/issuer/release/credential fields, and derives the account signing message,
   device request identity and sole operation-1 command locally. Both HTTP IDs,
   signing message, command and expiry projections must match those derivations.
   The returned `AcceptedIssuerChallengeV1` is only a checked signing request.
3. `prepare_proof` accepts exactly one raw 64-byte account signature and a bounded
   complete device frame. It verifies the typed account signing purpose, command,
   operation, complete challenge identity, device key, report digest and exact
   returned qualification. It retains the sole canonical possession proof.
4. Read that same proof and challenge ID for finish retries.
   `PreparedIssuerProofV1::complete` accepts only the issuer certificate. The host
   cannot replace the proof at completion. Issuer evidence authenticates all
   three signatures against the original native owner, nonce and release.

Every read and transition checks the original native deadline and live journal
ticket. Before certificate
authentication, timestamps are checked only for valid interval relationships;
neither an unsigned challenge time nor a host wall clock becomes trusted UTC.
Only the issuer's signed historical decision instant is used for historical
challenge validity. Admission still grants no hardware commit clock or monetary
authority. Consuming types have no clone/deserialize/restart constructor.

The 2026-09-12 phase tests cover validly governed and signed substitutions,
independent HTTP projection mismatches, same-client-nonce challenge replay,
account signing purpose, exact retry bytes, certificate commitment and expiry.
They use explicit test keys and the real data-model verification functions.
At source application, formatting and source review are complete; these new Rust
tests have **not been compiled or executed** because the shared native build lane
is occupied by Taira incident validation. Execute the focused maintained suite
once that lane is available, reusing its warm target:

```sh
cargo iroha-fast -- test -p connect_norito_bridge --lib kagemusha_core_coordinator_v1::initial_enrollment::tests
```

Production integration must still connect these consuming states to bounded,
revocable native registry handles, the authenticated enrollment attempt journal,
the real durable coordinator backend and Swift/Kotlin lifecycle APIs. The
backend must independently verify issuer-signed preparation and raw app
attestation, install authenticated issuer configuration, and retain exact
current-source native artifacts. The generic bridge supplies no C/JNI installer
for an unqualified backend. Device setup, funding/finality, crash recovery and
physical hardware qualification remain separate required outcomes.
