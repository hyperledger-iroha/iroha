# Cargo graph and JavaScript bundle integration — 2026-09-22

Work remains in `/Users/takemiyamakoto/devstuff/iroha` on `optimizations`,
HEAD `bdadfae6175a89e5fdb56292a5a64b93b44a231f` plus preserved uncommitted changes.
No checkout, branch, compatibility path or HSM prerequisite was introduced.
All fourteen overall release goals remain open. This checkpoint follows the
[runtime-floor record](javascript-node-floor-integration.md); that record's
failed measurements and original package identity remain historical evidence.

## Committed Cargo graph reconciliation

The privacy lock helper now pins the existing committed root lock:
`e3f5f8546b358db33bc501e4e46d65ed32da3aeb81c10f4e95325583df687de8`.
`Cargo.lock` itself is unchanged. The preceding `398cd15f...` identity joins
the older `6db7...` identity as a rejection-only test case; neither is accepted
as an alternative graph.

History review traces the mismatch to integrated `concread` workspace and Core
dependency changes followed by a merge that retained the older helper pin.
The lock has twelve added registry packages, no removals, and no changed
version, source or checksum for a retained package. Exact Rust/Cargo 1.93.1
locked offline metadata passes with 899 selected packages and 106 workspace
members. The maintained binary inventory passes with 23 default and 102
declared binaries. Five explicit normal/build/feature graphs pass; none of the
twelve registry additions occurs in those shipping selections.

Nine cached original crate archives match their lock checksums and 216 regular
payload files match the extracted source. Three optional originals initially
missing from cache were subsequently fetched into the review packet only:
`dhat` 0.3.3, `mintex` 0.1.4 and `thousands` 0.2.0. Their exact lock checksums
were verified before bounded tar inspection of 45 files and license texts.
No archive was installed or executed and Cargo's cache was not changed.
This establishes original bytes and license presence, not an independent
security audit. One cached proc-macro archive declares MIT without a separate
license text; the review retains that observation.

The integrated materialization suite passes **34 tests and 22 subtests**.
The maintained source-lock footer passes against actual HEAD, index and working
tree with complete original variable declarations, empty stderr and unchanged
observed inputs. An earlier diagnostic omitted `WORKFLOW_PATH` and emitted an
unbound-variable error despite exiting zero; it remains recorded and is not
counted as a clean footer pass.

The full shell harness remains unqualified. Its first local run used a Python
without `tomllib`; the Python 3.12 rerun refuses the actual repository's
ancestor `.cargo/config.toml` inherited by its temporary fixture checkout under
`target/`. No config policy was weakened and fixtures were not moved outside
the required workspace. The complete shipping-feature guard also still refuses
the trusted release source seal. Its captured observation was `076ea56d...`
against `803b6cb4...`; no seal was repinned to suppress that failure.

Evidence: `target/first-release-lock-authority-review-20260922/` and
`target/first-release-graph-reconciliation-validation-20260922/`, including
`actual-source-footer-complete.json` and both original full-harness failures.

The subsequent bounded seal review captures `eec174bd...` over 7,580 live inputs
at 04:14:07–04:14:14 UTC, stable only within that observation. The earlier
`076ea56d...` inventory reproduces from its retained original hashes/bytes.
There are 1,707 changed committed inputs between the seal-introducing commit
and current HEAD, including 167 semantic field changes in 56 Cargo manifests.
The [September 9 record](../2026-09-09/dependency-pruning.md) describes the
earlier narrowly reviewed `803b6cb4...` working surface; its referenced original
beforeimage directory is unavailable. The introducing commit contains later
changes and is not itself a proven passing baseline. All five inspected
bootstrap helper pins still match. The exact inventory and bounded next review
subsets are in `target/first-release-surface-seal-review-20260922/REVIEW.md`;
no source seal is approved by this comparison.

## Bundle ceilings and behavior

Torii reuses existing immutable type literals and four repeated field/context
constants. The deployment continuation reuses the existing error-throwing leaf
helpers. AST and binding review checks all 329 Torii literal substitutions and
32 throw boundaries, with no changed property names, validation messages,
ordinary error types or causes. Helper calls add a stack frame; the review does
not claim equivalence under hostile mutation of ambient JavaScript intrinsics.
Sixteen golden byte/census expectations were regenerated with every assertion
retained. Production bundle limits and reference budgets are unchanged.

| Measured output | Before | Integrated | Unchanged ceiling |
| --- | ---: | ---: | ---: |
| Torii eager bundle | 818,275 | 815,761 | 816,128 |
| Browser deployment continuation | 9,228 | 8,652 | 9,216 |

Actual Node 24.21.0 with esbuild 0.28.1 passes all seven declared bundle checks
and 22 browser export/Buffer checks. All **22 bundle/helper tests** pass.
Independent AST/binding review and 18 pure/mock controls also pass; overlapping
test identities are not additional coverage.

A broader unfiltered six-suite probe with addons disabled reports **21 passes
and six native-dependent failures** on both original and changed source. The
missing-addon failures remain recorded; those suites are not qualified. The
available canonical addon manifest describes an older debug revision and was
not relabeled or used as matching native evidence.

Evidence: `target/first-release-node-floor-bundles-20260922/candidate-static-literals/revision-2/`,
`target/first-release-bundle-semantic-review-20260922/`,
`target/first-release-bundle-behavior-20260922/` and
`target/first-release-bundle-postintegration-20260922/`.

## Fresh package observation

Two fresh offline `npm pack` executions with Node 24.21.0 and its matching npm
run the actual prepack guard and copy-to-dist recipe. Both produce:

`b095363cf96c416c4261a662f30e6ef3e1d09ad6e0ee5767bfe857dc7d9137da`

The package contains 199 exact members from 202 captured source inputs,
953,565 compressed bytes and 4,950,016 inflated tar bytes. Every member's bytes
and 0644 mode match the source projection. All nine retained original dependency
archives reauthenticate against the selected root lock. Observed source inputs
did not drift.

The native checksum manifest is still an opaque, unqualified original input;
no addon was built, packed, installed or loaded by this observation. This is
local package determinism, not native execution, complete toolchain provenance
or signed reproducible-release qualification. Exact commands and identities are
in `target/first-release-bundle-pack-20260922/identity.json`.

The fixed installed JavaScript producer and original-index adapter, full native
and platform execution, source-seal review, independent audits, signed aggregate
and matching-candidate release qualification remain open.
