# SoraFS current record before identity, envelope and retained-age validation

Historical source-scoped evidence, not current release readiness. The previous
account checkpoint remains verbatim in the [closure ledger](../../../specs/sorafs/v1_closure_ledger.md#native-account-custody-and-shared-check-checkpoint).
These are the exact displaced current-record excerpts, preserved once.

## status.md

Source SHA256: `0126b46a15799886c9145a35a1b6d4844ebb9a9e8440a72c3923a964ef91a4b5`.
Excerpt SHA256: `57878235a6eed43f6e606e41b8a8092f5c3ad059101523d7206fbdedb0a1501e`.

```markdown
SoraFS follows the [V1 implementation goals](specs/sorafs/v1_implementation_goals.md)
and the [account-custody checkpoint](specs/sorafs/v1_closure_ledger.md#native-account-custody-and-shared-check-checkpoint).
The account-custody checkpoint passes **5,063 tests** across the complete
Manifest, DataModel, ExecutorDataModel, Executor and SchemaGen libraries; exactly
six named DataModel manual fixture printers remain ignored. Another
**714 native tests** (155 Core, 70 Torii, 487 daemon and 2 SCCP) pass with no ignores.
All 8,181 captured inputs, selected binaries, 42 controls and
build/baseline bytes remain unchanged; CI selects the same native names and all
75 mandatory sentinels. **1,094 release/CI contracts** pass.
Role15 now has distinct deployment custody/CAS and an independently submitted
Current Check. One private proof owner authenticates exact signed External bytes,
successful aligned results and the same applied cut for both custody purposes.
Both UTC endpoints must follow native enrollment and satisfy current authorization.
Formatting and codec checks pass; the existing 241 source-budget findings remain.
The schema reference has 1,719 descriptors and three byte-identical runs.
Actual prepared account signing, non-exportable hardware, independently qualified
observer/UTC/floor authority and daemon submission/state/config integration remain
open. Core must preserve original observation age across interval endpoints and later use.
Local proofs do not establish deployed custody or prevent a revocation racing
an in-flight key call. Four inner hardware approvals, all seventeen genuine lanes,
the foundational envelope, four-validator/provider/gateway deployment, load/24-hour
soak, workspace/SDK/strict-lint/security and full release qualification remain required.
No goal or lane is closed.
Earlier scoped results are preserved in the [historical excerpt](docs/history/2026-09-13/sorafs-before-account-custody.md).

```

## specs/sorafs/v1_implementation_goals.md

Source SHA256: `894b0523c1989c7a0537a31cb22267cb2f418af3a0fff4160b3139a17f2045e4`.
Excerpt SHA256: `d8cfb8a2401c849002315eba4916501a8105bec7fffda6299b335cfcafd85715`.

```markdown
## G02 checkpoint — 2026-09-13

The account-custody checkpoint passes **5,063 tests** across the complete
Manifest, DataModel, ExecutorDataModel, Executor and SchemaGen libraries; exactly
six named DataModel manual fixture printers remain ignored. Another
**714 native tests** (155 Core, 70 Torii, 487 daemon and 2 SCCP) pass with no ignores.
All 8,181 captured inputs, selected binaries, 42 controls and
build/baseline bytes remain unchanged; CI selects the same native names and all
75 mandatory sentinels. **1,094 release/CI contracts** pass.
The canonical role15 account custody, native control history, separate permissions,
fee/query/registration/schema ownership and Current Check are implemented.
Observer must differ from the enrolled Ed25519 target; current observer Check and
target Operate permissions are rechecked at one authenticated applied cut.
One private proof owner serves the distinct account and receipt wrappers; exact
payload preparation and real transaction signing remain the next production layer.
The payload digest is an independent commitment, not evidence of a reviewed payload.
Actual prepared account signing, non-exportable hardware, independently qualified
observer/UTC/floor authority and daemon submission/state/config integration remain
open. Core must preserve original observation age across interval endpoints and later use.
Local proofs do not establish deployed custody or prevent a revocation racing
an in-flight key call. Four inner hardware approvals, all seventeen genuine lanes,
the foundational envelope, four-validator/provider/gateway deployment, load/24-hour
soak, workspace/SDK/strict-lint/security and full release qualification remain required.
No goal or lane is closed.
Prior source and test scopes remain in the closure ledger and dated history.

```
