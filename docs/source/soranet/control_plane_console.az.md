---
lang: az
direction: ltr
source: docs/source/soranet/control_plane_console.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9681863c0814072642b68aba110c8068a92414e381d34931a0c01588f6c81952
source_last_modified: "2025-12-29T18:16:36.185603+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Control Plane Console Wireframes (SNNet-15D)

The console mirrors the tenant-aware API and RBAC catalogue so operators can
exercise every control-plane flow with predictable affordances and evidence
capture. This draft focuses on the minimal MVP screens needed to unlock the
SNNet-15D acceptance criteria; final visual polish will match the SoraNet
dashboard theme once the service ships.

## Flows & Panels

- **Landing / login:** org/project selector with scoped token display, recent
  audit feed, and shortcut cards for domain onboarding, purge, analytics, and
  billing. The selector echoes `X-SN-Org` / `X-SN-Project` to keep SDK/CLI and
  console aligned.
- **Domain onboarding wizard:** four steps (hostname, TLS/ECH mode, DNS proof
  upload, validation summary). CTA surfaces cache profile defaults and WAF pack
  being applied; failure paths collect evidence hashes for GAR packets.
- **DNS zone editor:** structured record set grid with deterministic sorting,
  diff preview, and “publish” modal; export/import buttons emit the same JSON as
  the API’s zone payloads for fixture reuse.
- **Cache/WAF controls:** cache profile picker (TTL/serve-stale/constant-rate
  class), purge request composer (path|tag|cid), WAF ruleset list with policy
  labels, and a rate-limit tab that mirrors the `setRateLimit` payload shape.
- **Analytics dashboard:** preset cards (requests, latency, hit ratio, errors,
  purge volume) with window chips (5 m/1 h/1 d) wired to the analytics query
  helper; export buttons attach the JSON result to the download manifest for
  audit reuse.
- **Billing & audit:** statement table with period selector, meter drill-down,
  and an audit tail pane that streams the `/v2/audit/events` feed with
  download/export actions matching the RBAC catalogue.

## Accessibility & Determinism

- Keyboard navigation covers every CTA and input; focus order mirrors the
  wizard steps and grid columns. Descriptive `aria-label` text matches the API
  field names.
- Error toasts and inline validation bubble up the exact scope requirements
  from `control_plane_rbac.yaml` (e.g., “missing scope: cache:write”) and
  include the request id for support follow-up.
- All downloads (zone export, analytics export, audit tail) carry deterministic
  filenames and brotli-compressed JSON/NDJSON bundles so CI can diff them
  against fixtures.

## Evidence Capture (M0/M1)

- Each wizard and form logs a local “artifact summary” JSON containing the
  request body, response envelope, scope list, and timestamp. These summaries
  are written alongside downloaded bundles to a user-selectable directory,
  mirroring the CLI artefact layout for governance packets.
- Console telemetry emits `ui.control-plane.scope_request` and
  `ui.control-plane.export` events so the adoption dashboard can prove which
  scopes are exercised before GA.
