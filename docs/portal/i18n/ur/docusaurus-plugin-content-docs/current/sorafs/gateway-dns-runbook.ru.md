---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/gateway-dns-runbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

گیٹ وے اور DNS SoraFS کو لانچ کرنے کے لئے # رن بک

پورٹل میں یہ کاپی کیننیکل رن بک کی عکاسی کرتی ہے
[`docs/source/sorafs_gateway_dns_design_runbook.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md)۔
یہ विकेंद्रीकृत ڈی این ایس اور گیٹ وے ورکس اسٹریم کے لئے آپریشنل گارڈریوں کو ٹھیک کرتا ہے ،
تاکہ نیٹ ورکنگ ، او پی ایس اور دستاویزات لیڈز اسٹیک کی مشق کرسکیں
کک آف 2025-03 سے پہلے آٹومیشن۔

## دائرہ کار اور فراہمی

-لنک DNS سنگ میل (SF-4) اور گیٹ وے (SF-5) کو تعصب پسندانہ ریہرسل کے ذریعے
  میزبان مشتق ، ریزولورس ڈائرکٹری ریلیز ، TLS/GAR آٹومیشن اور
  ثبوت اکٹھا کرنا۔
- کک آف نمونے (ایجنڈا ، دعوت نامہ ، حاضری ٹریکر ، اسنیپ شاٹ رکھیں
  گار ٹیلی میٹری) تازہ ترین مالکان کے اسائنمنٹس کے ساتھ ہم آہنگ۔
- گورننس کے جائزہ لینے والوں کے لئے نمونے کا ایک آڈٹیبل بنڈل تیار کریں: نوٹ جاری کریں
  ریزولورز ڈائرکٹری ، گیٹ وے پروبس لاگز ، مطابقت پذیری کنٹرول آؤٹ پٹ اور سمری
  دستاویزات/ڈیوریل۔

## کردار اور ذمہ داریاں

| ورک اسٹریم | ذمہ داری | مطلوبہ نمونے |
| ------------ | -------- | ----------------------- |
| نیٹ ورکنگ TL (DNS اسٹیک) | ایک ڈٹرمینسٹک میزبان منصوبہ کو برقرار رکھیں ، رڈ ڈائرکٹری ریلیز چلائیں ، حل کرنے والے ٹیلی میٹری ان پٹ کو شائع کریں۔ | `artifacts/soradns_directory/<ts>/` ، `docs/source/soradns/deterministic_hosts.md` کے لئے مختلف ہے ، RAD میٹا ڈیٹا۔ |
| اوپس آٹومیشن لیڈ (گیٹ وے) | TLS/ECH/GAR آٹومیشن ڈرل انجام دیں ، `sorafs-gateway-probe` چلائیں ، پیجریڈی ہکس کو اپ ڈیٹ کریں۔ | `artifacts/sorafs_gateway_probe/<ts>/` ، تحقیقات JSON ، `ops/drill-log.md` میں اندراجات۔ |
| QA گلڈ اور ٹولنگ WG | `ci/check_sorafs_gateway_conformance.sh` چلائیں ، فکسچر کی نگرانی ، آرکائیو Norito سیلف سرٹ بنڈل۔ | `artifacts/sorafs_gateway_conformance/<ts>/` ، `artifacts/sorafs_gateway_attest/<ts>/`۔ |
| دستاویزات/ڈیوریل | ریکارڈ منٹ ، اپ ڈیٹ ڈیزائن پری ریڈ + ضمیمہ ، پورٹل میں ثبوت کا خلاصہ شائع کریں۔ | تازہ ترین `docs/source/sorafs_gateway_dns_design_*.md` اور رول آؤٹ نوٹ۔ |

## آدانوں اور شرائط

- ڈٹرمینسٹک میزبان تفصیلات (`docs/source/soradns/deterministic_hosts.md`) اور
  حل کرنے والوں کے لئے تصدیق کا فریم ورک (`docs/source/soradns/resolver_attestation_directory.md`)۔
- گیٹ وے نمونے: آپریٹر ہینڈ بک ، TLS/ECH آٹومیشن مددگار ،
  `docs/source/sorafs_gateway_*` کے تحت براہ راست موڈ اور سیلف سرٹ ورک فلو پر رہنمائی۔
- ٹولنگ: `cargo xtask soradns-directory-release` ،
  `cargo xtask sorafs-gateway-probe` ، `scripts/telemetry/run_soradns_transparency_tail.sh` ،
  `scripts/sorafs_gateway_self_cert.sh` اور CI مددگار
  (`ci/check_sorafs_gateway_conformance.sh` ، `ci/check_sorafs_gateway_probe.sh`)۔
- راز: GAR ریلیز کلید ، DNS/TLS ACME اسناد ، روٹنگ کلیدی پیجریڈی ،
  بازیافت حل کرنے والوں کے لئے Torii Auth ٹوکن۔

## پری فلائٹ چیک لسٹ

1. تازہ کاری کرکے شرکاء اور ایجنڈے کی تصدیق کریں
   `docs/source/sorafs_gateway_dns_design_attendance.md` اور موجودہ بھیجنا
   ایجنڈا (`docs/source/sorafs_gateway_dns_design_agenda.md`)۔
2. نمونے کی جڑیں تیار کریں ، جیسے۔
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` اور
   `artifacts/soradns_directory/<YYYYMMDD>/`۔
3. اپ ڈیٹ فکسچر (گار منشور ، ریڈ پروف ، بنڈل موافقت گیٹ وے) اور
   اس بات کو یقینی بنائیں کہ `git submodule` کی حیثیت تازہ ترین ریہرسل ٹیگ سے مماثل ہے۔
4. راز چیک کریں (ED25519 ریلیز کلید ، ACME اکاؤنٹ فائل ، پیجریڈی ٹوکن) اور
   چیکسمس کو والٹ سے ملاپ کرنا۔
5. دھواں ٹیسٹ ٹیلی میٹری کے اہداف کا انعقاد (پش گیٹ وے اینڈ پوائنٹ ، GAR Grafana بورڈ)
   ڈرل سے پہلے

## آٹومیشن ریہرسل اقدامات

### عین مطابق میزبان کا نقشہ اور ریڈ ڈائرکٹری کی رہائی1. مجوزہ سیٹ پر ڈٹرمینسٹک میزبان مشتق مددگار چلائیں
   ظاہر ہوتا ہے اور اس کے بارے میں بہاؤ کی عدم موجودگی کی تصدیق کرتا ہے
   `docs/source/soradns/deterministic_hosts.md`۔
2. حل کرنے والے ڈائرکٹری کا ایک بنڈل بنائیں:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. پرنٹ شدہ ڈائرکٹری ID ، SHA-256 اور اندر آؤٹ پٹ راستے پر قبضہ کریں
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` اور منٹ کک آف میں۔

### DNS ٹیلی میٹری کیپچر

- ٹیل ریزولور شفافیت لاگ ان ≥10 منٹ کے لئے
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`۔
- پش گیٹ وے میٹرکس برآمد کریں اور آرکائیو این ڈی جےسن اسنیپ شاٹس کے ساتھ
  ID ڈائریکٹریز چلائیں۔

### مشقیں آٹومیشن گیٹ وے

1. ایک TLS/ECH تحقیقات انجام دیں:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2
   اپ ڈیٹ کے لئے سیلف سرٹ ہیلپر (`scripts/sorafs_gateway_self_cert.sh`)
   Norito تصدیق کا بنڈل۔
3. آخر سے آخر تک کام کی تصدیق کے ل Pa پیجریڈی/ویب ہک واقعات پر قبضہ کریں
   آٹومیشن کے راستے

### پیکیجنگ ثبوت

- ٹائم اسٹیمپ ، شرکاء اور ہیشوں کی تحقیقات کے ساتھ `ops/drill-log.md` کو اپ ڈیٹ کریں۔
- رن آئی ڈی ڈائریکٹریوں میں نمونے کو محفوظ کریں اور ایگزیکٹو سمری شائع کریں
  منٹ میں دستاویزات/ڈیوریل۔
- کک آف جائزہ سے پہلے گورننس ٹکٹ میں موجود ثبوت کے بنڈل کا حوالہ دیں۔

## سیشن اعتدال اور ثبوت کی منتقلی

- ** ماڈریٹر ٹائم لائن: **
  - T -24 H - پروگرام مینجمنٹ `#nexus-steering` میں یاد دہانی + اسنیپ شاٹ ایجنڈا/حاضری شائع کرتا ہے۔
  - T -2 H - نیٹ ورکنگ TL نے GAR ٹیلی میٹری اسنیپ شاٹ کو اپ ڈیٹ کیا اور ڈیلٹا کو `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` پر ٹھیک کیا۔
  - T -15 M - OPS آٹومیشن تیاری کے لئے تحقیقات کی جانچ پڑتال کرتا ہے اور `artifacts/sorafs_gateway_dns/current` پر فعال رن ID لکھتا ہے۔
  - ایک کال کے دوران - ماڈریٹر اس رن بک کو شیئر کرتا ہے اور براہ راست مصنف تفویض کرتا ہے۔ دستاویزات/ڈیوریل ایکشن آئٹمز کے جاتے وقت ٹھیک کریں۔
- ** ٹیمپلیٹ منٹ: ** کنکال سے کاپی کریں
  `docs/source/sorafs_gateway_dns_design_minutes.md` (پورٹل بنڈل میں بھی جھلکتا ہے)
  اور ہر سیشن کے لئے مکمل مثال کا ارتکاب کریں۔ شرکاء کی ایک فہرست شامل کریں ،
  فیصلے ، ایکشن آئٹمز ، ہیشوں کے ثبوت اور کھلے خطرات۔
- ** شواہد لوڈنگ: ** آرکائیو `runbook_bundle/` ریہرسل سے ،
  مہیا کردہ پی ڈی ایف منٹ منسلک کریں ، SHA-256 ہیشوں کو منٹ + ایجنڈا میں لکھیں ،
  پھر لوڈ کرنے کے بعد گورننس جائزہ لینے والے عرف کو مطلع کریں
  `s3://sora-governance/sorafs/gateway_dns/<date>/`۔

## ثبوت کے اسنیپ شاٹ (کک آف مارچ 2025)

روڈ میپ اور منٹ میں مذکور تازہ ترین ریہرسل/براہ راست نمونے بالٹی میں ہیں
`s3://sora-governance/sorafs/gateway_dns/`۔ نیچے دیئے گئے ہیشوں نے کیننیکل کی عکاسی کی ہے
منشور (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`)۔

- ** خشک رن- 2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`) **
  - ٹربال بنڈل: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - منٹ پی ڈی ایف: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- ** براہ راست ورکشاپ- 2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`) **
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _۔

## متعلقہ مواد

- [گیٹ وے کے لئے آپریشنز پلے بوک] (./operations-playbook.md)
- [مشاہدہ کا منصوبہ SoraFS] (./observability-plan.md)
- [وکندریقرت DNS اور گیٹ وے ٹریکر] (https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)