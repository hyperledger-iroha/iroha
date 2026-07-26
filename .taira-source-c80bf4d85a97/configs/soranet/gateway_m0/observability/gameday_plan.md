# SN15-M0 GameDay Scenarios (soranet-pop-m0)

- **Quarterly cadence:** Follow `gameday_schedule.json` + `chaos_runbook.md` to rotate scenarios once per quarter (weekday + weekend). Store outputs under `artifacts/soranet/gateway_chaos/`.
- **Harness:** `cargo xtask soranet-gateway-chaos --pop soranet-pop-m0 --config chaos_scenarios.json --out artifacts/soranet/gateway_chaos --scenario <id> [--execute] [--note <text>]`
- **Scenarios:** Prefix withdrawal/failover, trustless verifier stall, resolver proof brownout (see `chaos_plan.md` + `chaos_runbook.md` for steps and budgets).
- **Signals to watch:** BgpSessionFlap, ResolverProofStale, CacheHitRateDrop, TailSamplingBackpressure, chaos_metrics.json expressions.
- **Evidence bundle:** Attach alert exports, Grafana screenshots, chaos_report.json/md from the harness, and any manual rollback notes to the GameDay invite.
