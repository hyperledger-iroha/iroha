---
lang: ka
direction: ltr
source: docs/examples/docs_preview_request_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 59948351a84b27efe0d9741545d8f93c7525fa5f545605a0942d9f2f574f6f06
source_last_modified: "2025-12-29T18:16:35.072823+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Docs Portal Preview Access მოთხოვნა (თარგი)

გამოიყენეთ ეს შაბლონი მიმომხილველის დეტალების აღებისას, სანამ მასზე წვდომას მიანიჭებთ
საჯარო გადახედვის გარემო. დააკოპირეთ მონიშვნა საკითხის ან მოთხოვნის ფორმაში და
ჩანაცვლების ადგილის მნიშვნელობების შეცვლა.

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

## თემის სპეციფიკური კითხვები (W2+)
- გადახედვისას წვდომის მოტივაცია (ერთი წინადადება):
- პირველადი მიმოხილვის ფოკუსი (SDK, მმართველობა, Norito, SoraFS, სხვა):
- ყოველკვირეული დროის ვალდებულება და ხელმისაწვდომობის ფანჯარა (UTC):
- ლოკალიზაციის ან ხელმისაწვდომობის საჭიროებები (დიახ/არა + დეტალები):
- საზოგადოების ქცევის კოდექსი + მისაღები გამოყენების დამატებების წინასწარი გადახედვა აღიარებულია (დიახ/არა):