---
title: SoraNet Downgrade Communication Template
summary: Boilerplate message for notifying operators and SDK consumers about temporary SoraNet downgrades.
---

**Subject:** SoraNet downgrade in region {{ region }} ({{ incident_id }})

**Summary:**

- **When:** {{ start_time }} UTC – {{ expected_end_time }} UTC
- **Scope:** Circuits in {{ region }} are temporarily falling back to direct mode while we remediate {{ root_cause }}.
- **Impact:** Increased latency and reduced anonymity for SoraFS fetches; GAR enforcement remains active.

**Operator Actions:**

1. Apply the published override (`transport_policy=direct-only`) until we announce recovery.
2. Monitor the brownout dashboards (`sorafs_orchestrator_policy_events_total`, `soranet_privacy_circuit_events_total`).
3. Record mitigation steps in your GAR logbook.

**SDK / Client Messaging:**

- Status page banner: "SoraNet circuits in {{ region }} are temporarily downgraded. Traffic remains private but not anonymous."
- API header: `Soranet-Downgrade: region={{ region }}; incident={{ incident_id }}`

**Next Update:** {{ follow_up_time }} UTC or earlier.

Please route any questions to the governance bridge (`#soranet-incident`).
