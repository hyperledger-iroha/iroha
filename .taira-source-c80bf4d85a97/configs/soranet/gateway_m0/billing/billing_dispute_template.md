# SoraGlobal Gateway Dispute Form

Period: 2026-11
Tenant: <tenant>
Invoice ID: <billing invoice id>
Submission timestamp: <UTC ISO8601>

Meters under dispute:
- meter_id: <http.request|http.egress_gib|dns.doh_query|waf.decision|car.verification_ms|storage.gib_month>
window: <start..end UTC>
expected_value: <number>
observed_value: <number>
evidence: <path to logs/artefacts>

Requested outcome: <credit memo | refund | reversal>
Impact summary: <short text>

Notes:
- Include Alertmanager extracts if caps/thresholds were crossed.
- Disputes reserve 20% of the net receivable in escrow until resolved.
