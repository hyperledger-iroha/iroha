---
lang: my
direction: ltr
source: docs/examples/docs_preview_request_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 59948351a84b27efe0d9741545d8f93c7525fa5f545605a0942d9f2f574f6f06
source_last_modified: "2025-12-29T18:16:35.072823+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Docs Portal အစမ်းကြည့်ရှုခွင့် တောင်းဆိုမှု (ပုံစံ)

ဝေဖန်သုံးသပ်သူအသေးစိတ်အချက်အလက်များကို ဖမ်းယူသည့်အခါ ဤပုံစံခွက်ကို အသုံးပြုပါ။
အများသူငှာ စမ်းကြည့်သော ပတ်ဝန်းကျင်။ အမှတ်အသားကို ပြဿနာတစ်ခု သို့မဟုတ် တောင်းဆိုမှုပုံစံသို့ ကူးယူပါ။
placeholder တန်ဖိုးများကို အစားထိုးပါ။

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

## ရပ်ရွာ-သီးသန့်မေးခွန်းများ (W2+)
- အစမ်းဝင်ရောက်ကြည့်ရှုမှုအတွက် လှုံ့ဆော်မှု (စာကြောင်းတစ်ကြောင်း)။
- မူလပြန်လည်သုံးသပ်ခြင်းအာရုံ (SDK၊ အုပ်ချုပ်မှု၊ Norito၊ SoraFS၊ အခြား)-
- အပတ်စဉ် အချိန်ကတိကဝတ်နှင့် ရရှိနိုင်မှုဝင်းဒိုး (UTC):
- ဒေသသတ်မှတ်ခြင်း သို့မဟုတ် ဝင်ရောက်နိုင်မှု လိုအပ်ချက်များ (ဟုတ်/မဟုတ် + အသေးစိတ်အချက်များ)။
- ကွန်မြူနတီကျင့်ထုံး + လက်ခံနိုင်သော-အသုံးပြုမှု နောက်ဆက်တွဲကို အစမ်းကြည့်ရှုခြင်း (ဟုတ်/မဟုတ်)-