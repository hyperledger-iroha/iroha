---
lang: ka
direction: ltr
source: docs/source/soranet_gateway_bug_bounty.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ef770782a60b646faacc449ab09a1c09311b6e5201311da4e4f959f2d917b575
source_last_modified: "2025-12-29T18:16:36.205724+00:00"
translation_last_reviewed: 2026-02-07
---

# SNNet-15H1 — Pen-test & Bug Bounty Kit

Use `cargo xtask soranet-bug-bounty` to generate a repeatable packet for the
SoraGlobal Gateway CDN bug bounty program. The helper validates that the
configuration covers edge, control-plane, and billing surfaces, then emits:

- `bug_bounty_overview.md` — owners, partners, scope/cadence, SLA, rewards, and links to dashboards/policy.
- `triage_checklist.md` — intake channels, duplication policy, evidence requirements, playbook, and per-surface checkpoints.
- `remediation_template.md` — deterministic report template with disclosure window and evidence placeholders.
- `bug_bounty_summary.json` — Norito JSON summary (paths relative to output dir) for governance packets.

## Usage

```
cargo xtask soranet-bug-bounty \
  --config fixtures/soranet_bug_bounty/sample_plan.json \
  --output-dir artifacts/soranet/gateway/bug_bounty/snnet-15h1
```

Options:
- `--config <path>`: Norito JSON plan describing owners, partners, scope, SLA, triage, rewards, and reporting. Required.
- `--output-dir <path>`: Destination directory. Defaults to `artifacts/soranet/gateway/bug_bounty`.

Scope guardrails:
- Required areas: `edge`, `control-plane`, and `billing`. The command fails fast if any are missing or targets are empty.
- Cadence must be non-empty per area.

## Evidence bundle shape
- Summary fields (`program`, `slug`, owners, partners, scope, SLA, rewards, triage, reporting) are written to JSON alongside relative output paths.
- Markdown files carry the generation timestamp for audit trails.
- The remediation template mirrors the disclosure window from the config so downstream uploads stay aligned with the public policy.

## Determinism
- All content is generated from the provided config; no network calls are made.
- File names are fixed within the output directory to keep CI snapshots and governance bundles stable.
