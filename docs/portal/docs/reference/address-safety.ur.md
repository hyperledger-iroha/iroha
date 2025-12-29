<!-- Auto-generated stub for Urdu (ur) translation. Replace this content with the full translation. -->

---
lang: ur
direction: rtl
source: docs/portal/docs/reference/address-safety.md
status: complete
translator: manual
source_hash: 582a75b7b68e86acd82b36ccacec2691d6552d45bb00e2f6fe5bed1424f2842a
source_last_modified: "2025-11-06T19:39:18.688464+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

---
title: ایڈریس سیفٹی اور ایکسیسبلٹی
description: Iroha ایڈریسز کو محفوظ طریقے سے دکھانے اور شیئر کرنے کے لیے UX تقاضے (ADDR‑6c)۔
---

یہ صفحہ ADDR‑6c ڈاکومنٹیشن deliverable کو summarize کرتا ہے۔ ان پابندیوں کو wallets،
explorers، SDK tooling، اور پورٹل کی ہر surface پر اپلائی کریں جو انسانوں کو نظر آنے
والے ایڈریس کو render یا قبول کرتی ہے۔ Canonical ڈیٹا ماڈل
`docs/account_structure.md` میں موجود ہے؛ نیچے دی گئی checklist یہ واضح کرتی ہے کہ
ان فارمیٹس کو سیفٹی اور ایکسیسبلٹی خراب کیے بغیر کیسے expose کیا جائے۔

## سیف شیئرنگ فلو

- ہر copy/share ایکشن کے لیے ڈیفالٹ، IH‑B32 ایڈریس ہونا چاہیے۔ checksum والی string
  کو سامنے رکھنے کے لیے resolved domain کو صرف supportive context کے طور پر دکھائیں۔
- ایسا “Share” کنٹرول فراہم کریں جو مکمل plain‑text address اور اسی payload سے نکلا
  ہوا QR code، دونوں کو bundle کرے۔ یوزر کو confirm کرنے سے پہلے دونوں کو inspect
  کرنے دیں۔
- جب جگہ کم ہونے کے سبب truncation ضروری ہو (چھوٹے cards، notifications وغیرہ) تو
  human‑readable prefix کو برقرار رکھیں، درمیان میں ellipses لگائیں، اور آخری 4–6
  حروف ضرور رکھیں تاکہ checksum anchor زندہ رہے۔ truncated string دکھانے کے باوجود،
  full string کو copy کرنے کے لیے tap/کی بورڈ شارٹ کٹ فراہم کریں۔
- clipboard desync سے بچنے کے لیے confirmatory toast دکھائیں جو بالکل وہی IH‑B32
  string preview کرے جو copy ہوئی ہے۔ جہاں telemetry دستیاب ہو، copy attempts اور
  share actions کی گنتی الگ الگ رکھیں تاکہ UX regressions جلد surface ہو سکیں۔

## IME اور input safeguards

- address فیلڈز میں non‑ASCII input reject کریں۔ اگر IME composition artefacts (full
  width، Kana، tone marks وغیرہ) ظاہر ہوں تو inline warning دیں جس میں بتایا جائے کہ
  Latin input کی بورڈ پر کیسے switch کیا جائے اور پھر دوبارہ کوشش کریں۔
- ایک plain‑text paste زون دیں جو combining marks کو ہٹا دے اور validation سے پہلے
  whitespace کو ASCII spaces سے replace کر دے۔ اس طرح IME disable ہونے کے باوجود
  یوزر کا progress محفوظ رہتا ہے۔
- validation کو zero‑width joiners، variation selectors اور دیگر “stealth” Unicode
  code points کے خلاف سخت کریں۔ reject ہونے والے code points کی category کو log کریں
  تاکہ fuzzing suites میں یہ telemetry import کی جا سکے۔

## Assistive technology کی توقعات

- ہر address block کے ساتھ `aria-label` یا `aria-describedby` شامل کریں جو
  human‑readable prefix کو واضح طور پر بتائے اور payload کو 4–8 حروف کے گروپس میں
  توڑ کر بیان کرے (“ih dash b three two …”)؛ اس سے screen readers، بے معنی حروف کا
  مسلسل stream پڑھنے کے بجائے قابلِ فہم بلاکس سنائیں گے۔
- copy/share کے کامیاب events کو polite live region اپ ڈیٹ کے ذریعے announce کریں،
  اور ساتھ میں destination (clipboard، share sheet، QR) کا حوالہ دیں تاکہ یوزر کو
  بغیر focus move کیے معلوم ہو جائے کہ ایکشن مکمل ہو گیا ہے۔
- QR preview کے لیے واضح `alt` ٹیکسٹ فراہم کریں (مثلاً: “IH‑B32 address for
  `<alias>@<domain>` on chain `0x1234`”)۔ QR canvas کے ساتھ “Copy address as text” کا
  fallback بھی دیں تاکہ low‑vision یوزرز، ایڈریس کو بطور text آسانی سے حاصل کر سکیں۔

## صرف Sora کے لیے compressed addresses

- Gating: `snx1…` والی compressed string کو ایک explicit confirmation کے پیچھے چھپائیں۔
  confirmation ڈائیلاگ میں واضح کریں کہ یہ فارم صرف Sora Nexus chains کے لیے درست
  ہے۔
- Labelling: ہر occurrence کے ساتھ نمایاں “Sora‑only” بیج ہو اور tooltip میں وضاحت
  ہو کہ دوسری نیٹ ورکس کے لیے IH‑B32 فارم کیوں ضروری ہے۔
- Guardrails: اگر active chain discriminant، Nexus allocation نہیں ہے تو compressed
  address ہرگز generate نہ کریں، بلکہ یوزر کو واپس IH‑B32 پر redirect کریں۔
- Telemetry: یہ log کریں کہ compressed فارم کتنی بار request اور copy ہوا، تاکہ
  incident playbook کسی بھی accidental sharing spike کو detect کر سکے۔

## Quality gates

- UI automated tests (یا Storybook a11y suites) کو extend کریں تاکہ address components
  کے ARIA metadata کو assert کیا جا سکے اور یہ بھی چیک ہو کہ IME rejection messages
  مناسب جگہ پر آ رہے ہیں۔
- ریلیز سے پہلے IME input (kana، pinyin)، screen reader pass (VoiceOver/NVDA)، اور
  high‑contrast تھیمز میں QR copy کے لیے manual QA scenarios شامل کریں۔
- ان checks کو release checklists میں IH‑B32 parity tests کے ساتھ ساتھ surface کریں،
  تاکہ regressions، correction کے بغیر merge نہ ہو سکیں۔

</div>
