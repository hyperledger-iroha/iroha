# Mochi snapshot restore ownership checkpoint

This records the snapshot restore extraction after the genesis ownership
checkpoint. It qualifies the captured Mochi selection, not the complete release.

## Ownership and preserved behavior

`supervisor/snapshot_restore.rs` owns the restore transaction, durable journal,
commit, reverse rollback, cleanup and startup recovery. The supervisor retains
generation admission and leases, lifecycle coordination, snapshot metadata and
shared filesystem helpers. Its production source is now 4,379 lines; the new
restore owner is 1,077 lines. Both satisfy the 5,000-line limit.

The extraction accounts for all 224 original function bodies: 184 remain in the
parent and 40 move to the child, byte-identical. All 154 existing supervisor
tests remain byte-identical. Visibility changes expose only the sibling access
required by the owner and its tests.

Create-new/0600 journal publication, file and directory synchronization, bounded
peer admission, canonical generation/ancestor validation and rollback order are
unchanged. Uncertain publication retains installed state, backups and ownership
for startup recovery. The commit marker precedes cleanup; journal removal
precedes marker removal. No configuration, dependency, ABI, codec, optimization
or stack setting changes in this checkpoint. The three original Native AMX
stack-root owners retain their earlier qualified hashes.

## Scoped qualification

The test build, strict Clippy for all Mochi targets with GUI/dev-tools, and a
separate default-feature production Core Clippy run pass with zero warnings.
They retain the same 182-input fingerprint:
`f6f6e5aa974525da4effef079bac282ff58a39793b014629525e24e6c66a07da`.
The captured source inputs and eight compiler-produced executable artifacts are
retained locally. All runtime runs preserve these inputs and use no stack override.

| Suite | Passed | Ignored |
| --- | ---: | ---: |
| core-runtime | 449 | 1 |
| gui-runtime | 181 | 0 |
| readiness-runtime | 12 | 1 |
| mock-runtime | 9 | 0 |
| integration-runtime | 3 | 1 |
| streams-runtime | 2 | 0 |
| real-kagami-runtime | 1 | 0 |

Readiness includes the ordinary supervisor scenarios. The real-Kagami case is
selected explicitly with its retained executable and verified environment; its
earlier 68-file direct Kagami/genesis capture is not a new full-transitive release
build. Formatting, the codec guard, and all 51 source-budget script tests pass.

The supervisor's obsolete size exception is removed. The existing GUI exception
tightens from 11,983 to 11,900 lines; its further decomposition remains open.
The other 170 exception entries, production/test limits and exclusions remain
unchanged. The complete source-size guard still fails with 237 findings and 171
exceptions. It reports no Mochi findings under those limits and existing ratchets;
this does not mean every Mochi file is below the ordinary file-size limit.

An initial isolated patch replay inherited an outer Git context and skipped
paths. Exact output comparison rejected that preparation attempt. The corrected
isolated replay reproduces every source byte; the failed preparation evidence
remains retained. No runtime failure is concealed by this correction.

Beforeimages, preservation proof, exact source/executable identities, checks and
the final scoped patch are under
`target/architecture-redesign/mochi-snapshot-owner/`. The capture does not include
the complete transitive workspace source closure. Full SDK/workspace tests,
mandatory four-validator scenarios, native/device delivery and pinned comparable
build-memory measurements remain open. The complete redesign goal remains active.
