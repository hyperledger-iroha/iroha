# Original JavaScript parent input integration

Work remains in `/Users/takemiyamakoto/devstuff/iroha` on `optimizations`, at
observed HEAD `1b8e5f92b6dadb1dcdd16f945286560e31b347ff`. This agent created no
branch, worktree, commit or release candidate. All fourteen release goals remain
open. First-release compatibility paths and an HSM prerequisite remain prohibited.

## Implemented input boundary

The [parent input owner](../../../specs/sorafs/javascript_parent_input_v1.md)
retains the original installed/source/tool trees, original native/checksum/ABI
files and their physical ancestry. It uses the existing content validators and
ABI schema, joins the two distinct source digests, and creates the fixed child's
bounded canonical input file exclusively at mode 0600. Eight tool hashes select
the reviewed originals; callers cannot substitute their own approval list.
The owner exposes the original descriptor and its byte digest for the future
fixed process runner. Refusal is terminal; cleanup attempts each detached
descriptor once, retains errors and leaves partial or replaced output paths
untouched. Native hashing streams bounded reads without buffering the artifact.

The twelve integrated files comprise six new implementation/test/spec files
and six CI/guard changes. Both workflow filters include the new inputs; the two
new Python suites execute in the existing release pytest batch. The canonical
guard verifies the complete three-suite ABI/input inventory. No compatibility
alias, alternate codec, native runtime matrix or release-source seal was added.

Input patch: `4d863b4e0a99051baa57a32ab30c9069be25b809ed1482670a5d27d8b0235506`.
CI patch: `548440c7e64165f9e7300414c32a89721ffb0fde33b4d2e88b895ef795b33469`.
The exact application receipt is
`target/first-release-javascript-parent-application-20260922.json`
(`04fc4f7cf2e262cedfa1a5947724de9d39b1f2d4dc0ee76f16536785628c308b`).

## Validation and remaining authority

Independent review passed all 57 new file/input controls and a pure Node 24
parser join with 429 scoped inputs unchanged. The first root parser harness
incorrectly read from the original writer descriptor's end position and was
refused; its script and failure remain recorded. The corrected positional-read
harness matches the existing child's read contract; product code was unchanged.

The canonical integrated run passed **1,705 unique tests**: 1,368 automation,
75 workflow contracts, 24 file, 33 parent input, six ABI, 107 installed/tree and
92 qualification/tree controls. All 5,115 setup/call/teardown phases passed,
with no skips, xfails, deselections, collection or internal errors. All 1,070
observed source files and 265 tool/module files remained unchanged. The source
census includes actual validator-owned docs/specs, status and roadmap. An earlier
run omitted documentation from its census; the corrected run supersedes it.
Standalone automation validation, shell syntax and scoped diff checks passed.

The final receipt is
`target/first-release-javascript-parent-integrated-validation-20260922-2/receipt.json`,
SHA-256 `793066294df1d0b05c83c84170ed50c9a9bd0bd169a962f7e7dadc4cb171fcad`.
The controls use inert archives/native bytes and real file/tree mechanics.
They do not execute SDK/addon bodies, verify an actual native ABI process,
authenticate a candidate or establish complete runtime/mapped-code authority.
Node/npm input custody, fixed process/output completion, the original-index
adapter, genuine native execution and signed release qualification remain open.

## Separate runtime evidence

The current canonical Core library passes a locked/offline check in 302.29
seconds with unchanged observed inputs. Two private helpers used only by test
paths still produce a library warning; their test-only scope is being corrected
in the pending runtime patch. This is baseline compilation, not runtime tests
or compilation of the unapplied membership proposal.

The exact target-only native composition passed 461 unit, eight integration and
four documentation tests with unchanged inputs and ordinary stacks. Its pruned
component lock contains 140 package identities/checksums from the proposed root
lock. Core integration still requires original reader notices to outlive every
State/World/effect/snapshot fence, including transitive merge validation and
autoscale reads. The current membership successor preserves the newer root and
record implementation and uses its exact charged publication identities.
These proposals do not establish complete State admission or Native cutover.

The later source-budget diagnostic retains 276 findings and no new finding
paths; an existing Taira test finding differs from the earlier diagnostic.
No ceiling or baseline was relaxed. This is not candidate qualification.
