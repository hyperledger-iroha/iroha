---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/chunker-conformance.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: chunker-conformance
כותרת: SoraFS מדריך התאמה ל-chunker
sidebar_label: התאמה של צ'אנקר
description: Fixtures اور SDKs میں deterministic SF1 chunker profile کو برقرار رکھنے کے لیے requirements اور workflows۔
---

:::note مستند ماخذ
:::

یہ regeneration workflow، signing policy، اور verification steps بھی document کرتی ہے تاکہ SDKs میں fixture consumers sync میں رہیں۔

## פרופיל קנוני

- זרע קלט (hex): `0000000000dec0ded`
- גודל יעד: 262144 בייטים (256 KiB)
- גודל מינימלי: 65536 בתים (64 KiB)
- גודל מקסימלי: 524288 בייטים (512 KiB)
- פולינום מתגלגל: `0x3DA3358B4DC173`
- זרע שולחן ציוד: `sorafs-v1-gear`
- מסיכת שבירה: `0x0000FFFF`

יישום התייחסות: `sorafs_chunker::chunk_bytes_with_digests_profile`.
کسی بھی SIMD acceleration کو یکساں boundaries اور digests پیدا کرنے چاہئیں۔

## חבילת מתקנים

גופי `cargo run --locked -p sorafs_chunker --bin export_vectors` מתחדשים
کرتا ہے اور درج ذیل فائلیں `fixtures/sorafs_chunker/` میں بناتا ہے:

- `sf1_profile_v1.{json,rs,ts,go}` — Rust, TypeScript, اور Go consumers کے لیے canonical chunk boundaries۔ ہر فائل
  آتے ہیں (مثلاً `sorafs.sf1@1.0.0`، پھر `sorafs.sf1@1.0.0`)۔ 20 הזמנות
  `ensure_charter_compliance` کے ذریعے enforce ہوتی ہے اور اسے تبدیل نہیں کیا جا سکتا۔
- `manifest_blake3.json` — BLAKE3-verified manifest جو ہر fixture فائل کو cover کرتا ہے۔
- `manifest_signatures.json` — manifest digest پر council signatures (Ed25519)۔
- `sf1_profile_v1_backpressure.json` اور `fuzz/` کے اندر raw corpora —
  deterministic streaming scenarios جو chunker back-pressure tests میں استعمال ہوتے ہیں۔

### מדיניות חתימה

Fixture regeneration **لازم** طور پر valid council signature شامل کرے۔ generator unsigned output کو reject کرتا ہے جب تک `--allow-unsigned` واضح طور پر نہ دیا جائے (صرف مقامی تجربات کے لیے)۔ Signature envelopes append-only ہوتے ہیں اور signer کے لحاظ سے deduplicate ہوتے ہیں۔

Council signature شامل کرنے کے لیے:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## אימות

CI helper `ci/check_sorafs_fixtures.sh` generator کو `--locked` کے ساتھ دوبارہ چلاتا ہے۔
اگر fixtures میں drift ہو یا signatures missing ہوں تو job fail ہو جاتا ہے۔ اس script کو
nightly workflows میں اور fixtures changes submit کرنے سے پہلے استعمال کریں۔

שלבי אימות ידני:

1. `cargo test -p sorafs_chunker`‏
2. `ci/check_sorafs_fixtures.sh` לוקס.
3. ‏

## שדרג את ספר המשחקים

نیا chunker profile propose کرتے وقت یا SF1 اپڈیٹ کرتے وقت:

یہ بھی دیکھیں: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) تاکہ
metadata requirements، proposal templates اور validation checklists مل سکیں۔

1. نئے parameters کے ساتھ `ChunkProfileUpgradeProposalV1` (RFC SF-1 دیکھیں) تیار کریں۔
2. `export_vectors` کے ذریعے fixtures regenerate کریں اور نیا manifest digest ریکارڈ کریں۔
3. مطلوبہ council quorum کے ساتھ manifest sign کریں۔ تمام signatures
   `manifest_signatures.json` میں append ہونی چاہئیں۔
4. متاثرہ SDK fixtures (Rust/Go/TS) اپڈیٹ کریں اور cross-runtime parity یقینی بنائیں۔
5. اگر parameters بدلیں تو fuzz corpora regenerate کریں۔
6. اس گائیڈ میں نیا profile handle، seeds، اور digest اپڈیٹ کریں۔
7. تبدیلی کو اپڈیٹڈ tests اور roadmap updates کے ساتھ submit کریں۔اگر اس عمل کے بغیر chunk boundaries یا digests تبدیل کیے جائیں تو وہ invalid ہیں اور merge نہیں ہونے ‏