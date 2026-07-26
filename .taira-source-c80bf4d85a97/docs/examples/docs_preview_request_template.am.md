---
lang: am
direction: ltr
source: docs/examples/docs_preview_request_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 59948351a84b27efe0d9741545d8f93c7525fa5f545605a0942d9f2f574f6f06
source_last_modified: "2025-12-29T18:16:35.072823+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# የሰነዶች ፖርታል ቅድመ እይታ መዳረሻ ጥያቄ (አብነት)

የመዳረሻ ፍቃድ ከመስጠትዎ በፊት የገምጋሚ ዝርዝሮችን ሲይዙ ይህን አብነት ይጠቀሙ
የህዝብ ቅድመ እይታ አካባቢ. ምልክቱን ወደ ችግር ወይም የጥያቄ ቅጽ ይቅዱ እና
የቦታ ያዥ እሴቶችን ይተኩ.

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

## የማህበረሰብ-ተኮር ጥያቄዎች (W2+)
- ለቅድመ እይታ መዳረሻ ማበረታቻ (አንድ ዓረፍተ ነገር)
- የመጀመሪያ ደረጃ ግምገማ ትኩረት (ኤስዲኬ፣ አስተዳደር፣ Norito፣ SoraFS፣ ሌላ)፡
- ሳምንታዊ የጊዜ ቁርጠኝነት እና ተገኝነት መስኮት (UTC)፦
- የአካባቢ ወይም የተደራሽነት ፍላጎቶች (አዎ/አይ + ዝርዝሮች)
- የማህበረሰብ የስነምግባር ህግ + ቅድመ-ዕይታ ተቀባይነት ያለው ጥቅም ላይ ሊውል የሚችል ተጨማሪ ድምር ተቀባይነት አግኝቷል (አዎ/አይ)