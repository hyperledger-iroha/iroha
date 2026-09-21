# Historical SoraFS authority checkpoint

These exact source excerpts precede the September 13 native Check checkpoint.
Their local results retain their original scope and do not qualify later source.

Source: `status.md`. Excerpt SHA-256: `cd51ec860bc74e31bf4046191052fe51580dc2266c9b7aea47ceac9cf565ff83`.

SoraFS follows the [V1 implementation goals](specs/sorafs/v1_implementation_goals.md)
and the [native authority checkpoint](specs/sorafs/v1_closure_ledger.md#native-deployment-authority-and-schema-checkpoint).
The native authority checkpoint passed **5,020 tests** across complete Manifest, DataModel,
Executor, ExecutorDataModel and SchemaGen library suites; six DataModel manual
fixture printers are ignored. Another **65 Core, 45 Torii and 122 daemon tests**
passed on unchanged captured inputs and binaries. Native
custody and operation histories enforce exact permissions, permanent replay
fences and atomic revocation. The schema reference is regenerated with three
byte-identical runs. The subsequent consensus Check/consumer changes need new validation.
All **30 cosign cryptographic regression cases** pass on unchanged captured
inputs; pinned Linux execution remains a CI qualification step. The final checker
requires hardware custody and all four inner hardware approvals, which remain
incomplete. A historical QC or logical header age cannot prove current revocation
state: actual hardware, native adapters and challenged quorum authority remain
open, as do full workspace/SDK checks, four-validator qualification, all 17 genuine
lanes and load/soak. Two unfinished-source inventory guards still fail. Earlier
local counts are retained in a [historical excerpt](docs/history/2026-09-12/sorafs-status-before-native-authority.md).

Source: `specs/sorafs/v1_closure_ledger.md`. Excerpt SHA-256: `6268ca49ad2cc6f56103b83f09d44fcfa42276e1b404103eec28477f9372b03c`.

### Native deployment authority and schema checkpoint

The same isolated candidate now implements the
[native role-14 contract](final_promotion_native_authority_v1.md): one
deployment-scoped mutation with Configure, Enroll, Revoke, Reserve, Complete and
Expire actions; separate exact custody-management and operation permissions;
bounded immutable custody/operation histories; permanent operation IDs and
monotonic fences; and atomic invalidation of outstanding reservations when
custody changes. Completion requires the original account and coordinates in a
later block than reservation, strictly before expiry. Native records retain
commitments without publishing unreleased signatures.

Both native custody owners use one generic Manifest policy/control schema and
one policy transition implementation. Retired StreamToken-only schema names are
removed without aliases. Corruption regressions cover orphan retired-key indexes,
rollback of an old operation's latest index, independently replayed control and
operation prefixes, and failure without partial publication. The shared Core
finality helper binds exact same-State and durable Kura history, authenticated
revision-4 finality and genesis-derived network identity. It deliberately proves
historical authenticity, not current revocation state.

The canonical schema generator explicitly roots both custody instructions,
their retained records and scoped permissions. Regression tests check the full
recursive descriptors while retaining framed policy/enrollment payloads as
opaque bytes and excluding runtime qualification types. Closed operation JSON
uses the canonical unit action `{"kind":"sign","value":null}`. Registry goldens
were recomputed after independently confirming the sole added instruction.
Broader model tests exposed capture builders that inherited a changed genesis
policy default; their five input fields are now explicit, with original golden
bytes and production defaults unchanged and complete header/signature checks.

The following local checkpoint was rebuilt and executed before adding the
consensus-ordered Check action, with no failed tests:

| Selection | Result | Retained log under `/Users/takemiyamakoto/.cache/sorafs-v1-20260912/` |
| --- | --- | --- |
| Complete Manifest, DataModel, Executor, ExecutorDataModel and SchemaGen library suites | 1,056 + 3,742 + 172 + 37 + 13 = **5,020 passed**; six DataModel manual fixture printers ignored | `native-authority-five-library-schema-final-tests.log` |
| Core final-promotion, signer finality, StreamToken custody, fee and warning-owner regressions | **65 passed**, no ignores | `native-authority-iroha_core-runtime-final-tests.log` |
| Torii hardware/finality and token lifecycle regressions | **17 + 28 = 45 passed**, no ignores | `native-authority-iroha_torii-runtime-final-tests.log`; `native-authority-iroha_torii-lifecycle-final-tests.log` |
| Daemon signer operations and software-provider rejection | **122 passed**, including all 13 immutable-statement tests; no ignores | `native-authority-irohad-runtime-final-tests.log` |

The cache index `native-authority-local-checkpoint.json` has SHA-256
`bd8d3f37bde460d2aa2e3156789dfb5fb6363a03dd4c0a897eac399c13e88d66`
and retains exact log hashes, commands and scope. All three runtime binaries
compiled in one package/feature graph. During the 232 runtime tests, all 8,128
captured source/manifests/toolchain inputs and the binaries stayed unchanged.
Capture starts after compilation; this is not a whole-build source seal or
complete Core/Torii/workspace execution. The checkpoint also records 804 release
owner-contract tests, format/codec checks and three identical schema generations.
The source-budget guard still reports 241 inherited findings, including three
changed Core files whose line counts did not grow in the warning fixes; Kagami
still emits 92 existing generator warnings. Strict Clippy remains unqualified.

The daemon service now pins one immutable reviewed statement and rejects invalid
bytes or coordinates before state/provider I/O. Signing and recovery take no
replacement byte argument; fresh custody, audit and completion fences remain.
The previously empty schema reference is regenerated with 1,708 unique descriptors,
including all eight explicit roots;
three generator invocations are byte-identical. The 572,504-byte artifact has
SHA-256 `5b8bae97020e3c17817a3b33311554bb694922bf919a712421537e6831c8791e`.
Actual hardware and native submission/state adapters remain unimplemented. The
subsequent Check action and applied-state consumer are under implementation and
require fresh validation. The authority review also establishes
that V2's signed logical time can admit an old future-dated QC into a later age
window. Header age and a newly signed single-node observation cannot supply a
fresh consensus observation; the [current-authority prerequisite](final_promotion_native_authority_v1.md#current-authority-prerequisite)
requires an exact successful challenge transaction, committee continuity and
current applied-state fencing, including account/role permission revocation.

This checkpoint completes no goal or lane. Genuine HSM/PKCS#11/KMS custody,
production provider/state adapters and operation-authority qualification remain
open. The genuine four-validator deployment with mandatory signed RS16
DA/RBC, multiple providers, two independently administered regional gateways,
load/soak, recovery rehearsal, workspace/SDK checks and independent security
qualification are still required. All 17 signed readiness lanes and the trusted
nine-prerequisite foundational envelope in its specified order remain mandatory.


Source: `specs/sorafs/v1_implementation_goals.md`. Excerpt SHA-256: `b197b424c0b28c5c18d965a8255a4e753cd0519a39a9c3bc638a5600083de6a0`.

## G02 checkpoint — 2026-09-12

The isolated candidate implements native StreamToken custody and the separate
role-14 deployment authority, exact executor permissions, protected native
namespaces, shared policy transitions and same-State durable-finality checks.
The [current closure checkpoint](v1_closure_ledger.md#native-deployment-authority-and-schema-checkpoint)
records the full Manifest (1,056), DataModel (3,742), Executor (172),
ExecutorDataModel (37) and SchemaGen (13) library passes: **5,020 tests** in all.
Six ignored DataModel cases are manual fixture printers. The same checkpoint
passes **65 Core, 45 Torii and 122 daemon** runtime checks against unchanged
captured inputs and binaries. The empty schema reference has three
byte-identical generator runs. Full Core/Torii library and workspace/SDK
qualification remain open. The subsequent Check/consumer changes need new validation.

Final promotion uses one Ed25519 role and deployment purpose. Its producer now
pins the exact canonical reviewed statement before state/provider I/O; signing
and recovery accept no replacement bytes. Fresh custody, audit and original
completion fences remain mandatory. The statement binds chain, network,
deployment and the full signer tuple along with replay/archive/toolchain and
provenance inputs. The CLI remains an offline receipt consumer.

Actual hardware and native transaction/state adapters remain unimplemented;
the consensus-ordered purpose-native `Check`/runtime consumer is under implementation. Bounded models supply
design evidence only. A signed historical QC, logical header
age or freshly timestamped local read cannot prove current revocations under
partition. Complete the production adapter and all underlying hardware signer
profiles together, with no software-only compatibility path. Historical private
packets, local tests, syntactic preparation and an outer hardware signature do
not qualify deployment custody, all 17 genuine lanes, or the 24-hour soak.

