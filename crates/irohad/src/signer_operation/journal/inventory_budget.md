# Private signer journal resource accounting

`inventory_pool.rs` admits each phase before its filesystem work. The configured
pool is shared across receipt purposes. Credits are local capacity, never signing
permission or consensus authority. `PrivateDirectory` supplies native custody;
committed signer operation state supplies replay and rollback authority.

The single journal implementation uses exact canonical absolute paths on Unix and
Windows. Ancestor handles remain pinned. A persistent empty private control file
holds the exclusive ownership lock; it is validated separately and does not consume
a receipt slot. Sealed files expose bounded reads and seeks, with no writable-file
or raw-handle accessor. Failed receipt creation leaves its final-name tombstone;
interrupted pending-Reserve publication leaves its original in-progress identity.
Existing records are never hardened, replaced, removed, or retried under a new ID.

## Filesystem work

Let `d` be retained ancestor count, including the root (`2..=65`), and `E` be
examined inventory entries. A probe is one native enumeration, metadata, open, ACL-fetch, or
synchronization operation. Native in-memory ACL parsing is included in the resident
allowance. This is a bound on logical filesystem work, not physical storage bytes
or elapsed time. The platform owner documents the underlying recurrences in
`crates/iroha_fs`.

The journal takes the maximum portable coefficient for each primitive. The following
phase bounds include all nested lineage checks; byte I/O is additionally bounded by
the selected receipt profile. The opaque writer/reader delegate bounded `Write`, `Read` and
`SeekFrom::Start` directly to their retained descriptor; they add no hidden lineage
revalidation. `SeekFrom::End` adds one bounded-length observation, but the journal
never uses that mode. Strict open and sealing each add one extent observation;
these are included below. Interrupted-call retries and hidden kernel work are not
additional owner-level credits.

| Phase | Probe ceiling | Included work |
| --- | ---: | --- |
| Open | `68d + 20` | Exact ancestor open, control-file open/sync, lock acquisition/identity, initial lineage validation |
| Lineage validation | `36d + 9` | Two directory checks, named control open, both identities and lengths |
| Stable read | `72d + 49` | Two strict retained snapshots and one bounded length observation |
| Pinned recheck | `144d + 67` | Two lineage validations and the stable read |
| Create and seal | `81d + 47` | Durable no-replace creation, bounded extent check and native read-only seal |
| Recover before final recheck | `143d + 80` | Lineage check, strict bounded open and stable read |
| Pending publication before final recheck | `198d + 128` | Durable creation, seal, stable read and retained no-replace rename |
| Complete inventory | `8E + 92d + 34` | Streaming native inventory plus two journal lineage checks |

One conservative inspection reservation (`198d + 128`) covers create, recover or
recheck. Create/recover release it before obtaining the final recheck reservation;
there is no double charge for sequential work. The scan reserves
`E = max_records + 2`: all receipt slots, its control entry and the first excessive
entry that terminates a scan. Inventory does not collect all filenames or reopen
all ancestors for every entry. Pending Reserve additionally reserves a bounded
operation-ID vector to reject final/in-progress duplicates.

## Resident bytes and handles

An opened journal reserves `2 * path_bytes * d + 256d + 4096` requested bytes for
retained full-prefix paths, shared links, the exact preflight and owner state. The
maximum path is 4096 encoded bytes with 64 components. Temporary preflight names
are dropped before retained filesystem construction. Opening, inspection, and scans
each separately reserve 256 KiB for native ACL/security descriptors, directory
buffers, names and temporary metadata. Native ACL length has a 16-bit bound.

A pinned file reserves its complete profile byte ceiling plus `16d + 2048` for its
shared ancestor-pointer vector, name and owner. Its exact retained byte buffer stays
charged until the last receipt drops. A reread reserves a second complete profile
buffer until comparison finishes. A pending scan reserves another 128 bytes per
operation ID, bounded at 4096 IDs. Receipt scans retain no per-record collection.
The largest supported pending receipt remains 136 KiB; production receipt ceilings
are the existing purpose-owned constants. The aggregate disk ceiling remains 64 MiB
and the regular receipt count remains 65,536.

Persistent handles are `d + 1` (ancestors and control lock), plus one per retained
file. Every active opening/scan/inspection phase reserves four transient handles;
shared ancestor links never duplicate their native handles. Concurrent phases must
independently fit all three credits or return `Capacity`, refunding any partial
acquisition without filesystem access for the refused phase.

The configuration minimum is 2 MiB requested resident bytes, 530,318 probes
(`8 * 65,538 + 92 * 65 + 34`) and 71 handles (`65 + 1 + 1 + 4`). Resident examples
at the deepest allowed path are 553,216 retained-path bytes, 786,432 pending-scan
bytes, 142,352 pinned-pending bytes and 401,408 reread bytes. Every individual
supported phase fits the 2 MiB minimum; overlapping phases remain subject to their
independent handle/probe ceilings. The unchanged resident default is 16 MiB and
handle default is 1024. The probe default is 1,000,000 so one full portable inventory
fits; the prior 300,000 default could not fund its actual native metadata work.

Exact and one-below tests cover configuration minima and runtime acquisitions.
Real-filesystem tests cover immutable recovery, process exclusion, partial writes,
no-replace publication, lock substitution and original Unix mode/link/ancestor
attacks. Passing a host test suite does not qualify Windows runtime packaging or
release behavior; those gates require the native platform candidate.
