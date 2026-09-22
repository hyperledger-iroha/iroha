# Installed JavaScript content and native snapshot custody

This implementation was integrated only in
`/Users/takemiyamakoto/devstuff/iroha` on `optimizations`, based on
`bdadfae6175a89e5fdb56292a5a64b93b44a231f` with existing changes preserved.
No commit, branch, worktree, compatibility path or HSM requirement was added.
All fourteen overall release goals remain open.

## Integrated behavior

`scripts/sorafs_javascript_installed.py` derives the exact installed package
graph from the existing SDK/source and nine dependency archive consumers.
`sorafs_javascript_install_metadata.py` validates the mandatory generated npm
hidden lock against those originals and fixed private consumer metadata.
Package locations, bytes and modes must agree, including the nested hashes
dependency. The installed tree admits at most 8,192 files, 192 MiB content and
64 path components. It cannot establish candidate authenticity by itself.

`sorafs_javascript_installed_custody.py` retains the node_modules root and all
ancestor descriptors. Descriptor-relative no-follow reads check complete
initial/final inventory, bytes and physical identities; unowned files,
directories, metadata, links and addons reject. Leaf reads are bounded to
64 KiB. A failed or reentrant check permanently invalidates the owner.
Ambiguous close failures drain all other owned handles once without retrying
a descriptor number that may have been reused.

The original four-file proposal was rejected after independent review exposed
cleanup leaks, reused-descriptor closure, swallowed reentrancy and admission
checks performed after allocation. Revision 2 preserves its original 96
controls and adds eleven meaningful controls: the original proposal fails
all eleven, while revision 2 passes all 107.

That review also exposed an inherited partial-acquisition defect in
`release_manifest_signing._open_release_output_parent`. Its shared repair
attempts cleanup of every acquired descriptor once, preserves the original
acquisition exception and retains cleanup diagnostics. The only bootstrap pin
change is the exact reviewed helper successor:
`c7dbbbd6f3b21e05cff934f2a9db47d331988c4fc3141e7ad7b2e2edfe7cab4a`
to `4f2790c50923823db41b598b69b356bf761bfc597e2b4fe554f1de9136a8531d`.
The full trusted release-surface seal remains unchanged and unqualified.
All 59 original signing controls and 37 new acquisition controls pass;
the preserved original helper fails 28 of the new controls and passes nine.

`scripts/sorafs_javascript_child_files.mjs` retains the original native file
and controlled temporary-root ancestry before SDK import, then joins the actual
normal-loader snapshot through the original `NativeCacheObservation` method.
It checks the loader's exact path policy, permissions, sole snapshot member,
raw byte equality, physical identities and unchanged cache. Reads use 64 KiB
chunks and the existing 1 GiB per-original ceiling. It neither loads an addon
nor claims mapped-memory attestation. Node lacks openat; these before/after
lineage observations do not prevent a transient rename restored between checks.
The 81 maintained controls and four additional independent controls pass.

CI explicitly watches these source/test files. The existing Node 24 step runs
both event and native-file suites unconditionally, while the strict release
pytest batch includes installed-tree and shared-opener regressions. Private
event-only guard names were replaced with the current child-control names,
without aliases. SDK discovery, runtime profiles and native job seals are
unchanged. The workflow controls reject omission of either Node suite.

## Recorded validation

The integrated run at
`target/first-release-installed-and-snapshot-validation-20260922/` passed
**1,916 Python tests and 304 Node 24 tests: 2,220 distinct controls**.
There were no failures, errors, skips, cancellations, TODO outcomes or changes
to the 853 observed source inputs. Python took 153.02 seconds and Node 1.09
seconds. The Python selection includes signing, bootstrap rejection, shared
Python/index callers, installed content and both CI suites. Node covers event,
snapshot, cache, assertion structure and release-profile component behavior.
Repeated author/peer runs are not additional coverage.

The canonical installed owner also replays the existing inert offline npm tree:
966 files, 10,132,577 bytes, 999 file/directory seals and ten retained ancestry
descriptors. Its complete report matches the independently reviewed original
join. SDK archive SHA-256 is
`b095363cf96c416c4261a662f30e6ef3e1d09ad6e0ee5767bfe857dc7d9137da`;
hidden lock SHA-256 is
`45c1cb72ecfd62271950f097614b78067b6fc16ba750bab1370f35fbaf6ce22d`.
This replay did not run npm, import the SDK or load native code.

Automation validation, workflow action-pin checks, shell syntax and
`git diff --check` pass. Source-budget validation still refuses 276 existing
findings across 12,671 files; neither findings nor limits/baselines changed.
The accepted application packet records exact pre/post hashes under
`target/first-release-installed-and-snapshot-application-20260922.json`.
Its initial driver stopped after three successful patches because it selected
the wrong snapshot patch filename; the preserved partial record was verified
before resuming with the reviewed `implementation.patch`. No mismatched
snapshot bytes were applied. An initial CI-proposal generator also refused
an ambiguous marker; its corrected narrow anchor passed the focused controls.

Independent reviews are retained in the corresponding target directories:
`first-release-javascript-installed-review-20260922`,
`first-release-release-parent-cleanup-review-20260922`,
`first-release-javascript-child-files-review-20260922`, and
`first-release-javascript-child-wiring-review-20260922`.

## Remaining integration

The fixed qualification source/fixture snapshot, actual same-process child,
selected runtime/npm original custody, native ABI join, producer, original-index
adapter and matching-candidate execution remain required. The preserved six
suite bodies contain 172 assertions; component traces do not execute or qualify
those native assertions. The current local addon is not a matching release
candidate, and the earlier unfiltered suite failures remain recorded.

The shared-opener repair does not fix `_ReleaseOutputTransaction`'s separate
publication/rollback cleanup paths. Their ambiguous-close and partial leaf
acquisition defects require the next bounded repair. Complete source-surface
review, native/platform/hardware runs, independent audits, signed aggregate
approval and SF11/final promotion remain open.
