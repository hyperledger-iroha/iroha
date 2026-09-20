# Retained public source custody

`scripts/taira_retained_source.py` archives and, in a separate explicit action,
retires one pinned inactive public source from releases85–93. Its authority is the
actual compatible native closed rollback, signed Git commit/tree, completed source
transfer and native source closure. It does not manufacture a terminal-custody
receipt or admit releases82–84. It grants no deployment or ledger authority.

The closed owner0600 plan extends the retained-binary plan with schema
`taira.retained-public-source.v1`, exactly one `releases` descriptor, and:

| Field | Meaning |
| --- | --- |
| `source` | Exact `root`, `commit`, `tree`, `signer`, and `pack` `{size, sha256}` |
| `source_closure` | Native public closure `{path, sha256}` from the pinned inventory |
| `git_controls` | Ordered `{name, size, sha256}` pins for HEAD, ORIG_HEAD, config, shallow, branch ref and two reflogs |

The original inventory, terminal, binary/source receipts, source closure, current
assembly94, bound current deployment, configs, secrets, ledger and native history
remain protected. The selected source must be exactly
`/opt/iroha/taira-source-releaseNUMBER-COMMIT`. No discovery glob selects a target.

The local signed controller authenticates the common custody, retry and source
helpers and this owner. Local canonical Git verifies the historical signature and
complete object closure. The guest classifies the full namespace before reading
payloads; it then checks every tracked blob, absent-relative symlink, empty
gitlink, exact Git pack/object set, index and reverse index. Actual public import
control bytes are explicitly pinned and restricted to inert settings and closed
import reflogs. Extra files, objects, index extensions or payload bytes fail.
This archive grammar does not change new source-import or deployment admission.

An existing pack is authenticated as the exact receipt-pinned byte sequence: its
bounded size, SHA256, PACK v2 object count and SHA1 trailer must match. Its strict
index/reverse-index census, Git verification and complete canonical signed object
inventory must also match, including every object type, size and SHA256. Git may
represent that same retained object closure using deltas. Retirement does not
decode those deltas itself or require the current shipping producer's encoding;
new source capture and import still require their canonical delta-free packs.

```sh
python3 -B scripts/taira_retained_source.py archive \
  --plan /absolute/private/source-plan.json \
  --output-dir /absolute/private/fresh-source-archive
python3 -B scripts/taira_retained_source.py verify \
  --archive-dir /absolute/private/fresh-source-archive
python3 -B scripts/taira_retained_source.py retire \
  --archive-dir /absolute/private/fresh-source-archive \
  --output-dir /absolute/private/fresh-retirement-evidence
```

Archive reads the guest only. It retains actual public file bytes and symlink
text in one indexed `payload.bin`, verifies every segment off-host, and publishes
completion only after the guest's final revalidation. Original metadata and
relative paths remain in `admission.json`. Complete archives must be retained;
the owner exposes no extraction or restore command. Interrupted archive output is
preserved and cannot authorize retirement.

The source stream uses only `zlib-chunks-v1`: each zlib member has an eight-byte
big-endian expanded/compressed length header, at most 1 MiB expanded and at most
expanded size plus 1 KiB compressed. The receiver bounds decompression, requires
exact length and member termination, rejects trailing bytes, and keeps the same
absolute stream deadline through the final admission frame and EOF. Both ends
come from the same signed controller. Compression changes transport only;
`payload.bin`, its indexed offsets, hashes, receipt schema and uncompressed disk
capacity admission remain unchanged. There is no raw-stream fallback.

Retirement holds the exact initial off-host archive through dispatch, borrows only
existing update/reset locks and uses a separate source-custody lock. Supervisor
absence is revalidated, not represented as continuously fenced authority. Current
bindings, process descriptors, inode aliases and mount namespaces are checked
before and after whole-root exclusive quarantine and between bounded deletion
batches. Each member is removed only after an exact inode/content/parent check.
Immutable batch intents permit only a contiguous deletion prefix on resume;
unexpected members, replacement ancestors or original-name reappearance stop work
and retain the archive/quarantine.

Capacity charges actual admission/intent publication overlap and bounded progress
records, filesystem slack, and separate256MiB guest/physical operating reserves.
Deployment headroom remains unchanged. Supported sync/TRIM and fresh guest/Mac
observations follow retirement; anticipated freed blocks never count as capacity
admission and guest removed-byte totals do not claim physical backing reclaim.

Focused offline checks, using isolated signed Git fixtures without SSH or Cargo:

```sh
python3 -B -m unittest discover -s pytests/scripts -p 'test_taira_retained_source.py'
```
