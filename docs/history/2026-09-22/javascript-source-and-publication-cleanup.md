# Qualification source custody and publication cleanup

Integrated only in `/Users/takemiyamakoto/devstuff/iroha` on `optimizations`,
based on `bdadfae6175a89e5fdb56292a5a64b93b44a231f`, preserving existing work.
No branch, worktree, commit, compatibility path or HSM requirement was added.
All fourteen overall release goals remain open.

## Integrated behavior

The JavaScript qualification source owner admits exactly 191 original files:
nine pinned code/contract inputs and 182 named fixture/payload files. The two
fixture trees and fixed payload are complete; the metadata cannot select another
payload. Source count, path and byte bounds precede projection allocation.
The catalog fixes source code and fixture names; authentic candidate fixture
bytes still require their independent original authority.

Installed-package and qualification-source owners now use one physical tree
implementation, `sorafs_javascript_tree_custody.py`. Their separate typed
content relations remain mandatory. The extraction preserves the reviewed
descriptor-relative reads, original ancestor custody, terminal invalidation,
and detach-before-close behavior. It does not fabricate npm metadata for a
source tree or prove which bytes executed between physical observations.

`_ReleaseOutputTransaction` now retains an output descriptor before its first
metadata read, drains acquired descriptors once, invalidates failed/reentrant
operations, and preserves the original exception with cleanup diagnostics.
Cleanup-only failure reports whether validated outputs remain. Interrupted
container insertion transfers ownership by exact retained-record identity;
local and terminal cleanup cannot both close the same transferred descriptor.

Independent review rejected the first transaction proposal: an insertion that
stored its record and then raised caused double close and could erase a reused
descriptor's unrelated file. Revision 2 preserves those reproductions and adds
before/after-insertion controls, including an interruption immediately after a
real builtin list append. The rejected `93e1e099…` helper was never integrated.
The accepted helper is
`4966d0b408f7d67d154b578875a1b0cb77584de77b4b1561e8704043b99b60fd`;
the pipeline changes only its matching direct bootstrap pin. The full trusted
release-surface seal remains unchanged and unqualified.

The existing concurrent `stat`/`unlink` pathname-replacement race remains:
the inode comparison is not an atomic conditional unlink. This patch does not
close that separate publication issue or claim foreign-path preservation under
concurrent namespace mutation.

Both new regression suites execute in the strict release pytest batch. CI
watches the shared tree owner, source relation, catalog, spec and tests. The
existing Node 24 event/native-file command, native job seals, SDK discovery and
runtime profiles are unchanged.

## Validation

The integrated affected Python/CI run passed **2,069 tests**, with no failures,
errors, skips or changes to its 860 observed inputs, in 157.84 seconds. It covers
both CI suites, signing/cleanup, original-index and Python shared callers,
installed/source custody and bootstrap rejection. The unchanged Node suites
were not rerun; their earlier 304-test receipt retains its original scope.
No SDK/addon was loaded and no candidate execution is qualified by this run.

The complete source proposal passed 199 author and independent controls;
transaction revision 2 passed 136 maintained controls and independent actual
descriptor-reuse/interruption reproductions. These repeated component runs are
not additional integrated coverage. The original 127-test transaction result
and its missed regressions remain preserved.

The application receipt is
`target/first-release-source-and-transaction-application-20260922.json`
(`4b75dba69a2e7594894a1f16933b94c389cfa480069951f242c8d3c2a0a6d626`).
The validation packet is
`target/first-release-source-and-transaction-validation-20260922/`:
identity `d953d49c1bd6d2326a768e3840e59cc096a1ccc50edf998b0de7373afad28e54`,
source observation `6f2df6d48705c41718165f5a0c43c79c432474a6c02e3807bcbf131d08842daa`.
Independent source and transaction reviews are retained under their respective
`first-release-javascript-qualification-source-review-20260922` and
`first-release-release-transaction-review-20260922` target directories.

Automation validation, shell syntax and `git diff --check` pass. Source-budget
validation still refuses the same 276 findings across 12,676 source files;
no findings, ceilings or baselines were relaxed.

## Remaining work and separate runtime proposals

The fixed same-process JavaScript child, runtime/npm original custody, native
ABI join, producer and original-index adapter remain under implementation.
The child must join the unchanged six suite bodies and all 172 assertions to
the original installed/source/native owners, actual final hook, complete event
stream and process completion. Module-loader observations alone cannot prove
final compiled bytes: a later hook can transform a previously observed result.
The runner and runtime must have their own authenticated source closure.

The finite transaction-history config, Kura pool, native membership storage and
fresh/snapshot constructors remain **target-only proposals**, not integrated
runtime qualification. Review found that stringified restore capacity errors
would be classified as snapshot corruption and permit empty-State replay
fallback. The revised proposal preserves typed membership/hash admission
through snapshot loading and daemon policy. Its native owner still needs a
reviewed publication/unwind boundary that preserves ordinary prepared readers
and original local retry custody. JSON scratch and current hot-tip allocations
remain separately unadmitted. No production Native activation or full State
resource-admission completion is claimed.

Matching native/platform/hardware runs, full release-source review, independent
audits, signed aggregate approval, SF11 and final promotion remain open.
