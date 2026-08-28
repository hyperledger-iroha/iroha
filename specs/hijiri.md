# Hijiri V1 validation-fee policy

This specification defines the first-release Hijiri production surface. Hijiri
is a deterministic validation-fee multiplier owned by on-chain state. It is not
a peer-reputation or consensus-weighting system in V1.

## Authority and state

Hijiri has one consensus authority: custom parameters in the World State View.
There is no `[hijiri]` node configuration, environment override, or secondary
runtime policy source.

The state is split so admission never scans or decodes a ledger-wide risk
table:

- `HijiriParametersV1`, under the reserved identifier
  `iroha:hijiri_parameters_v1`, contains the bounded global fee bands, penalty
  cap, and default account risk. Its encoded payload is limited to 64 KiB and a
  policy contains at most 256 bands.
- At most one `HijiriAccountRiskV1` custom parameter applies to each universal
  `AccountId`. Its reserved identifier starts with
  `iroha:hijiri_account_risk_v1:` and includes a domain-separated hash of the
  canonical account identity. Each encoded record is limited to 4 KiB.

Both records declare V1, begin at revision 1, and form independent
digest-linked revision sequences. A successor increments its predecessor by
exactly one and includes the predecessor's canonical digest. An account-risk
successor cannot change its embedded `AccountId`.

Risk and multiplier values use unsigned Q16.16. Risk is in the inclusive range
`[0, 1]`. Fee bands have strictly increasing, non-zero upper bounds and the last
band ends at exactly `1`. Every multiplier and the penalty cap is at least `1`,
so Hijiri cannot discount the base validation fee; evaluation clamps a band to
the penalty cap.

First-release shipped genesis manifests install revision 1 with no predecessor,
one band ending at risk `1` with multiplier `1`, a penalty cap of `1`, and a
default account risk of `0`. This neutral bootstrap makes Hijiri state and its
signed quote binding explicit without changing the base validation-fee amount.

If a custom genesis omits the global parameter, Hijiri is inactive. If it is
present but an account-specific record is absent, admission uses the global
`default_account_risk`. An explicit account record whose value equals the
default remains distinct from an absent record for signed-state binding.

## Validation-fee admission

Admission resolves only the execution account involved in each fee context:

- the transaction authority for the top-level context; or
- the controlled account for a nested or deferred multisig context.

It derives the reserved account-risk parameter identifier for that account and
performs one exact lookup. It does not enumerate all Hijiri parameters and does
not consult a fraud-service score, node-local configuration, observer record,
or peer reputation.

For one execution context, let `n` be the number of qualifying validation-fee
transfers and let `f` be the configured base fee in minor units. Admission first
computes the aggregate base with checked integer arithmetic:

```text
base = checked_u64(n * f)
```

When Hijiri is active, it selects the band for the account's effective risk and
then rounds upward exactly once:

```text
required = ceil(base * multiplier_raw / 65536)
```

When Hijiri is inactive, `required = base`. Overflow, malformed policy state,
or a mismatched account-risk record fails admission. In particular, admission
does not round each qualifying transfer separately before aggregation.

## Signed state binding

A fee-bearing request commits to the exact Hijiri state used to quote it. The
composite fee-quote hash commits to:

- the canonical digest of `HijiriParametersV1`;
- the execution `AccountId`; and
- either the canonical digest of the matching `HijiriAccountRiskV1` or explicit
  absence of that record.

Transactions carry this value in signed metadata under
`validation_fee_hijiri_fee_quote_hash`. The metadata binds the execution
account selected by the explicit fee coordinate, which is the transaction
authority for an ordinary payment and the controlled account for a nested
multisig payment. A fee-bearing multisig proposal carries that same
context-specific value in its canonical validation-fee marker. The marker
encodes a present hash as lowercase 64-character hexadecimal and absence as
`-`.

When Hijiri is active, the binding must be present, well formed, and exactly
match current state. A missing, stale, substituted, or account-mismatched value
is rejected. When Hijiri is inactive, the binding must be absent at the top
level and the multisig marker must contain `-`; an extra binding is rejected.
These rules prevent a payer from selecting an uncommitted risk or policy
snapshot.

## Governance, queries, and events

`SetParameter` is the only update instruction. Outside genesis bootstrap, a
caller must hold the dedicated `CanSetHijiriParameters` permission; generic
parameter-writing authority does not grant control over validation fees.
Transition and payload validation run before the parameter enters state.

The neutral Hijiri bootstrap does not install or activate the protected base
validation-fee policy registry. That registry can only be created by an enacted
SORA Parliament validation-fee proposal, and its first policy becomes effective
after the mandatory 120,960-block activation delay. Until an enabled
exact-network base policy is active, the Hijiri quote route remains unavailable
even though the global Hijiri parameter is present.

Clients may still read raw Hijiri records through `FindParameters`, and updates
use the existing `ConfigurationEvent::Changed(ParameterChanged)` surface.

The production quote boundary is the canonical-account-authenticated native
Norito `POST /v1/validation-fee/hijiri/quote` route. Its V1 request is limited
to 4 KiB and contains the account to price plus a transfer count in
`1..=100_000`. The body account must either exactly equal the authenticated
principal or name a live multisig controller whose canonical specification
contains that principal as a direct signatory. Torii validates that membership
and reads the selected account-risk record from the same committed state
snapshot; non-members receive the same forbidden response as every other
cross-account request, before Torii derives or reads that account's risk key.
From the snapshot at height `h`, Torii selects the exact-network enabled
validation-fee policy scheduled for checked height `h + 1`, reads the global
Hijiri parameter and at most one authorized account-risk parameter, and returns
a response that clients bound to 64 KiB. The response includes the policy
version/hash, fee asset and treasury, Hijiri record digests, selected Q16
multiplier, composite quote hash, and exact per-transfer and aggregate
minor-unit amounts.

The response assurance is
`EVALUATED_PROJECTION_NOT_INDEPENDENTLY_WITNESS_VERIFIED`: the route is an
authenticated same-snapshot evaluation, not an independent state proof. A
client must copy the returned policy version, policy hash, and composite Hijiri
quote hash into the signed validation-fee metadata (or nested multisig marker)
and pay the returned aggregate fee at the exact fee coordinate. If policy or
Hijiri state changes before inclusion, admission rejects the stale binding and
the client must request a new quote. The existing finality-bound current-policy
proof can independently verify the base policy, but it does not witness custom
Hijiri parameters. The Nexus `/v1/fees/quote` route is a separate fee surface
and does not construct this validation-fee payment.

Rust, C#, Kotlin, Java, Swift, JavaScript, and Python expose the same typed V1
quote boundary without a JSON wire fallback. The non-Rust SDKs delegate request
encoding and canonical response decoding, re-encoding, coherence checks, and
exact-request binding to the ABI-23 native Norito implementation through the C,
JNI, N-API, or PyO3 bridge appropriate to that SDK. Their transport wrappers
reject non-HTTPS origins before native encoding, pin the signed and final URL,
deny redirects and transparent decompression, require the native media type and
identity representation, require unqualified `private` and `no-store`
directives on every status, and reject any `public` directive including a
parameterized form. They reject a success carrying a reject-code header and
bound the actual request and response bytes to 4 KiB and 64 KiB respectively.
Where a response declares `Content-Length`, clients require one canonical value
equal to the bytes they actually consumed. ABI-23 artifact manifests, Apple
slice inventories, JVM JNI checks, and managed-package consumer smokes require
the corresponding Hijiri quote exports; an artifact that reports ABI 23 but
omits the additive quote surface is not a valid SDK artifact.

## Deferred surfaces

The repository contains portable Hijiri observer, evidence, and incentive data
types, but V1 does not give them a production ingestion or state-transition
owner. The following remain deferred and must not be described as active:

- authenticated observer or evidence ingestion and attestation validation;
- an independent witness proof over both base-policy and Hijiri custom-parameter
  state;
- peer reputation or any effect on consensus membership, voting, or topology;
- registry credits or settlement;
- Hijiri-specific checkpoints; and
- dedicated Hijiri events, metrics, dashboards, or telemetry.

Any future evidence ingress must first define authenticated provenance,
canonical path semantics, bounded collections, deterministic state ownership,
and adversarial integration coverage.

## Source ownership

The canonical data model and hashing rules live in
`crates/iroha_data_model/src/hijiri/mod.rs`. Validation-fee resolution and
rounding live in `crates/iroha_core/src/validation_fee.rs`. Parameter transition
validation lives in `crates/iroha_core/src/smartcontracts/isi/world.rs`, and
authorization lives in both the initial and deployed default executors with its
permission type in `crates/iroha_executor_data_model/src/permission.rs`. The
native quote DTO and validation contract live in
`crates/iroha_torii_shared/src/validation_fee_api.rs`; Torii's same-snapshot
handler lives in `crates/iroha_torii/src/validation_fee_api.rs`. The ABI-23 C
and JNI projection is owned by `crates/connect_norito_bridge`, with the N-API
and PyO3 projections in `crates/iroha_js_host` and
`python/iroha_python/iroha_python_rs` respectively; each SDK owns only its
typed transport and projection surface above that native contract.
