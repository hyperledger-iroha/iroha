# SoraFS current record before the receipt-Check observer checkpoint

Historical source-scoped evidence, not current release readiness. The preceding
identity/envelope/age checkpoint remains verbatim in the [closure ledger](../../../specs/sorafs/v1_closure_ledger.md#identity-envelope-and-retained-age-checkpoint).
These exact displaced current-record excerpts are preserved once.

## status.md

Source SHA256: `198b9499b7caff66ac5a6b880de19dae20db9e4c9a682c96ce7c0bfbee8b3143`.
Excerpt SHA256: `7e15cad98349ab68645fdcd3b4c4b0c46d48164e8616e77c7bfed58eff97841b`.

```markdown
SoraFS follows the [V1 implementation goals](specs/sorafs/v1_implementation_goals.md)
and the [identity, envelope and retained-age checkpoint](specs/sorafs/v1_closure_ledger.md#identity-envelope-and-retained-age-checkpoint).
The current local checkpoint records **6,984 passes**, no failures and exactly six
existing DataModel manual fixture printers ignored. It combines ten unchanged
successful target runs with complete environment-corrected configuration reruns;
all 8,190 captured inputs, twelve binaries and 112 controls remain unchanged.
All 27 added regressions and the 75 mandatory native CI sentinels are included.
**1,094 release/CI contracts**, formatting and the codec guard pass. The global
source-budget guard still fails with the same 241 findings and no new offender.

Production identities now use one exact-component grammar, allowing `attester`
while rejecting reserved non-production components. Core's structural account
preflight enforces the sole 64-KiB signed External bound without granting signing
or spending authority. Both custody consumers retain the original earliest
observation time across initial endpoints and later eligibility rechecks; neither
the authenticated snapshot nor the monotonic deadline is renewed.
Actual prepared account signing, non-exportable hardware, independently qualified
observer/UTC/floor authority and daemon submission/state/config integration remain
open. Workspace/SDK/strict-lint/security, the four-validator/provider/gateway
reference deployment, load/24-hour soak, all seventeen lanes and the foundational
envelope still require qualification. No goal or lane is closed.
The prior account/schema and separate Kagami prototype results keep their original
source scopes in the ledger and [displaced current excerpts](docs/history/2026-09-13/sorafs-before-identity-envelope-age.md).

```

## specs/sorafs/v1_implementation_goals.md

Source SHA256: `13b083c7acfa571fabbcc408047ebffc36068e3841eeafa707689df9a959d58b`.
Excerpt SHA256: `c311eb984b84c81d4a67463e6f2582709f233b0d722580db4045ec4a1dde4135`.

```markdown
## G02 checkpoint — 2026-09-13

The [identity, envelope and retained-age checkpoint](v1_closure_ledger.md#identity-envelope-and-retained-age-checkpoint)
records **6,984 passes**, no failures and six existing DataModel manual fixture
printers ignored. Ten successful target runs retain their exact source/binary
provenance; the two complete configuration targets pass after correcting the
runner's Cargo environment. All 8,190 captured inputs, twelve binaries and 112
controls remain unchanged. All 27 added regressions and 75 native CI sentinels
are included; **1,094 release/CI contract tests** also pass.

One canonical identity grammar now rejects exact reserved components without
rejecting real words such as `attester`. Core exposes a structural account-envelope
preflight through its sole complete signed-External size owner. Both account and
receipt custody consumers retain the original earliest observation time across
both initial endpoints and later eligibility checks, without renewing their
snapshot or original monotonic deadline. The existing role15 native custody,
distinct permissions and shared exact Check proof owner remain in force.

Actual prepared account signing, non-exportable hardware, independently qualified
observer/UTC/floor authority and daemon submission/state/config integration remain
open. Local checks cannot prevent revocation racing an in-flight key call.
Four inner hardware approvals, all seventeen genuine lanes, the foundational
envelope, four-validator/provider/gateway deployment, load/24-hour soak,
workspace/SDK/strict-lint/security and full release qualification remain required.
No goal or lane is closed. Earlier account/schema and Kagami prototype results
retain their separate source scopes; the [displaced excerpts](../../docs/history/2026-09-13/sorafs-before-identity-envelope-age.md)
preserve the previous current record.

```
