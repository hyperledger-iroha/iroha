# JavaScript runtime floor and package integration — 2026-09-22

Work remains in `/Users/takemiyamakoto/devstuff/iroha` on `optimizations`,
HEAD `bdadfae6175a89e5fdb56292a5a64b93b44a231f` plus preserved uncommitted changes.
No branch, checkout, compatibility path or HSM requirement was introduced.
All fourteen overall release goals remain open. These are component results,
not installed native execution, a supported-platform matrix or promotion.

## Integrated behavior

The package and root npm lock now declare Node `>=20.19.0`, matching the actual
production closure's nested `@noble/hashes` requirement. Third-party versions,
integrity values and dependency edges are unchanged. The same guard runs first
in prepack and prepublish, and after the privacy runner's existing Node 20
selection but before native compilation. SoraFS/other CI major selections and
the four sealed native privacy job bodies are unchanged.

`node-engine-contract.mjs` is the sole pure dependency/runtime policy owner.
It checks exact/caret dependency selection, canonical nested lock locations,
the reached production graph, reviewed engine ranges and the advertised floor.
`check-node-engine.mjs` is an unconditional CLI with no argument or runtime-policy
override; it reads the canonical physical script's sibling and metadata.
There is no main-detection compatibility export. The unpublished CLI and pure
contract are both mandatory, exactly pinned package source inputs.

Review reproduced a symlink invocation bypass in the initial combined CLI/API
proposal and a stdin-import regression in the first attempted correction. The
split above removes both. A noncanonical scope-directory lock row could also
shadow a production dependency despite being marked development-only; the
fixed narrow lock-location grammar rejects that layout in every row.

The original thirty controls remain, with twenty-nine additional maintained
cases for normal/file/ancestor invocation, preserved main symlinks, canonical
metadata selection, refusal of arguments, pure imports and unsupported rows.
The privacy source-order/trigger controls and negative subprocess controls live
in the existing native gate test owner. Injected version observations there are
explicitly synthetic controls; they are not executions on Node 20.18.9.

## Original package observation

Two fresh owned staging directories each contain the same 202 captured source
inputs. Actual offline `npm pack` executes the reviewed prepack guard and
copy-to-dist recipe. Both produce the identical archive:

`b5404a1f688f6d79d802114c25eacfa21baf567f701bcfe91de5539191d8d69f`

Each archive has 199 members, 953,066 compressed bytes and 4,947,968 inflated
tar bytes. The production content owner compares every name, byte and 0644
permission with the captured source projection. All nine retained original
dependency tarballs also pass their actual integrity/location joins against
the newly selected lock digest
`fab1fa4abc744c7950327ca62e32a0bd807e16b60edb5cef4fac91945cbe00a4`.

The original native checksum manifest is only an unqualified opaque input.
Its custody permissions remain unchanged; only its public staged copies use
0644. No addon was packed, loaded, built or installed. Node 26.9.0/npm 11.19.1
are local observation tools, not release runtime qualification. Captured source
and observed tool bytes stayed unchanged. This is local package determinism,
not complete toolchain or signed reproducible-release evidence.

Packet: `target/first-release-node-floor-pack-20260922/identity.json`.
The prior package/archive checkpoints retain their original identities and
must not be relabeled as evidence for these changed source bytes.

## Integrated validation

The source-observed combined run passes **1,682 Python/CI controls** in 178.46
seconds and **228 Node controls** in 3.39 seconds. There are zero failures,
errors, skips, cancellations or TODO cases, and zero drift across 833 observed
files. The 1,910 case identities include the narrower 68-Node/138-Python loops;
those earlier loops are not additional coverage. Logs, commands, exact counts
and input identities are in
`target/first-release-node-floor-combined-validation-20260922/identity.json`.

Focused strict ESLint, shell/Node syntax, SoraFS automation validation and diff
whitespace checks pass. Source-budget validation still refuses 276 findings;
neither ceilings nor baselines were changed. These are genuine component
executions with inert native-cache records and shared assertion registration,
not execution of the six native assertion callbacks.
Historical archive verification passes for 64,736 records and 67,311
occurrences. Root status/roadmap remain within their limits at 300/146 lines.

## Qualification still open

The full privacy JavaScript runner still refuses the committed Cargo graph:
current lock `e3f5f854...` differs from the helper's `398cd15f...` pin. A separate
history review traces this to integrated `concread` workspace/Core dependency
changes, with twelve registry additions and no retained package identity or
checksum changes. The pin was not changed to make the Node-floor controls pass.
See `target/first-release-lock-authority-review-20260922/REVIEW.md` for the
original graph comparison and the required reconciliation evidence.

Fresh bundle measurement also exposes existing ceiling failures: Torii eager
output is 818,275 bytes against 816,128; the browser deployment continuation is
9,228 bytes against 9,216. Torii's former `node18` and current `node20.19` targets
produce identical output bytes. Budgets were not increased. These failures
remain separate from passing metadata and archive controls.

The actual installed JavaScript producer still needs complete source/runtime/npm
custody, exact offline installation, retained native snapshot/ABI identity,
complete six-suite execution observations and original-index verification.
Independent audits, all hardware/platform runs, signed aggregate and final
matching-candidate qualification remain required.
