# Prepared Taira transfer

`scripts/taira_release_transfer.py` imports the four executables from a completed
`taira_release.py prepare` result, its public qualification evidence and the exact signed Git source into fresh,
inactive custody on the approved MacStadium Dublin Linux guest. It runs no
validator, deployment, reset, service, transaction or activation command.

Prerequisites: Python 3.11+, Git and GnuPG locally; AArch64 Linux root with Python,
Git and GnuPG on the guest; an existing guest-owned mode0700 runtime directory;
and the explicitly approved, pinned guest and Mac backing-host SSH routes. The
local evidence parent must be an existing mode0700 directory with safe ancestors.
The driver and all its controller dependencies must match the same signed commit
as the completed preparation. Use the full GPG signing-key fingerprint. The
source owner carries only the exported public key and verifies the actual commit
signature again in an isolated keyring on import.

Run from the canonical `optimizations` checkout:

```sh
python3 -B scripts/taira_release_transfer.py \
  --repo-root /Users/takemiyamakoto/dev/iroha \
  --plan /absolute/private/transfer-plan.json \
  --output-dir /absolute/private/fresh-transfer-evidence
```

The owner-private mode0600 plan has exactly these fields:

| Field | Required value |
| --- | --- |
| `schema` | `taira.release-transfer.v1` |
| `provider` | `macstadium-dublin` |
| `preparation` | `{ "path": "/absolute/preparation/result.json", "sha256": "actual SHA256 of that file" }` |
| `expected_commit` | Full 40-character signed Git commit |
| `expected_signer` | Full uppercase GPG signing-key fingerprint |
| `runtime_root` | Existing approved private guest runtime directory |
| `backing_path` | Explicit approved Mac directory containing the guest disk |
| `guest_ssh`, `backing_ssh` | Exact pinned route objects documented for [Taira retry](taira_retry.md), ending in `/usr/bin/python3 -I -` |

Routes require explicit identity paths, strict host-key checks, the exact pinned
public known-host files, no ambient SSH configuration, and no agent forwarding.
The driver validates that fixed route, then substitutes only its own constant
Python bootstrap command to carry a bounded binary stream. It accepts no remote
command from the plan. SSH handles its authentication; the driver reads no
runtime private key, token, password or validator configuration.

Before remote I/O, the command verifies the signed controller sources, the exact
preparation request/result/capture, its native qualification checkpoint, the
frozen source snapshot, and all four captured AArch64 ELF hashes. The original
mode0500 captures are retained unchanged. The signed-source owner exports only
the selected commit and complete tree/object closure, excluding parent history,
working-tree changes and gitlink contents.

The shared capacity checker first probes the Mac backing host, then the guest's
actual filesystem, then rechecks the Mac against the guest's full allocation
bound. The receiver rechecks guest capacity under its private import lock before
writing payloads. Capacity observations are not reservations. Required space
includes retained transport files, materialized source and Git objects, metadata,
filesystem allocation slack and operating headroom; nothing is deleted.

The guest creates `release-import-COMMIT-RESULT_SHA256` under `runtime_root`.
Executables are independent, single-link mode0755 files in `artifacts/bin`.
`source/source` is the verified, clean, shallow signed source artifact. Transport
files remain in mode0600 `source/source.pack` (the retained-import retirement
contract) and mode0400 `source/source-capture.json`. The same import carries
mode0400 `preparation/result.json`, `preparation/request.json`,
`preparation/checks.json` and `preparation/capture.json`; the latter is the exact
completed attempt capture and must match the result bytes. These files retain
the original producer records without rewriting their local build paths. The
closed stream requires all ten payloads; a six-payload import is incomplete.
Their bytes, file entries and parent directory are included in capacity admission.
No separate proof upload is required. No tar
extraction is used. Each stream length and SHA256 is checked before publication.

Only after revalidating every file and the actual Git signature does the receiver
publish `artifacts/verified-manifest.json`, `source/verified-manifest.json` and
`completed.json`. Local `binary-transfer.json`, `source-transfer.json` and
`completed.json` retain the exact returned receipts and remote receipt pins.
`activated` remains false; these are import observations, not deployment success.

Repeating the same plan with a fresh local evidence directory revalidates the
completed guest import without overwriting it. It still authenticates the
incoming stream. A changed request, corrupted existing file, unexpected entry,
partial import, hardlink, symlink substitution or unsafe mode fails closed.
Partial files and logs remain for diagnosis; this command does not repair or
adopt them. Keep failure evidence and select a separately reviewed destination
only after resolving the cause.

Next use the same-revision guest `iroha taira public-reset source-manifest` and
native public-input/configuration, supervisor-plan and inventory assembly
commands documented in [Taira release](taira_release.md). Native assembly derives
its own pins from the actual imported files. The transfer receipts can later be
consumed by the unchanged-artifact retry controller once a matching native
inventory exists; transfer does not construct that inventory or authorize a reset.

Offline checks (no SSH or Cargo):

```sh
python3 -B -m unittest discover -s pytests/scripts -p 'test_taira_release_transfer.py'
python3 -B -m unittest discover -s pytests/scripts -p 'test_taira_source_capture.py'
```
