# Brownout / Downgrade Response Playbook

1. **Detect** — alert fires for downgrades or brownouts.
2. **Stabilise** — disable guard rotation, apply direct-only override for affected clients, capture compliance hash.
3. **Diagnose** — collect support bundle, review PoW queue, identify PQ/compliance issues.
4. **Escalate** — notify governance bridge and open incident ticket.
5. **Recover** — re-enable rotation, revert overrides once metrics stabilise.
6. **Postmortem** — submit report within 48 hours and update runbooks if needed.
