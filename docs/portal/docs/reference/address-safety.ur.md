---
lang: ur
direction: rtl
source: docs/portal/docs/reference/address-safety.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 582a75b7b68e86acd82b36ccacec2691d6552d45bb00e2f6fe5bed1424f2842a
source_last_modified: "2025-11-06T19:39:18.688464+00:00"
translation_last_reviewed: 2025-12-30
---

---
title: Address safety اور accessibility
description: Iroha addresses کو محفوظ انداز میں پیش کرنے اور شیئر کرنے کیلئے UX requirements (ADDR-6c).
---

یہ صفحہ ADDR-6c documentation deliverable کو ظاہر کرتا ہے۔ ان پابندیوں کو wallets، explorers، SDK tooling، اور کسی بھی portal surface پر لاگو کریں جو human-facing addresses کو render یا accept کرتا ہو۔ canonical data model `docs/account_structure.md` میں ہے؛ نیچے دیا گیا checklist بتاتا ہے کہ ان formats کو safety یا accessibility کو متاثر کئے بغیر کیسے expose کیا جائے۔

## Safe sharing flows

- ہر copy/share action کی default کو I105 address بنائیں۔ resolved domain کو supporting context کے طور پر دکھائیں تاکہ checksummed string سامنے رہے۔
- ایک “Share” affordance فراہم کریں جو full plain-text address اور اسی payload سے بنے QR code کو bundle کرے۔ users کو commit کرنے سے پہلے دونوں inspect کرنے دیں۔
- جب جگہ کم ہو (tiny cards, notifications)، human-readable prefix رکھیں، ellipses دکھائیں، اور آخری 4–6 characters برقرار رکھیں تاکہ checksum anchor قائم رہے۔ truncation کے بغیر full string copy کرنے کیلئے tap/keyboard shortcut دیں۔
- clipboard desync کو روکنے کیلئے confirmation toast دیں جو بالکل وہی I105 string preview کرے جو copy ہوئی۔ جہاں telemetry دستیاب ہو، copy attempts کو share actions کے مقابل count کریں تاکہ UX regressions جلد سامنے آئیں۔

## IME اور input safeguards

- address fields میں non-ASCII input reject کریں۔ جب IME composition artifacts (full width, Kana, tone marks) ظاہر ہوں تو inline warning دکھائیں جو بتائے کہ دوبارہ کوشش سے پہلے keyboard کو Latin input پر کیسے لایا جائے۔
- ایک plain-text paste zone فراہم کریں جو combining marks ہٹائے اور whitespace کو ASCII spaces سے بدل دے، پھر validation کرے۔ اس سے users IME بند کرنے پر بھی progress نہیں کھوتے۔
- zero-width joiners، variation selectors، اور دیگر stealth Unicode code points کے خلاف validation سخت کریں۔ rejected code point category log کریں تاکہ fuzzing suites telemetry import کر سکیں۔

## Assistive technology expectations

- ہر address block کو `aria-label` یا `aria-describedby` سے annotate کریں جو human-readable prefix spell کرے اور payload کو 4–8 character groups میں chunk کرے (“ih dash b three two …”). اس سے screen readers بے معنی کرداروں کی لڑی نہیں بولتے۔
- successful copy/share events کو polite live region update کے ذریعے announce کریں۔ destination (clipboard, share sheet, QR) شامل کریں تاکہ user کو focus بدلے بغیر action مکمل ہونے کا پتا ہو۔
- QR previews کیلئے descriptive `alt` text دیں (مثال: “I105 address for `<account>` on chain `0x1234`”). کم بصارت والے users کیلئے QR canvas کے ساتھ “Copy address as text” fallback دیں۔

## Sora-only i105-default addresses

- Gating: i105-default string `i105` کو explicit confirmation کے پیچھے چھپائیں۔ confirmation میں دہرائیں کہ یہ form صرف Sora Nexus chains پر کام کرتی ہے۔
- Labelling: ہر occurrence میں واضح “Sora-only” badge اور tooltip دیں جو بتائے کہ دوسری networks کو i105 form کیوں چاہیے۔
- Guardrails: اگر active chain discriminant Nexus allocation نہ ہو تو i105-default address generate کرنے سے مکمل انکار کریں اور user کو I105 پر واپس بھیجیں۔
- Telemetry: i105-default form کے request/copy کی frequency ریکارڈ کریں تاکہ incident playbook accidental sharing spikes کو detect کر سکے۔

## Quality gates

- automated UI tests (یا storybook a11y suites) کو بڑھائیں تاکہ address components مطلوبہ ARIA metadata expose کریں اور IME rejection messages ظاہر ہوں۔
- manual QA scenarios میں IME input (kana, pinyin)، screen reader pass (VoiceOver/NVDA)، اور high-contrast themes پر QR copy شامل کریں، release سے پہلے۔
- ان checks کو release checklists میں I105 parity tests کے ساتھ شامل کریں تاکہ regressions درست ہونے تک blocked رہیں۔
