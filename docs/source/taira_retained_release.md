# Archive and retire inactive Taira release binaries

`scripts/taira_retained_release.py` owns one operation: preserve explicitly
selected inactive public executables off-host, then retire those exact guest
files. It neither deploys nor resets validators. It reads pinned public
inventories, native terminal records, and source/binary transfer receipts to
establish ownership, and preserves those records. It does not read or remove
source payloads, transport packs, runtime configuration, keys, ledger data, or
currently bound executables. Canonical local Git history supplies signature
verification and remains unchanged.

Run from the canonical `optimizations` checkout. Archive creation authenticates
the controller and its three shared modules against the plan's signed commit
and full signer fingerprint. Resume and retirement require an explicit signed
execution-controller commit and signer; the current four-module closure must
match that commit. The immutable archive plan retains its original controller,
whose signature and full signer are verified independently and whose commit
must be an ancestor of the execution controller under the same signer. This
separates archive provenance from current execution without accepting another
archive schema or weakening native ownership records. Both SSH routes must pass
`taira_retry.validate_ssh` and match the
pinned MacStadium bound-deployment record. Python 3.11 or later and Git/GPG are
required locally. The guest must be root on AArch64 Linux; the backing observer
runs on the approved Mac. Apple Python 3.9 supports the backing bootstrap.

## Explicit plan

The UTF-8 JSON plan has exactly these fields:

| Field | Required value |
| --- | --- |
| `schema` | `taira.retained-public-release.v1` |
| `provider` | `macstadium-dublin` |
| `controller` | Full `commit` and uppercase full `signer` fingerprint |
| `guest_ssh`, `backing_ssh` | Existing strict route objects, including known-host pins |
| `backing_path` | Exact bound VM backing directory |
| `deployment` | Local bound-deployment `{path, sha256}` |
| `current_inventory` | Guest current `assemblyN/inventory.json` `{path, sha256}` |
| `units` | Four ordered canonical validator unit `{path, sha256}` references |
| `releases` | One to twelve explicit release descriptors, below |

Each release descriptor contains exactly `inventory`, `terminal`,
`binary_manifest`, and `source_manifest`, each an absolute `{path, sha256}`
reference. These are respectively the public assembly inventory, native
`journal-v1/rolled-back/<authorization_sha256>.json`, release
`verified-manifest.json`, and source-transfer `verified-manifest.json`.
No glob, inferred release range, private config path, or source-tree deletion
is accepted. The native rolled-back record must satisfy the current native
contract; older incompatible records are ineligible.

The owner joins native runtime roles for `iroha3d_taira`, `iroha`, and
`sorafs-node` to the completed four-artifact binary receipt. `kagami` is the
fourth receipt artifact, rather than an invented native runtime role. Source
receipts establish the signed commit/tree join; source payloads remain unread.
Every bin directory has exactly these four public files before archive.

## Two phases

Use absolute paths inside an existing private directory. The plan must be
mode `0600`. Output directories must be fresh; interrupted outputs are retained.

```sh
python3 scripts/taira_retained_release.py --repo-root "$PWD" archive \
  --plan /absolute/private/retained-plan.json \
  --output-dir /absolute/private/retained-archive

python3 scripts/taira_retained_release.py verify \
  --archive-dir /absolute/private/retained-archive

python3 scripts/taira_retained_release.py --repo-root "$PWD" retire \
  --execution-controller-commit "$FULL_SIGNED_EXECUTION_COMMIT" \
  --execution-controller-signer "$FULL_SIGNER_FINGERPRINT" \
  --archive-dir /absolute/private/retained-archive \
  --output-dir /absolute/private/retirement-evidence
```

Archive streams held guest descriptors directly into exclusive local files.
It does not create a guest archive or copy executables on the guest. Local
objects are rehashed after writing, synchronized, made read-only, and retained
with their original mode in the admission record. `completed.json` is published
only after the entire source stream and authority revalidation succeed.

Retirement rehashes the completed archive and holds its descriptors through
capacity probes and retirement. Original plan, admission and completion hashes
are pinned alongside every payload; an internally valid replacement archive
cannot substitute for the one originally verified. The operation records both
controller identities and the execution module hashes in separate evidence.
It borrows only existing updater/native reset locks and creates
its own private `retained-public-release-v1` intent directory. Missing epoch
authority is never created. Those advisory locks are not represented as a
continuous fence against every supervisor creator.

Before any unlink, all selected binaries are renamed exclusively to exact
same-directory quarantine names recorded in the durable intent. Current
inventory/unit bindings, supervisor absence, process descriptors/maps and
mount references are freshly checked after that barrier and before each
unlink. Only the owner's admitted custody descriptors are exempted. Original
names reappearing, changed bytes/inodes/parents, added links, live references,
or new supervisor authority stop the operation and preserve unresolved
quarantine files. Each unlink has a durable exact intent and a synchronized
parent. Records publish through exclusive same-directory pending files and a
no-clobber rename; an interrupted pending write can resume only when its
single-link, owned bytes are an exact prefix of the expected record. Changed
or foreign pending records are retained and rejected. Completion records
cannot authorize deployment.

## Capacity and interruptions

The off-host archive requires all materialized binary bytes, bounded records,
directory allocation, and a 256 MiB operating reserve. Guest retirement charges
only its bounded serialized intent, progress receipts, simultaneous publication
copies, directory allocation, and a separate fixed 32 MiB operating reserve.
The binary-retirement backing operation charges that entire guest demand plus
a fixed 32 MiB physical operating reserve. Each reserve exceeds the enforced
maximum metadata peak: 26,435,428 bytes with 4 KiB allocation units, 48 files
and an 8 MiB intent. Actual allocation-unit rounding is always included; a
rounded metadata peak above 32 MiB is refused. The shared archive default and
source-retirement policies retain their own 256 MiB reserves. There are at
most 48 files, 8 MiB per record, 4 GiB per binary, and 32 GiB total. Expected
reclamation is never credited during admission. Deployment's existing 2 GiB
guest plus 2 GiB backing reserves are unchanged.

An interrupted archive cannot authorize retirement. Resume preserves its exact
metadata and original diagnostics, verifies every completed object, and accepts
only a contiguous object prefix with at most one final `0600` partial object.
Its size can be zero or the complete expected size if interruption occurred
before sealing. The fresh remote held source must match the partial prefix hash
before any missing bytes are sent. Only missing bytes are copied; completed
objects are held unchanged. Full remote source revalidation, full local object
rereads and synchronization are required before publishing the existing
completion record. Missing middle objects, later objects after a partial file,
changed source identities, corrupt prefixes or replaced metadata fail closed.
Resume diagnostics and controller evidence use a fresh directory outside the
original archive:

```sh
python3 scripts/taira_retained_release.py --repo-root "$PWD" resume-archive \
  --execution-controller-commit "$FULL_SIGNED_EXECUTION_COMMIT" \
  --execution-controller-signer "$FULL_SIGNER_FINGERPRINT" \
  --archive-dir /absolute/private/retained-archive \
  --output-dir /absolute/private/archive-resume-evidence
```

For interrupted retirement, rerun `retire` with the same complete
archive and a fresh evidence output. The owner validates its original intent,
quarantine identities, and per-file deletion intents before resuming. An absent
file without the owner's durable deletion intent is an error. A completed
repeat performs no further deletion. Keep the off-host archive: it is the
restoration source, and restoration is an explicit separately reviewed action.

The result reports guest allocated bytes removed, not physical Mac capacity
recovered. After publishing the durable retirement receipt, the owner verifies
the runtime remains on the approved guest root filesystem, runs supported
`sync -f` and `fstrim`, and records fresh guest and Mac backing observations
through the pinned routes. A failed trim is recorded as a failure and does not
claim physical recovery. Deployment must still pass its own fresh capacity
gate. This owner does not truncate logs, alter services, or stop the VM.

Run the offline custody regression suite with:

```sh
python3 -m unittest discover -s pytests/scripts -p 'test_taira_retained_release*.py'
```
