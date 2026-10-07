# Enrollment eligibility authority and middleware contract

Any compatible fungible token can opt into KAGEMUSHA through its authorized asset/scheme
operator. Bank-required enrollment uses the user's bank middleware for current KYC approval
and freeze status. Other schemes may use their issuer or another authorized operator. SORA
Parliament is one optional provider for tokens it governs, not a protocol prerequisite.
Explicit public-enrollment policy is permitted; missing or unavailable bank data never selects
it as a fallback. This authority selection does not change payment proofs, hardware enrollment,
irreversible Send, reserve accounting or controls.

`iroha_data_model::kagemusha::enrollment_eligibility_v1` defines one canonical Norito protocol.
`iroha::kagemusha_enrollment` and Kotlin/JVM's
`org.hyperledger.iroha.sdk.offline.KagemushaEnrollmentEligibilityV1` provide middleware adapters,
including Java-callable lookup, clock and signer interfaces. The bank supplies its existing
current-state lookup and signing callback. It can use authenticated software signing custody;
the optional convenience signer accepts an existing Ed25519 key pair. The adapter generates
no keys and neither stores nor logs secrets or KYC documents.

## Current authority selection

The issuer must independently obtain the current policy from its authenticated configuration
and check the actual ledger asset/scheme registration and rooted Enrollment-role certificate.
The policy binds genesis network, scheme, exact asset incarnation/scale, positive revision,
bank FI identity or scheme operator identity, one strong Ed25519 middleware key and a positive
response-time bound. Both authority kinds must match the exact selected enrollment route.
Bank middleware must additionally check the subject's actual bank/account relationship.
Neither token classification nor Parliament membership is a wire-format authority arm.
There is no public caller-selected key or fallback to a different provider after a failure.

The policy type is DATA: construction, validation, self-consistent hashing and a matching
signature do not authenticate operator authorization, account routing or current
revocation status. This policy is separate from the platform enrollment policy, so it changes
neither E1 nor credential/circuit bytes. Its Ed25519 key authenticates a middleware observation;
only the existing scheme-rooted Enrollment-role P-256 signer can issue a credential.

## Fresh requests and exact responses

Before every pre-key permit (including Resume), E5 verification selection, credential-body
signing selection and E6 delivery (including recovery), the issuer retains a new CSPRNG nonce
and the exact request. The request binds the complete authority-policy digest, canonical
domainless account digest, authenticated actor digest, unchanged enrollment attempt, exact
operation-original digest, closed purpose and trusted creation/exclusive expiry times.
The lifetime cannot exceed the policy's response-time bound. Pre-key and new E5-verification
requests must also remain within the original E1 deadline. Already verified evidence and
exact retained E6 recovery use a new current eligibility interval without restarting E1 or
pretending to perform fresh platform verification; E1 expiry cannot erase a retained result.
The service enforces this phase-dependent deadline; the wire verifier checks the response-time
bound. Exact operation-original selection belongs to the service, not the SDK
caller or mobile app. An eligibility check cannot reset E1 or replace a retained E6.

A retained E5 in the `Verifying` phase may still name an unclaimed worker preparation.
Live recovery that can claim it therefore consumes its own fresh `VerifyEvidence` observation
within E1, bound to the exact retained request. After E1 expiry, the issuer may only use the
private worker's passive `Inspect` action: return the exact retained result, report an unknown
claimed outcome or report unavailable without starting verification. Inspect creates no
credential authority; signing and E6 delivery keep their separate current eligibility checks.

Middleware authenticates the calling issuer and resolves actor/account/provider binding using
its current authority records (the bank's customer directory for the bank route). It reads
approval and freeze status atomically from current source records. An operator's explicitly
public-enrollment policy still binds the authenticated account, scheme and current policy;
it is not permission to issue arbitrary monetary value.
Unknown subjects, missing records, failed authentication and unavailable stores produce an
error. A source observation carries a positive audit revision; revision alone permits no cache
hit. Frozen takes precedence over approved. The SDK reads once per request, checks genuine
time before and after the lookup and after signing, and rejects rollback or expiry. The
callback supplies the actual current-read time (not a record's last-update time), bounded by
the before/after samples. A slow callback cannot relabel an earlier observation as later.

The signed response binds the full request digest, one definitive decision (`ApprovedUnfrozen`,
`NotApproved`, `Frozen`), positive source revision, observation time and exclusive validity
deadline. Observation must be at or after request creation and not in the future; validity may
not extend the request. The SDK verifies the actual signer output under the selected key
before returning canonical bytes. Errors and denials cannot become approval.

The issuer rechecks current authority/routing after the response, verifies its exact retained
request and current time, retains the original response and consumes the nonce at the named
durable boundary. Repeated delivery requires a fresh observation, while returning the original
E6 byte for byte. Eligibility cannot promise that a bank will never freeze an account after an
observation; it is checked at each named online enrollment boundary. It adds no online call to
offline Send/Receive.

## Encoding

Every policy, request, response body and response has a unique first-release Norito schema.
Complete policy/request/response frames are capped at 2,048 bytes before decoding under
payload-derived resource limits. Enum discriminants are explicit positive indices; unknown
arms and noncanonical frames reject. There are no extension maps, old-layout decoders or
signature algorithm negotiation. Current Rust measurements are 236-byte policy, 264-byte
request and 174-byte signed response frames, for both authority arms and every purpose.

Identities and the signing message are SHA-256 over `domain || LE64(frame length) || frame`,
where `frame` is the complete canonical Norito frame and the domains are respectively:

- Policy: `iroha:kagemusha:eligibility:policy:v1\0`.
- Request: `iroha:kagemusha:eligibility:request:v1\0`.
- Response signing message: `iroha:kagemusha:eligibility:response:v1\0` over the response body.

The response signature is canonical Ed25519 over its 32-byte signing message. No payment
Poseidon domain, signer certificate role, proof descriptor or proof bytes change.

## Component evidence and remaining integration

The actual Rust generator emits 36 public DATA cases across the three allowed authority/class
combinations, four purposes and three decisions in
[`enrollment_eligibility_v1_vectors.json`](../fixtures/kagemusha/enrollment_eligibility_v1_vectors.json).
All canonical frames, policy/request digests, response signing messages and signatures match
Kotlin/JVM byte for byte. The captured component executables pass 11 model tests (one explicit
maintenance generator ignored), eight Rust middleware cases and nine Kotlin/Java cases. Tests
include current freezes, denied/unavailable sources, wrong keys, rotated policy, changed
account/actor/attempt/operation, expiry, time rollback and a slow source read. Test keys and
policies are unadmitted DATA. These results establish neither an actual bank integration nor
current authority installation or a release candidate's complete source closure.

The node enrollment journal retains each complete request, selected Scheme/policy, signed
response and consumption timestamp in its single atomic attempt record. The index is append-only
under the existing 3 MiB record bound: there is no separate nonce file to lose and recreate, no
eviction and no arbitrary retry-count cap. A consumed nonce cannot succeed again after reopening.
Attempt phase changes invalidate earlier observations; current policy/routing must still be
selected independently at consumption. A crash after consumption requires a fresh observation.
The copied Core executable passes 36 journal cases (23 existing and 13 eligibility cases), with
unchanged journal sources and temporary files confined to this checkout. Broader concurrent
source changes keep this evidence scoped to the journal component. Local atomicity does not
establish protection against arbitrary rollback of the entire issuer store.

The wire verifier and middleware adapter do not establish a serving issuer. TODO: qualify the
configured provider's transport and authority selection through the journal's durable
eligibility operations and the node enrollment owner. Qualify revocation during lookup,
unavailable provider state, replay/restart, freeze between enrollment steps and exact E6
recovery. A configured operator claiming to represent Parliament must have that authorization;
the claim itself grants no privilege and is not required for other token operators.
No real bank or Parliament authority is bundled in fixtures or selected by default.
