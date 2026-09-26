# SoraFS V1 stream-token signer custody contract

The stream-token role has one provider-scoped signing contract. Operators choose
software custody or an optional hardware adapter; both use the same authenticated
Ed25519 protocol. Configuration, opaque key operations and independently signed
state evidence have separate owners. Public handles and signatures alone do not
establish current authorization or deployment readiness. The protocol makes no
claim about a participant's server hardware or private-key exportability.

## Configuration and runtime ownership

The complete [public-pin template](snippets/stream_token_signer_binding.toml)
contains the required table hierarchy. Its placeholders and zero generations or
intervals are deliberately invalid: replace every reviewed public pin before
admission. No signing-seed file, key path, or environment enablement is accepted.
Credentials, private keys and any optional device sessions or PINs remain with the
configured runtime provider. Public configuration identifies the authorized key
and its scope without prescribing the operator's key-storage implementation.

- `sorafs.storage.stream_tokens.enabled` defaults to false. Disabled issuance
  rejects configured signer pins and injected signer, observer or approved
  anchor dependencies. Enabled issuance also requires storage, its nonzero
  `provider_id_hex`, operator-signature authentication and the existing governed
  admission capture.
- `signer` is all-or-nothing: runtime/key handles, signer service/administrator,
  strong Ed25519 key, key/policy revisions and policy digest, plus independently
  configured `attester` and `observer` tables. All six service/administrator
  identities and all three public keys must be distinct. Signer `key_revision`
  is the sole generation and must fit `1..=u32::MAX`; the token's
  `token_pk_version` is its checked conversion, not another configuration value.
- Provider identity comes from storage; chain and genesis-derived network identity
  come from the enclosing authenticated node context. They are not response-selected
  or duplicated inside `signer`. Handles are credential-free identifiers;
  `software`, `signer`, `hsm`, `kms` and `pkcs11` schemes use the same binding
  checks. A provider label is never authorization. Observer routing is independent
  and permits software implementations under the same public trust checks.
- Custody lifetime and anchor-age limits are positive and at most 86,400,000 ms.
  Observer age/lifetime is positive and at most 300,000 ms. Eligibility intervals
  have exclusive ends and are evaluated against a caller-owned clock at runtime.

`iroha_config::parameters::{user,actual}` own configuration. Torii's
`StreamTokenSignerPinsV1::from_config` derives the full immutable configuration
digest and canonical Manifest binding. `ToriiRuntimeDeps` receives
`with_sorafs_stream_token_signer_client`,
`with_sorafs_stream_token_state_observer` and
`with_sorafs_stream_token_approved_anchor`. The approved full custody anchor is an
independent deployment input bound to that complete digest; neither client may
bootstrap it from its own response. Registry slot 11 carries the one capability
and its two separately injected transports.

## Native custody authority

`MutateSorafsStreamTokenCustody` owns the provider's governed public signer binding,
independent attester trust, enrollment sequence and predecessor, active signed
enrollment digest, and terminal signer/attester revocation flags. Its dedicated
provider-scoped `CanManageSorafsStreamTokenCustody` permission governs configuration,
enrollment and revocation. Expected revision and digest bind each atomic mutation;
immutable history retains the executing height, deterministic ordinal and authority.
This history excludes token bodies, signatures, reservations and the operation
journal. Those remain with their existing durable owners.

The action uses one tagged JSON form, `{"action":"revoke","value":{"signer":true,
"attester":false}}`, and a typed revocation payload carrying both flags. Configure
and enroll carry their exact Manifest-owned frames; the native DTO does not
redefine those signed payloads.
The shared `signer::custody_control::SignerCustodyPolicyV1` and
`SignerCustodyControlStateV1` own the common binding, attester trust and enrollment
shape. StreamToken native admission and historical readers additionally require
role 11 and the exact provider purpose; a valid deployment-approval policy cannot
enter this provider-scoped authority.

Enrollment verifies the complete canonical signer attestation against the
previous committed control snapshot and that block's actual identity and time.
A policy change in the current block cannot be presented as already committed
under its predecessor's block hash. Genuine policy changes clear the active head
and require enrollment; the enrollment sequence and predecessor never reset.
Revocation cannot be cleared by relabelling a retired key or changing policy within
the same key generation. Same-key renewal uses a new attested enrollment.

Each provider has a total limit of 8,194 control-history records. Ordinary changes
may consume the first 8,192; the last two records are reserved for monotonic
revocation. Earlier revocations count against the same total. History is not pruned
to admit more changes. This bound limits lifetime control changes and must be
considered during deployment planning.

Core's `read_stream_token_custody_control_at_v1` reads the exact provider control
snapshot at a requested committed height from one `StateReadOnly` view. It checks
the canonical binding, chain and genesis-derived network, bounded history/index
coherence, and derives the custody anchor from native state and the same view's
block identity. Torii requires this anchor to match the signed observer claim and
compares the complete active head, both revocation flags, and independently pinned
attester trust. The independent approved anchor remains a required deployment input.
Neither observer signatures nor a nonzero state digest substitute for this native
association.

Historical control records remain readable when a provider is removed from the
current registry. They do not confer current serving authority. Torii separately
requires the pinned provider to remain registered in the same State view used for
each capture and validation; native mutations and exact retries also require
current provider registration and permission.

## One-body issuance and recovery

The Manifest owner authenticates independent signer attestation and signed,
challenge-bound current/completed observations. Torii retains its exact prepared
body, original custody and authenticated-operator quota across the operation.

1. Startup requires a fresh `CurrentCustody` observation with phase `Startup`.
   It contains no invented token or completed row.
2. Each issuance has a separate fresh `BeforeProvider` observation. The service
   derives the same operation from its own pinned provider binding and exact
   canonical body before reading state or calling the signer.
3. Torii makes one `Sign`. An ambiguous result permits at most one read-only
   recovery of that same operation, with no new reservation, commit or signing.
   Consumed issuance quota is not refunded. The echoed HTTP nonce does not create
   idempotency across separate HTTP requests.
4. The bounded receipt contains exactly four ordered signatures for role payload,
   audit record, provenance and response. Fresh `AfterCommit` and then separate
   `BeforeRelease` observations authenticate the original custody and exact durable
   completion. Startup or pre-sign evidence cannot replace either observation.
5. Before publication, Torii checks approved, current and historical signing custody
   anchors against the native control reader and Kura's verified finality artifacts
   using one immutable Core State view. Completion keeps its separate finalized
   operation-block identity; it is not interpreted as custody control state. Larger
   heights alone prove no ancestry. Torii resamples time after finality reads and
   rejects expiry, rollback, revocation or custody drift.

The signer client and broker return only bounded untrusted receipt bytes.
`SignerStreamTokenServiceV1` owns durable production and read-only recovery over
`SignerOperationCoordinatorV1` and the StreamToken-purpose `SignerReceiptJournalV1`.
Private files, exclusive reservation/CAS, staged receipt checks and pre/post-I/O
state fencing precede release; the transport never creates a verified authority.
The coordinator rejects a generic role-11 Sign reservation. Its stream-token
entry point checks the exact canonical body against the borrowed domain-prefixed
signing bytes and passes a sealed body, time-window and request-digest review to
the source's purpose-specific Reserve. The source must independently rederive
that review and perform the finalized CAS; the current test source exercises this
boundary, but a production finalized operation source is still required.

The token signature covers `sorafs.stream-token.signature.v1\0` followed by the
canonical body frame. The token transport is canonical padded base64 over one
canonical Norito V1 frame, capped at 2048 decoded bytes and 4096 header bytes.
Same-value alternate layouts and compression are rejected; finite decoder limits
also intersect any stricter caller budget.

## Current custody before serving admission

Every new CAR-range or chunk admission authenticates a separately challenged
`CurrentCustody` observation with the current-only `BeforeAdmission` phase. Static
canonical token, signature, key generation, provider, manifest, profile and initial
issue/expiry checks precede observer I/O. The driver authenticates current custody
and repeats local finalized-history checks, then resamples its monotonic trusted
time and checks token issue/expiry again before any accepted quota transaction.
The exact final millisecond becomes the accepted quota and reputation-callback
timestamp; that qualification is never cached for another request.

Revoked, stale, unavailable or substituted authority evidence fails closed with
HTTP `503`. The typed `Excluded(SignerAuthorityUnavailable)` callback retains the
decoded body digest and key generation without counting a provider observation or
violation or obtaining an accepted quota lease. Its failure timestamp is the initial
request observation, since a failed or rolled-back clock cannot establish a trusted
final admission time. If the durable audit provider also fails, no terminal outcome
is fabricated. An already-expired token is rejected before observer I/O; expiry while
waiting on observer/finality work is an infrastructure exclusion.

This is a logical cutoff for new admissions using freshly authenticated evidence
and the node's known finalized history. It neither proves globally latest custody
state nor cancels packets from a previously admitted stream. Same-key custody
renewal does not revoke existing token signatures. Governed revocation must be
terminal for the revoked key generation; reactivation with the same key/version
would otherwise revive earlier tokens. Native control transitions enforce terminal
generation revocation, and every current observation must agree with that state.

## Cutover and qualification

Generate a replacement key with the configured provider, obtain independent
attestation and governed activation, and atomically update the reviewed signer
pins, approved full custody anchor and authenticated provider inventory. A
provider descriptor pins one key; switch its `gateway-key` and token together.
An in-flight operation cannot be retried or relabelled under renewed custody.
Finalize the old custody's terminal audit/revocation transition before revocation
takes effect; no post-revocation signing or silent multi-key fallback is allowed.
Retain public fingerprints, generations, policy/record/completion digests and
approval evidence, never credentials or key material.

TODO: complete coherent native caller/broker/producer validation and qualify the
configured signer provider, attester, current finalized-state observer and
authoritative durable operation source. Validate the native custody transitions and
Torii reader together against durable certified history and four voting validators
with mandatory signed RS16 DA/RBC. The present source contracts and signed
simulations do not complete those qualification outcomes.
