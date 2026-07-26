---
lang: ba
direction: ltr
source: docs/examples/docs_preview_request_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 59948351a84b27efe0d9741545d8f93c7525fa5f545605a0942d9f2f574f6f06
source_last_modified: "2025-12-29T18:16:35.072823+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Доктар порталы алдан ҡарау инеү запросы (Шаблон)

Был ҡалыпты ҡулланыу, ҡасан реквизанттар реквизиттарын тотоу алдынан рөхсәт алыу өсөн .
йәмәғәт алдан ҡарау мөхите. Күсермә маркдаун мәсьәлә йәки үтенес формаһында һәм
урын хужалары ҡиммәттәрен алмаштырыу.

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

## Йәмәғәтселек-специфик һорауҙар (W2+)
- Алдан ҡарау өсөн мотивация (бер һөйләм):
- Беренсел тикшерелгән йүнәлеш (СДК, идара итеү, I18NT0000000000X, SoraFS, башҡа):
- Аҙналыҡ ваҡыт йөкләмәһе & доступность тәҙрә (UTC):
- Локализация йәки ҡулайлыҡ ихтыяждары (эйе/юҡ + ентекле):
- Йәмәғәтселек тәртибе кодексы + алдан ҡарау ҡабул итеү-ҡулланыу өҫтәмә танылған (эйе/юҡ):