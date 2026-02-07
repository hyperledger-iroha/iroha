---
lang: hy
direction: ltr
source: docs/examples/soranet_testnet_operator_kit/05-incident-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d2fbce156952c669e73d74c13284fca317013d706ee401359028c3638341d34b
source_last_modified: "2025-12-29T18:16:35.091815+00:00"
translation_last_reviewed: 2026-02-07
---

# Brownout / Downgrade Response Playbook

1. **Detect**
   - Alert `soranet_privacy_circuit_events_total{kind="downgrade"}` fires or
     brownout webhook triggers from governance.
   - Confirm via `kubectl logs soranet-relay` or systemd journal within 5 mins.

2. **Stabilise**
   - Freeze guard rotation (`relay guard-rotation disable --ttl 30m`).
   - Enable direct-only override for affected clients
     (`sorafs fetch --transport-policy direct-only --write-mode read-only`).
   - Capture current compliance config hash (`sha256sum compliance.toml`).

3. **Diagnose**
   - Collect latest directory snapshot and relay metrics bundle:
     `soranet-relay support-bundle --output /tmp/bundle.tgz`.
   - Note PoW queue depth, throttle counters, and GAR category spikes.
   - Identify whether PQ deficit, compliance override, or relay failure caused the event.

4. **Escalate**
   - Notify the governance bridge (`#soranet-incident`) with summary and bundle hash.
   - Open incident ticket linking to the alert, including timestamps and mitigation steps.

5. **Recover**
   - Once root cause addressed, re-enable rotation
     (`relay guard-rotation enable`) and revert direct-only overrides.
   - Monitor KPIs for 30 minutes; ensure no new brownouts appear.

6. **Postmortem**
   - Submit incident report within 48 hours using governance template.
   - Update runbooks if new failure mode discovered.
