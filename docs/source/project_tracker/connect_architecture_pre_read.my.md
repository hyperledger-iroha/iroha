---
lang: my
direction: ltr
source: docs/source/project_tracker/connect_architecture_pre_read.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 49086d73514ea1175441eb7484634acb34de9fc21f124db1227edf7fb727408b
source_last_modified: "2025-12-29T18:16:36.009368+00:00"
translation_last_reviewed: 2026-02-07
---

# Connect Architecture Workshop Pre-Read (Feb 2026)

This note packages the material that must be circulated ahead of the Feb 2026
cross-SDK workshop. Send the links below in the calendar invite and attach the
rendered PDF bundle if stakeholders need an offline copy.

## Contents

1. **Strawman spec** — `docs/source/connect_architecture_strawman.md`  
   Latest blueprint for session IDs, key derivation, flow control, telemetry,
   and feature negotiation. Regenerate HTML/PDF via `make docs` before sharing.
2. **Feedback checklist** — `docs/source/connect_architecture_feedback.md`  
   Captures every Android/JS decision and the owners who signed off.
3. **Follow-up tracker** — `docs/source/project_tracker/connect_architecture_followups_ios.md`  
   Ticket status for post-workshop actions (retry policy, rotation control,
   telemetry exporters, etc.).

Bundle the sources plus the generated HTML/PDF into `artifacts/connect/pre-read/`
so attendees have a snapshot that matches the invite.

## Distribution Steps

1. Run `make docs-html` and collect the rendered files from
   `docs/build/html/connect_architecture_strawman.html` (and related pages).
2. Copy the rendered HTML/PDF plus this summary into
   `artifacts/connect/pre-read/<YYYYMMDD>/`.
3. Attach or link the bundle in the calendar invite titled
   “Connect Architecture Workshop (Feb 2026)” and mention:
   - Strawman section anchors to review (`Encryption & Key Management`,
     `Sequence Numbers & Flow Control`, `Telemetry & Diagnostics`).
   - Feedback checklist row owners.
   - Expected asks (open questions list).
4. Post a reminder in `#sdk-council` with the bundle hash so reviewers can
   confirm they looked at the right revision.
5. After the workshop, archive the bundle path in `status.md` under the IOS7 /
   AND7 readiness log.

> ✅ **Current bundle:** `artifacts/connect/pre-read/20260129/` (generated after
> updating the strawman and SDK docs on 2026-01-29).
