---
lang: fr
direction: ltr
source: docs/examples/docs_preview_request_template.md
status: complete
translator: manual
source_hash: 59948351a84b27efe0d9741545d8f93c7525fa5f545605a0942d9f2f574f6f06
source_last_modified: "2025-11-10T20:01:03.610024+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traduction française de docs/examples/docs_preview_request_template.md (Docs Portal Preview Access Request Template) -->

# Demande d’accès au preview du portail de docs (template)

Utilisez ce template pour collecter les informations d’un relecteur avant de lui accorder
un accès à l’environnement de preview public. Copiez le markdown dans un ticket ou un
formulaire de demande et remplacez les valeurs placeholder.

```markdown
## Request Summary
- Requester: <full name / org>
- GitHub handle: <username>
- Preferred contact: <email/Matrix/Signal>
- Region & timezone: <UTC offset>
- Proposed start / end dates: <YYYY-MM-DD → YYYY-MM-DD>
- Reviewer type: <Core maintainer | Partner | Community volunteer>

## Compliance Checklist
- [ ] Signed the preview acceptable-use policy (link).
- [ ] Reviewed `docs/portal/docs/devportal/security-hardening.md`.
- [ ] Reviewed `docs/portal/docs/devportal/incident-runbooks.md`.
- [ ] Acknowledged telemetry collection & anonymised analytics (yes/no).
- [ ] SoraFS alias requested (yes/no). Alias name: `<docs-preview-???>`

## Access Needs
- Preview URL(s): <https://docs-preview.sora.link/...>
- Required API scopes: <Torii read-only | Try it sandbox | none>
- Additional context (SDK tests, documentation review focus, etc.):
  <details here>

## Approval
- Reviewer (maintainer): <name + date>
- Governance ticket / change request: <link>
```

---

## Questions spécifiques à la communauté (W2+)
- Motivation pour l’accès au preview (une phrase) :
- Focal principal de la revue (SDK, gouvernance, Norito, SoraFS, autre) :
- Engagement hebdomadaire & créneau de disponibilité (UTC) :
- Besoins en localisation ou accessibilité (oui/non + détails) :
- Code de conduite communautaire + addendum acceptable‑use du preview lus et acceptés (oui/non) :
