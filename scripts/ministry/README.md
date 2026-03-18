# Ministry Automation Helpers

This directory hosts helper scripts referenced by `docs/source/ministry/transparency_plan.md`.

- `dp_sanitizer.py` — convenience wrapper that calls
  `cargo xtask ministry-transparency sanitize` with the arguments required to
  generate the sanitized metrics JSON (`sanitized_metrics.json`) and the DP
  audit report (`dp_report.json`). Run `./dp_sanitizer.py --help` for usage.
- `transparency_release.py` — creates `checksums.sha256` plus
  `transparency_manifest.json`, ensuring every artefact (ingest snapshot,
  metrics, sanitized outputs, summaries) has a SHA-256 digest captured before
  uploading to SoraFS and anchoring the governance vote. Pass either
  `--sorafs-cid` or `--sorafs-summary` (pointing at a `sorafs_cli car pack`
  or `proof verify` summary) to populate the bundle CID, and the script will
  emit `transparency_release_action.json` with the `TransparencyReleaseV1`
  governance payload (including the manifest digest and dashboards git SHA).
- `transparency_release.py --governance-dir <path>` additionally runs
  `cargo xtask ministry-transparency anchor` so the `.to` payload and JSON
  summary are written into the governance DAG directory automatically.
  When `--governance-dir` is provided, the script now also writes a publisher
  head-update request under
  `<governance-dir>/publisher/head_requests/ministry_transparency/`. The file
  (`MinistryTransparencyHeadUpdateV1`) records the quarter, SoraFS CID, IPNS key
  alias (defaults to `ministry-transparency.latest`), manifest paths, and the
  generated governance action so the publisher service can update the IPNS head
  without manual intervention. Pass `--auto-head-update` alongside
  `--governance-dir` to immediately process the emitted request through
  `publisher_head_updater.py`; use `--head-update-ipns-template` to forward a
  command such as `"/usr/local/bin/ipfs name publish --key {ipns_key} /ipfs/{cid}"`
  and `--head-update-dry-run` when the queue/log/state should be left untouched.
  Pass `--skip-anchor` for dry runs where building the `cargo xtask` binary is
  undesirable; the governance artefacts will be left untouched but the
  head-request JSON (and optional auto-processing) will still run.
- `publisher_head_updater.py` — consumes the pending head-request files,
  optionally runs an IPNS publishing command template (example:
  `/bin/echo publish {ipns_key} {cid}`), records the resolved head state under
  `<governance-dir>/publisher/ipns_heads/`, appends `head_updates.log`, and moves
  processed requests into `<queue>/processed`. Pass `--dry-run` to verify queue
  entries without mutating state, and use `--ipns-template` to integrate with
  `ipfs name publish` or any other publisher tooling.
- `check_transparency_release.py` — validates a release directory by recomputing
  the SHA-256 digests listed in `checksums.sha256`, ensuring the manifest matches
  those digests, and confirming the governance payload references the correct
  quarter/CID. CI runs it via `ci/check_ministry_transparency.sh` so drift is
  caught before artefacts are published.
- `scaffold_red_team_drill.py` — scaffolds red-team drill artefacts referenced by
  `docs/source/ministry/moderation_red_team_plan.md`, creating the report stub
  from the template plus the evidence directory layout under
  `artifacts/ministry/red-team/<YYYY-MM>/<slug>/`.
- `check_red_team_reports.py` — scans every committed red-team report and fails
  if template placeholders remain. CI invokes it via
  `ci/check_ministry_red_team.sh` so governance-ready artefacts never ship with
  incomplete metadata.
- `export_red_team_evidence.py` — copies dashboards/logs/manifests/evidence for
  a given scenario id into the canonical directory layout (see
  `docs/source/ministry/moderation_red_team_plan.md`) and writes an
  `evidence_manifest.json` describing every file (category, source, SHA-256).
  Supports Grafana API exports via `grafana:<uid>` specs when `--grafana-url`
  (and optionally `--grafana-token` or `GRAFANA_API_URL`/`GRAFANA_API_TOKEN`)
  are configured, allowing drill leads to archive the exact dashboard snapshots
  referenced in reports without manual copy/paste.
- `moderation_payload_tool.py` — command-line helper that bundles red-team
  payload fixtures (with SHA-256 manifest) and stages denylist patches copied
  from `docs/source/sorafs_gateway_denylist_sample.json`, keeping evidence
  directories consistent across drills.
- `cargo xtask ministry-transparency anchor --action <path> --governance-dir <dir>` —
  consumes the `transparency_release_action.json` emitted by the Python helper,
  encodes a canonical `TransparencyReleaseV1` Norito artefact, and drops both the
  `.to` payload and the JSON summary into the configured governance DAG directory
  (e.g., `$GOVERNANCE_DAG_DIR/ministry/releases/<quarter>/`). Point Torii’s
  `sorafs_por.governance_dag_dir` (or the ministry publisher) at the same path
  so the builder/publisher pipeline can ingest the release automatically.

All scripts assume they are run from the workspace root (or that `cargo` can
find the workspace through `CARGO_MANIFEST_DIR`). Use Python 3.11+ to match CI.
