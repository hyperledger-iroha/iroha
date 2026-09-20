# Taira deployment allocation admission

`scripts/taira_disk_capacity.py` checks a public metadata plan on the machine
that will allocate the files. It has no SSH, configuration-reader, secret,
cleanup, or reservation capability. Exit status zero requires enough available
bytes **and** inodes after summing every allocation on the same actual filesystem.
Use `f_bavail`, not root-reserved `f_bfree`.

The exact input schema is `taira.disk-capacity.plan.v1`, with `allocations` rows
containing `path`, `label`, `bytes`, and `inodes`. Each row describes conservative
**additional allocated** resources at a directory, including simultaneously live
temporary files. Existing copies remain charged by the filesystem observation.
The helper resolves existing path components without following symlinks; missing
destination directories use their nearest existing directory's filesystem.

The native inventory requires an explicit `qualification_scope`. For
`core_testnet`, the fresh topology is **3A + explicit headroom**. All 30 artifact
roles are included: seven per validator (including its unit), plus two edge
roles. Core requires the stage/SF1 inputs to be explicit null, stage lists empty,
and store/runtime path lists empty. It never needs an Inrou preparation.

For `full_inrou` with coordinator and four validators on one guest, the complete
rollout topology is **3A + 2S + 4P + 4R + explicit headroom**:

- A: all four validator artifact sets plus the edge artifact set. The coordinator
  snapshots each role, uploads it, and installs another copy. Shared input paths
  do not deduplicate these role copies.
- S: the complete Inrou stage tree. One coordinator snapshot and one physical-host
  upload remain live together. These include full materialized sparse payloads.
- P: one store containing the bundle, guest, and discovery payloads, plus manifest,
  PoR/index metadata, publication temporaries, directories, and file-block slack.
  Four distinct stores each retain a complete payload set.
- R: one replica's runtime materialization. The daemon streams the immutable
  guest files out of its SoraFS store, then makes a separate writable root disk.
  Include both copies, the full lease and ephemeral storage budgets, app bundle
  cache/extraction/block-device copies, and publication overhead. Four replicas
  have distinct materialization and lease directories even on one physical host.
  The stores remain present while the runtime copies are created.

`cohost_peak_plan()` expresses the full Inrou topology, while `allocation_bound()` bounds
per-file block slack from metadata-only byte/file/directory counts. Callers must
provide four runtime paths and an explicit per-replica runtime footprint; preseed
capacity alone does not admit a complete rollout. Callers must also supply
explicit metadata and filesystem headroom. `max_capacity_bytes` in the
SoraFS configuration limits logical storage use; it is not physical free space.
The bounded current persisted store formats permit 64 MiB index, 64 MiB metadata
per manifest, and 16 MiB manifest files; conservative planning can charge both
old and temporary copies of those limits. This is deliberately different from
estimating serialized metadata size from one small fixture.

For a sparse VM disk, run a separate plan on its physical backing host, charging
full planned guest allocation growth plus host headroom. Guest free blocks do not
prove backing-host capacity. The backing plan counts backing files, not guest
inodes. A successful observation is not a reservation: deployment orchestration
checks before preparation and again immediately before authorization/apply.
Any concurrent storage writer can invalidate it. A later phase may use a reduced
plan only when its already-allocated postconditions are actually established.

Focused validation: `python3 scripts/tests/taira_disk_capacity_test.py` covers
cohost aggregation, inode exhaustion, sparse backing-host refusal, symlink
rejection, arithmetic bounds, required runtime allocations, exact copy topology,
and metadata-only reads.


## Derive the retained first-release footprint

`derive_capacity` constructs guest and physical backing plans from the actual
maintained build receipt and the public `taira.public-capacity-inputs.v1` and
`taira.public-runtime-capacity-inputs.v1` observations. The maintained
[`taira_retry.py` command](taira_retry.md) obtains these observations automatically
from the preceding native inventory. Only `full_inrou` needs the runtime
observation and current stage. It checks the three
small manifest hashes against native SF1 admission before using the 64 KiB chunk
minimum; it never decodes Norito in Python or rereads large payloads for hashing.

The derivation charges four daemon, four SoraFS and five CLI role copies; each
config uses the native 1 MiB output bound, and all four unit files are charged.
For `full_inrou`, it includes all four guest hydration,
writable root/data leases, ephemeral storage and bundle publication footprints.
Unknown stage or service-artifact layouts reject rather than produce a partial
budget. The backing plan includes full future guest growth plus 2 GiB beyond the
guest's own 2 GiB reserve. Both plans still require fresh filesystem evaluation.

The scope is never inferred from absent files. `inrou` is rejected as an old
spelling, an incomplete `full_inrou` budget fails, and a core budget with supplied
Inrou inputs fails. Both scopes retain the same source/build identity, complete
artifact-role, available-inode and physical backing checks.


## Routine updater binding and admission

The current `taira.runtime-deployment.v1` record requires `backing_ssh` (the
approved Mac SSH argument vector and pinned host-key records) and `backing_path`
(the canonical absolute VM backing directory), alongside its pinned guest route.
The updater rejects missing, partial, or unknown fields. Author the explicit
binding locally into a fresh record with the maintained command:

```sh
python3 scripts/taira_update.py --bind-backing-storage \
  --deployment /owner/source-deployment.json \
  --backing-route /owner/approved-mac-route.json \
  --backing-path /Users/administrator/apps/approved-vm \
  --output /owner/bound-deployment.json
```

The output parent must be owner-private. This command validates both route pins,
contacts no host, preserves the source record byte for byte, and refuses an
existing output or an already/partially bound source. Runtime update commands
consume only the complete current schema; they do not infer backing ownership.

Preparation checks Mac free space before contacting the guest, then evaluates
actual artifact sizes, file-block slack, evidence budgets and per-filesystem
guest headroom under the deployment locks. The final Mac check charges that full
guest growth plus its own 2 GiB reserve. Each artifact transfer rechecks remaining
guest allocation under the deployment lock before writing. Apply repeats Mac
and guest admission, then rechecks guest capacity under the operation lock before
creating its attempt and again before pausing services. Existing artifact copies
remain charged by the filesystem and are not counted as future apply growth.
Failures retain bounded public capacity receipts; no storage is deleted or reserved.

Offline updater coverage: `python3 -m pytest -q scripts/tests/taira_update_test.py`.
