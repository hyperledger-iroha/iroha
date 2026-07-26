<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Telemetry Override Audit Log

Record each approved redaction override here. Populate the table after the
override is applied and again after revocation.

| Ticket ID | Request Timestamp (UTC) | Approver | Token Hash (Blake2b) | Expiry (UTC) | Revoked (UTC) | Notes |
|-----------|-------------------------|----------|----------------------|--------------|---------------|-------|
| SUP-OVR-2214 | 2026-02-15T09:25:03Z | Liam O’Connor (SRE on-call) | 65f1a2c8a4c3446c9b6df2c59e1782ad | 2026-02-15T09:55:03Z | 2026-02-15T09:56:30Z | AND7 rehearsal Scenario C2; CLI output stored under `docs/source/sdk/android/readiness/screenshots/2026-02-15/override-console-2026-02-15.log`. |

> **Retention:** Keep entries for 13 months, then archive under
> `docs/source/sdk/android/telemetry_override_log_archive/`.
