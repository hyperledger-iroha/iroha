# Historical project records

[`status.md`](../../status.md) and [`roadmap.md`](../../roadmap.md) are concise
current views. This directory preserves the source evidence that formerly
accumulated in those roots. An archive is historical, including paragraphs once
labelled active or release-ready; it never attests the current candidate.

| Capture | Source and navigation |
| --- | --- |
| [2026-09-06](2026-09-06/index.md) | Exact dirty working copies, with dated subsystem pages, source hashes, reversible links and occurrence/order metadata. |
| [2026-09-07 Norito helper compaction](2026-09-07/norito-helper-compaction.json) | Exact retired source guard and original measurements; known preimages are verifiable, while missing postimage identities remain unverified. Current codec contracts use `scripts/check_norito_codec_contracts.py`; source-size budgets remain separate. |
| [2026-09-08 architecture merge checkpoints](2026-09-08/index.md) | Two distinct earlier dirty-status paragraphs, retained verbatim with source ranges and hashes; later current findings remain in status. |
| [2026-09-09 broker stack correction](2026-09-09/index.md) | Measured dispatcher frames, removed stack overrides, ordinary-stack regressions and separate concurrency/lint failures. |
| [2026-09-09 merge checkpoints](2026-09-09/merge-checkpoints.md) | Verbatim branch merge and privacy observations with source and excerpt hashes; qualification remains scoped to their recorded inputs. |
| [2026-09-10 Musubi SDK and resolver](2026-09-10/index.md) | Canonical account queries, persistence identities and ordinary-stack resolver repair, with passing and failed source-bound checks retained. |
| [2026-09-11 JSON context and primitives](2026-09-11/index.md) | Canonical primitive APIs and ordinary iterative Value destruction, complete compiler UI execution and benchmark consumer qualification; source and release limits remain explicit. |

The [current roadmap coverage map](current-roadmap-coverage.json) accounts for
every original roadmap area using stable current outcome IDs. It is maintained
with the current roadmap, outside the immutable capture. Consolidation neither
completes an obligation nor promotes a historical checkpoint into release proof.

## Capture and verification

`scripts/archive_project_history.py` reads the actual UTF-8 working files; it
does not substitute `HEAD`. It splits headings and unindented evidence-list
items outside backtick/tilde code fences, retaining byte ranges, original line
numbers and complete heading context. Exact identical records share one body;
source-local fragment/reference meaning remains distinct. Dates come from
original headings/items; records without a date are explicitly `undated`.

Pages are grouped by subsystem and event date, targeting 500 lines. An original
indivisible section can exceed that target; it is retained intact rather than
truncated. The subsystem assignment is navigation, not a change to provenance.
The readable `manifest.json` records original SHA-256, byte/line counts and every
page hash. Its authenticated `record-inventory.json.gz` is deterministic gzip of
UTF-8 JSON containing all records, occurrences and reversible link edits.
Compression removes metadata repetition; it does not omit measured evidence.

Supported inline Markdown, reference-definition and HTML link destinations are
rewritten relative to their original source. Defined reference uses are expanded
so their scope survives a page split; code remains unchanged. Original root
fragments target stable archive record anchors. The original-link audit records
pre-existing missing paths/root fragments; external URLs and fragments in other
files are not network-checked. Verification reproduces canonical rewrites and
reconstructs both originals byte-for-byte before accepting an archive.

```sh
python3 scripts/archive_project_history.py verify --archive docs/history/2026-09-06 --check-current
python3 scripts/archive_project_history.py reconstruct --archive docs/history/2026-09-06 --output-dir /tmp/iroha-original-records
```

Reconstruction requires a new output directory and cannot overwrite the roots.
For a future capture, use a new dated archive path with `capture --date YYYY-MM-DD`.
Prepare both current views separately, then use `replace --status-view PATH
--roadmap-view PATH`. Replacement verifies both original hashes before staging,
after staging and immediately before each rename. A detected concurrent update
aborts; coordinate ownership of both root files for the short replacement window.
The verifier rejects schema drift, missing/extra files, invalid paths/symlinks,
changed hashes, overlapping bodies, incorrect occurrences and noncanonical link
metadata. Both current roots remain limited to 300 lines.
