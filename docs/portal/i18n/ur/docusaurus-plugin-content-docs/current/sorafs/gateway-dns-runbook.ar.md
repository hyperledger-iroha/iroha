---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/gateway-dns-runbook.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS میں گیٹ وے اور DNS اسٹارٹ اپ گائیڈ اسٹارٹ کریں

پورٹل کاپی اس منظور شدہ آپریشنل دستی کی عکاسی کرتی ہے
[`docs/source/sorafs_gateway_dns_design_runbook.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md)۔
یہ विकेंद्रीकृत DNS اور گیٹ وے کے آپریشنل کنٹرول کو اپنی گرفت میں لے لیتا ہے تاکہ ٹیمیں ...
لانچ 2025-03 سے پہلے نیٹ ورکنگ ، آپریشنز اور آٹومیشن پیکیج کی تربیت کی دستاویزات۔

## دائرہ کار اور آؤٹ پٹ

-ڈی این ایس (SF-4) اور گیٹ وے (SF-5) کی خصوصیات کو عین مطابق میزبان اخذ کردہ مشقوں کے ذریعہ خصوصیات ،
  ریزولورس ڈائریکٹری ورژن ، TLS/GAR آٹومیشن ، اور ڈائریکٹری مجموعہ۔
- لانچ اندراجات (ایجنڈا ، دعوت نامہ ، حاضری ٹریکر ، گار) رکھیں
  مالک کی تازہ ترین تقرریوں کے ساتھ ہم آہنگ۔
- گورننس آڈیٹرز کے لئے ایک آڈٹیبل آرٹیکٹیکٹ پیکیج تیار کرنا: رہائی کے نوٹس
  حل کرنے والے ، گیٹ وے چیک لاگز ، تعمیل ٹول آؤٹ پٹ ، اور ایک دستاویزات/ڈیوریل سمری۔

## کردار اور ذمہ داریاں

| راستہ | ذمہ داریاں | مطلوبہ نمونے |
| -------- | --------------- | ----------------------------------------------------------- |
| نیٹ ورکنگ TL (DNS پیکٹ) | ڈٹرمینسٹک میزبانوں کے منصوبے کو برقرار رکھیں ، ریڈ ڈائرکٹری ورژن چلائیں ، ٹیلی میٹرک اندراجات کے حل کو شائع کریں۔ | `artifacts/soradns_directory/<ts>/` ، `docs/source/soradns/deterministic_hosts.md` اختلافات ، اور RAD میٹا ڈیٹا۔ |
| اوپس آٹومیشن لیڈ (گیٹ وے) | نافذ TLS/ECH/GAR آٹومیشن مشقیں ، `sorafs-gateway-probe` ، اور اپ ڈیٹ پیجریڈی ہکس۔ | `artifacts/sorafs_gateway_probe/<ts>/` ، تحقیقات کے لئے JSON ، اور `ops/drill-log.md` ان پٹ۔ |
| QA گلڈ اور ٹولنگ WG | `ci/check_sorafs_gateway_conformance.sh` ، فارمیٹ فکسچر ، اور Norito کے لئے محفوظ سیلف سرٹ پیکجوں کو چلائیں۔ | `artifacts/sorafs_gateway_conformance/<ts>/` ، `artifacts/sorafs_gateway_attest/<ts>/`۔ |
| دستاویزات/ڈیوریل | منٹ کی نقل ، ڈیزائن پری ریڈ + ضمیمہ کو اپ ڈیٹ کریں ، اور اس پورٹل میں شواہد کا خلاصہ شائع کریں۔ | `docs/source/sorafs_gateway_dns_design_*.md` فائلوں کو اپ ڈیٹ کیا گیا اور نوٹ لانچ کریں۔ |

## آدانوں اور شرائط

- عین مطابق میزبانوں کی تفصیلات (`docs/source/soradns/deterministic_hosts.md`) اور فن تعمیر
  سپورٹ ریزولورز (`docs/source/soradns/resolver_attestation_directory.md`)۔
- گیٹ وے نمونے: آپریٹر گائیڈ ، TLS/ECH آٹومیشن مدد ، براہ راست موڈ ہدایات ،
  اور سیلف سرٹ کا راستہ `docs/source/sorafs_gateway_*` کے تحت ہے۔
- ٹولز: `cargo xtask soradns-directory-release` ،
  `cargo xtask sorafs-gateway-probe` ، `scripts/telemetry/run_soradns_transparency_tail.sh` ،
  `scripts/sorafs_gateway_self_cert.sh` ، اور CI افادیت
  (`ci/check_sorafs_gateway_conformance.sh` ، `ci/check_sorafs_gateway_probe.sh`)۔
- راز: GAR ورژن کی کلید ، DNS/TLS کے لئے ACME اسناد ، پیجریڈی روٹنگ کلید ،
  اور توثیق کا کوڈ Torii حل کرنے والوں کو بازیافت کرنے کے لئے۔

## نفاذ سے پہلے چیک لسٹ

1. حاضری کی تصدیق کریں اور ایجنڈے کو اپ ڈیٹ کریں
   `docs/source/sorafs_gateway_dns_design_attendance.md` اور موجودہ ایجنڈے کو مرکزی دھارے میں شامل کرنا
   (`docs/source/sorafs_gateway_dns_design_agenda.md`)۔
2. نمونے کی جڑوں کی تیاری ، جیسے
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` اور
   `artifacts/soradns_directory/<YYYYMMDD>/`۔
3. فکسچر کو اپ ڈیٹ کریں (گار منشور ، ریڈ دستورالعمل ، گیٹ وے مطابقت پیک) اور یقینی بنائیں
   اس بات کو یقینی بنائیں کہ `git submodule` کی حالت آخری تربیتی ٹیگ سے مماثل ہے۔
4. راز کی تصدیق کریں (ED25519 ورژن کلید ، ACME اکاؤنٹ فائل ، پیجریڈی کوڈ) اور میچ
   والٹ میں چیکسم۔
5. ٹیلی میٹرک اہداف کے لئے دھواں ٹیسٹ کریں (پش گیٹ وے کا اختتامی نقطہ ، Grafana پر گار بورڈ)
   ورزش سے پہلے

## آٹومیشن ورزش کے اقدامات

### تعی .ن پسند نقشہ اور ریڈ ڈائریکٹری ورژن کی میزبانی کرتا ہے

1. مجوزہ منشور کے ذخیرے پر مشتق میزبانوں سے مشتق مددگار چلائیں اور تصدیق کریں
   اس کے مقابلے میں بہاؤ کی کمی کی
   `docs/source/soradns/deterministic_hosts.md`۔
2. ریزولورز ڈائریکٹری پیکیج بنائیں:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. پرنٹ شدہ ڈائرکٹری ID ، SHA-256 ، اور اندر آؤٹ پٹ راستے لکھیں
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` اور لانچ ریکارڈ میں۔

### DNS ٹیلی میٹرک کیپچر- شفافیت لاگز ≥10 منٹ کے لئے ریزولورز کو ٹریک کریں
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`۔
- رن آئی ڈی فولڈر کے ساتھ ہی پش گیٹ وے میٹرکس اور آرکائیو این ڈی جےسن اسنیپ شاٹس برآمد کریں۔

### گیٹ وے آٹومیشن مشقیں

1. ایک TLS/ECH اسکین چلائیں:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. مطابقت کا آلہ (`ci/check_sorafs_gateway_conformance.sh`) اور سیلف سرٹ اسسٹنٹ چلائیں
   (`scripts/sorafs_gateway_self_cert.sh`) سپورٹ پیکیج Norito کو اپ ڈیٹ کرنے کے لئے۔
3. پیجریڈی/ویب ہک کے واقعات پر قبضہ کریں تاکہ یہ ثابت کیا جاسکے کہ آٹومیشن پائپ لائن اختتام سے آخر میں کام کررہی ہے۔

### ثبوت اکٹھا کریں

- ٹائم اسٹیمپس ، شرکاء ، اور تحقیقات ہیشوں کے ساتھ `ops/drill-log.md` کو اپ ڈیٹ کیا گیا۔
- رن آئی ڈی فولڈرز میں نمونے اسٹور کریں اور دستاویزات/ڈیوریل ریکارڈز میں ایگزیکٹو سمری شائع کریں۔
- لانچ کا جائزہ لینے سے پہلے ثبوت کے پیکیج کو گورننس ٹکٹ میں جوڑنا۔

## سیشن کا انتظام کرنا اور ثبوت فراہم کرنا

- ** ایڈمن ٹائم لائن: **
  - T-24 H- پروگرام مینجمنٹ `#nexus-steering` پر یاد دہانی + ایجنڈا/حاضری اسنیپ شاٹ پوسٹ کرتا ہے۔
  - T-2 H- نیٹ ورکنگ TL GAR ٹیلی میٹک اسنیپ شاٹ کو اپ ڈیٹ کرتا ہے اور `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` پر اختلافات کو ریکارڈ کرتا ہے۔
  - T-15 M- OPS آٹومیشن تحقیقات کی تیاری کی جانچ پڑتال کرتا ہے اور فعال رن ID کو `artifacts/sorafs_gateway_dns/current` پر لکھتا ہے۔
  - کال کے دوران - ماڈریٹر اس ثبوت کو شیئر کرتا ہے اور ایک براہ راست ٹرانسکرپٹسٹ تفویض کرتا ہے۔ سیشن کے دوران دستاویزات/ڈیوریل ایکشن آئٹمز پر قبضہ کرتے ہیں۔
- ** کارروائی ٹیمپلیٹ: ** ڈھانچے کو کاپی کریں
  `docs/source/sorafs_gateway_dns_design_minutes.md` (بنڈل گیٹ وے میں ظاہر ہوتا ہے)
  ہر سیشن کے لئے مکمل ٹرانسکرپٹ فائل کرنے کا عہد کریں۔ حاضری ، فیصلوں اور ایکشن آئٹمز کی ایک فہرست شامل کریں
  نازک ثبوت اور کھلے خطرات۔
- ** شواہد اپ لوڈ کریں: ** `runbook_bundle/` فولڈر پر کلک کریں ، مشق کے لئے ، ٹرانسکرپٹس کے پی ڈی ایف کو منسلک کریں
  برآمد کنندہ ، منٹ + ایجنڈے میں SHA-256 ہیشوں کو ریکارڈ کریں ، پھر جائزہ لینے والوں کے نام کو نوٹ کریں۔
  `s3://sora-governance/sorafs/gateway_dns/<date>/` پر فائلیں اپ لوڈ کرنے کے بعد مجاز۔

## ثبوت اسنیپ شاٹ (مارچ 2025 سے شروع ہو رہے ہیں)

نقشہ اور ریکارڈ سے وابستہ تازہ ترین نمونے میں محفوظ ہے
`s3://sora-governance/sorafs/gateway_dns/`۔ ذیل میں ہیش ٹیگ کی عکاسی ہوتی ہے
تائید شدہ منشور (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`)۔

-** خشک رن-2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`) **
  - ٹربال پیکیج: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - پی ڈی ایف ریکارڈز: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
-** براہ راست ورکشاپ-03-03-2025 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`) **
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  -`030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  -`5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  -`87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _ (متوقع اپ لوڈ: `gateway_dns_minutes_20250303.pdf`- جب پی ڈی ایف پیکیج میں دستیاب ہے تو دستاویزات/ڈیوریل SHA-256 کی قیمت میں اضافہ کریں گے۔) _

## متعلقہ مواد

- [گیٹ وے آپریشنز آپریشن دستی] (./operations-playbook.md)
- [مانیٹرنگ پلان SoraFS] (./observability-plan.md)
- [وکندریقرت DNS ٹریکر اور گیٹ وے] (https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)