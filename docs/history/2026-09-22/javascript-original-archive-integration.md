# JavaScript original archive checkpoint — 2026-09-22

Work stayed in `/Users/takemiyamakoto/devstuff/iroha` on `optimizations`. No
alternate checkout, branch, compatibility path or HSM prerequisite was added.
The checkout advanced through other source checkpoints during this work; the
scoped source observations below are the validation identity, not a sealed
release candidate. All fourteen overall goals remain open.

## Implemented

The [original npm contract](../../../specs/sorafs/javascript_original_archives_v1.md)
has one bounded gzip/ustar/PAX owner and one fixed nine-dependency lock/content
owner. Gzip integrity, tar checksums, safe member/ancestor ownership, PAX field
linkage, exact regular permissions and complete end-of-archive consumption are
checked before captured content can be used. Inflation, members, metadata and
path inventories have fixed bounds; shared callers can only reduce the budget.
The parser performs no extraction, install, import or package execution.

Dependency admission preserves the two distinct hashes packages and their
original locations. Actual SHA-512 integrity precedes parsing; package identity,
edges and engine declarations must match the independently selected candidate
lock. All nine archives share compressed/inflated limits. Bundled packages,
install hooks, implicit executable declarations, alternate resolution,
unreviewed platform metadata, secondary locks and native/retired-VM artifacts
reject. No mutable package-name lookup or online resolution supplies authority.

The helpers, tests, actual inert npm fixture and contract are registered in
SoraFS release-CI watches and checks. Existing source-native SoraFS tests and
portable package-smoke behavior are unchanged.

## Validation

- Combined archive/dependency/release-automation tests: **1,413 passed** in
  70.60 seconds, zero failures/skips. Raw output and JUnit are
  `target/first-release-javascript-archive-combined-20260922.{log,xml}`.
- Source-observed archive/dependency/index/JSON tests: **340 passed**, zero
  failures/errors/skips or observed source drift, 13.55 seconds including
  original-byte replay. Packet:
  `target/first-release-javascript-content-validation-20260922/`.
  Its `source-observation.json` SHA-256 is
  `044676d93d2e42a2194caa8951959810a8e38e1dd0c074ca5cf7a9c81b888533`.
- The runs cover **1,507 distinct test identities**, with 246 shared cases.
  Production-bound controls exercise 32 MiB members, 256 MiB cumulative
  inflation, 20,000 members, 80,000 path nodes, 16 MiB retained ancestor names
  and 1,024-byte UTF-8 paths. Compressed/PAX admission branches use reduced
  exact limits alongside fixed-ceiling assertions; no giant metadata fixture
  is represented as a full production execution.
- Nine real public registry originals match the current source lock's SHA-512
  pins and replay as **766 members**, **971,865 compressed bytes** and
  **5,939,200 inflated tar bytes**. Exact member hashes are in the packet's
  `original-replay.json`. No package or native module was installed or loaded.
- A separate agent compared all actual member names, bytes and modes against
  stdlib tar interpretation, rejected all 445 truncations of the actual npm
  fixture, checked five trailing-stream forms and 25 synthetic archives. This
  is agent review and component evidence, not the required independent audit.
- Automation check, shell syntax, diff whitespace and historical archive
  verification pass. Historical verification covers 64,736 records and 67,311
  occurrences. The source-budget guard still reports **276 findings**; none
  concerns the new helpers and no limit was raised. Logs use the
  `target/first-release-javascript-archive-` prefix; source-budget JSON is
  `target/first-release-source-budget-after-javascript-archives-20260922.json`.

An initial agent run passed 133 controls and failed the stdlib PAX-positive
case because its inert metadata header uses mode zero. The final parser admits
zero only for PAX metadata and retains 0644/0755 for actual files; the revised
143-case archive packet passes. Original failing and passing logs remain in
`target/first-release-javascript-archive-tests-20260922/`.

Review also demonstrated missing refusals for `directories.bin` and tar-only
`acceptDependencies`/`os`/`cpu`/`libc` policy. The reproductions explicitly reseal
synthetic archive and lock identities; they do not bypass an unchanged trusted
lock pin. Canonical corrections reject each reproduction while retaining the
nine actual inputs. Frozen pre-fix sources, repros and corrected observations
remain in `target/first-release-javascript-archive-review-20260922/`.

## Still open

F12/SF11 require the actual installed JavaScript producer and original-index
adapter, source-to-dist/package equality, pinned offline runtime/npm custody,
complete installed/loaded-module observations, native snapshot and ABI-23
execution, all original assertion bodies, signed aggregate authority and
matching-candidate platform runs. The package's >=18 advertisement conflicts
with the nested dependency's >=20.19.0 requirement; content checks do not resolve
or qualify the Node support matrix. Privacy, service/consensus integration,
formal/hardware/distributed testing, independent audit and promotion gates
remain governed by their existing ledgers.
