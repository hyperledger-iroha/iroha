# SNNet-15F1 Chaos & GameDay Plan

- **Pop:** `soranet-pop-m0`
- **Harness:** `cargo xtask soranet-gateway-chaos --pop soranet-pop-m0 --config chaos_scenarios.json --out artifacts/soranet/gateway_chaos`
- **Artefacts:** `chaos_runbook.md`, `gameday_schedule.json`, `chaos_plan.md`, `chaos_metrics.json`, `chaos_injector.sh`, harness outputs under `artifacts/soranet/gateway_chaos/`.

- **Evidence:** Keep Grafana screenshots (cache hit, resolver proof, latency), Alertmanager exports, and the JSON/Markdown outputs from the harness/injector.
- **Logging:** Append every drill to `ops/drill-log.md` using `scripts/telemetry/log_sorafs_drill.sh --scenario <id> --status <pass|fail|follow-up>`.
- **Rollback:** Every inject step must include the rollback command in the injector notes; operators should practice it even when running in simulate mode.

## Quarterly schedule
- 2025-Q4 → prefix-withdrawal
- 2026-Q1 → trustless-verifier-failure
- 2026-Q2 → resolver-brownout
- 2026-Q3 → prefix-withdrawal, trustless-verifier-failure, resolver-brownout

## Scenarios

### Prefix withdrawal / failover — prefix-withdrawal

Alert: `BgpSessionFlap` · Alert budget: 300s · Recovery budget: 900s · Backlog cap: 10%

Withdraw one upstream prefix, validate alerting, and prove cache + resolver health before restore.

**Inject steps:**
- Withdraw primary prefix (120s) — `echo simulate prefix withdraw for soranet-pop-m0`
- Observe scrape loss (60s) — `echo watch GatewayScrapeMissing alert fire`

**Verification:**
- Verify cache hit floor (60s) — `echo ensure cache hit rate floor stays above 0.89`
- Check backlog gauges (60s) — `echo review hedging/backlog gauges for spillover`

**Remediation:**
- Restore prefix (120s) — `echo restore primary prefix and clear drains`
- Confirm alert recovery (120s) — `echo ensure GatewayScrapeMissing clears`

---

### Trustless verifier stalled pipeline — trustless-verifier-failure

Alert: `GatewayVerifierStall` · Alert budget: 240s · Recovery budget: 600s · Backlog cap: 5%

Stop the trustless verifier path to force cache binding failures and prove quarantine + alerting.

**Inject steps:**
- Pause verifier process (60s) — `echo pause verifier service to trigger stall`
- Send probe (60s) — `echo run probe fetch against gateway`

**Verification:**
- Check cache binding alerts (60s) — `echo confirm moderation/cache-version alerts raised`
- Inspect quarantine counters (60s) — `echo inspect quarantine counters for stalled receipts`

**Remediation:**
- Resume verifier (120s) — `echo resume verifier service and replay receipts`
- Validate backpressure cleared (120s) — `echo confirm backlog gauges below 5%`

---

### Resolver RAD sync brownout — resolver-brownout

Alert: `ResolverProofStale` · Alert budget: 300s · Recovery budget: 600s · Backlog cap: 8%

Throttle resolver RAD sync to prove ResolverProofStale alerting and GAR rollback steps.

**Inject steps:**
- Throttle RAD sync (60s) — `echo simulate RAD sync throttle for resolver`
- Record proof age (60s) — `echo capture proof age before alert`

**Verification:**
- Watch ResolverProofStale (60s) — `echo confirm ResolverProofStale alert triggers`
- Validate GAR enforcement pause (60s) — `echo check GAR enforcement paused for stale proofs`

**Remediation:**
- Restore RAD sync (120s) — `echo restore RAD sync cadence`
- Re-arm enforcement (120s) — `echo re-arm GAR enforcement and note proof ages`

---

