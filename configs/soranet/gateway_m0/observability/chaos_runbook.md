# Gateway Chaos Runbook (soranet-pop-m0)

Use `cargo xtask soranet-gateway-chaos` to orchestrate drills. Default runs are dry-run; pass `--execute` to run the scripted steps.

## Scenarios
- **Prefix withdrawal / failover** (alert: `BgpSessionFlap`) — alert budget 300s, recovery budget 900s, backlog cap 10%.
- **Trustless verifier stalled pipeline** (alert: `GatewayVerifierStall`) — alert budget 240s, recovery budget 600s, backlog cap 5%.
- **Resolver RAD sync brownout** (alert: `ResolverProofStale`) — alert budget 300s, recovery budget 600s, backlog cap 8%.

## Quarterly schedule
- 2025-Q4 → scenarios: prefix-withdrawal (owner: sre-oncall)
- 2026-Q1 → scenarios: trustless-verifier-failure (owner: sre-oncall)
- 2026-Q2 → scenarios: resolver-brownout (owner: sre-oncall)
- 2026-Q3 → scenarios: prefix-withdrawal, trustless-verifier-failure, resolver-brownout (owner: sre-oncall)
