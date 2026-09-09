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

For the current fresh deployment with coordinator and four validators on one
guest, the complete rollout topology is **3A + 2S + 4P + 4R + explicit headroom**:

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

`cohost_peak_plan()` expresses that topology, while `allocation_bound()` bounds
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
from the preceding native inventory and its current stage. It checks the three
small manifest hashes against native SF1 admission before using the 64 KiB chunk
minimum; it never decodes Norito in Python or rereads large payloads for hashing.

The derivation charges four daemon, four SoraFS and five CLI role copies; each
config uses the native 1 MiB output bound. It includes all four guest hydration,
writable root/data leases, ephemeral storage and bundle publication footprints.
Unknown stage or service-artifact layouts reject rather than produce a partial
budget. The backing plan includes full future guest growth plus 2 GiB beyond the
guest's own 2 GiB reserve. Both plans still require fresh filesystem evaluation.
