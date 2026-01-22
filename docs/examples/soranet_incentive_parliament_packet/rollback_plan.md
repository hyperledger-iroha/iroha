# Relay Incentive Rollback Plan

Use this playbook to disable automatic relay payouts if governance requests a
halt or if the telemetry guardrails fire.

1. **Freeze automation.** Stop the incentives daemon on every orchestrator host
   (`systemctl stop soranet-incentives.service` or the equivalent container
   deployment) and confirm the process is no longer running.
2. **Drain pending instructions.** Run
   `iroha app sorafs incentives service daemon --state <state.json> --config <daemon.json> --metrics-dir <spool> --once`
   to ensure there are no outstanding payout instructions. Archive the resulting
   Norito payloads for audit.
3. **Revoke governance approval.** Edit `reward_config.json`, set
   `"budget_approval_id": null`, and redeploy the configuration with
   `iroha app sorafs incentives service init` (or `update-config` if running a
   long-lived daemon). The payout engine now fails closed with
   `MissingBudgetApprovalId`, so the daemon refuses to mint payouts until a new
   approval hash is restored. Record the git commit and the SHA-256 of the
   modified config in the incident log.
4. **Notify Sora Parliament.** Attach the drained payout ledger, the shadow-run
   report, and a short incident summary. Parliament minutes must note the hash
   of the revoked configuration and the time the daemon was halted.
5. **Rollback validation.** Keep the daemon disabled until:
   - telemetry alerts (`soranet_incentives_rules.yml`) are green for >=24 h,
   - the treasury reconciliation report shows zero missing transfers, and
   - Parliament approves a new budget hash.

Once governance re-issues a budget approval hash, update `reward_config.json`
with the new digest, re-run the `shadow-run` command on the latest telemetry,
and restart the incentives daemon.
