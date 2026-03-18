---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/gateway-dns-runbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS گیٹ وے اور DNS کک آف رن بک

پورٹل کی اس کاپی میں کیننیکل رن بک کی عکاسی ہوتی ہے
[`docs/source/sorafs_gateway_dns_design_runbook.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md)۔
وکندریقرت ڈی این ایس اور گیٹ وے ورک اسٹریم سے آپریشنل حفاظتی اقدامات جمع کرتا ہے
تاکہ نیٹ ورکنگ ، او پی ایس اور دستاویزات لیڈز اسٹیک کی مشق کرسکیں
کک آف 2025-03 سے پہلے آٹومیشن۔

## دائرہ کار اور فراہمی

-مشتق کی مشق کرکے DNS (SF-4) اور گیٹ وے (SF-5) سنگ میل کو لنک کریں
  ہوسٹ ڈٹرمینسٹک ، ریزولور ڈائرکٹری ریلیز ، TLS/GAR آٹومیشن
  اور ثبوت کی گرفتاری۔
- کک آف ان پٹ (ایجنڈا ، دعوت نامہ ، حاضری ٹریکر ، اسنیپ شاٹ کو برقرار رکھیں
  گار ٹیلی میٹری) تازہ ترین مالکان کے اسائنمنٹس کے ساتھ ہم آہنگ۔
- گورننس کے جائزہ لینے والوں کے لئے قابل آڈٹ نمونے کا ایک بنڈل تیار کریں: تربیت کے نوٹ
  حل کرنے والے ڈائرکٹری کی رہائی ، گیٹ وے کی تحقیقات کے نوشتہ جات ، استعمال کی پیداوار
  تعمیل اور دستاویزات/ڈیوریل سمری۔

## کردار اور ذمہ داریاں

| ورک اسٹریم | ذمہ داریاں | مطلوبہ نمونے |
| --------- | --------- | ------------------------- |
| نیٹ ورکنگ TL (DNS اسٹیک) | ڈٹرمینسٹک میزبان منصوبہ کو برقرار رکھیں ، ریڈ ڈائرکٹری سے ریلیز پر عمل کریں ، حل کرنے والوں سے ٹیلی میٹری ان پٹ شائع کریں۔ | `artifacts/soradns_directory/<ts>/` ، `docs/source/soradns/deterministic_hosts.md` کے فرق ، RAD میٹا ڈیٹا۔ |
| اوپس آٹومیشن لیڈ (گیٹ وے) | TLS/ECH/GAR آٹومیشن ڈرل چلائیں ، `sorafs-gateway-probe` چلائیں ، پیجریڈی ہکس کو اپ ڈیٹ کریں۔ | `artifacts/sorafs_gateway_probe/<ts>/` ، تحقیقات JSON ، `ops/drill-log.md` میں اندراجات۔ |
| QA گلڈ اور ٹولنگ WG | `ci/check_sorafs_gateway_conformance.sh` ، کیور فکسچر ، آرکائیو سیلف سرٹ بنڈل Norito چلائیں۔ | `artifacts/sorafs_gateway_conformance/<ts>/` ، `artifacts/sorafs_gateway_attest/<ts>/`۔ |
| دستاویزات/ڈیوریل | منٹ ریکارڈ کریں ، پری ریڈ ریڈ + ضمیموں کو اپ ڈیٹ کریں اور اس پورٹل پر شواہد کا خلاصہ شائع کریں۔ | تازہ ترین فائلیں `docs/source/sorafs_gateway_dns_design_*.md` اور رول آؤٹ نوٹ۔ |

## اندراجات اور شرائط

- ڈٹرمینسٹک میزبان تصریح (`docs/source/soradns/deterministic_hosts.md`) اور
  حل کرنے والے تصدیقی سہاروں (`docs/source/soradns/resolver_attestation_directory.md`)۔
- گیٹ وے نمونے: آپریٹر دستی ، TLS/ECH آٹومیشن مددگار ،
  `docs/source/sorafs_gateway_*` کے تحت براہ راست موڈ گائیڈ اور سیلف سرٹ فلو۔
- ٹولنگ: `cargo xtask soradns-directory-release` ،
  `cargo xtask sorafs-gateway-probe` ، `scripts/telemetry/run_soradns_transparency_tail.sh` ،
  `scripts/sorafs_gateway_self_cert.sh` ، اور CI مددگار
  (`ci/check_sorafs_gateway_conformance.sh` ، `ci/check_sorafs_gateway_probe.sh`)۔
- راز: GAR ریلیز کلید ، ACME DNS/TLS اسناد ، پیجریڈی روٹنگ کلید ،
  ریزولورز حاصل کرنے کے لئے Torii کا Auth ٹوکن۔

## پری فلائٹ چیک لسٹ1. تازہ کاری کرکے شرکاء اور ایجنڈے کی تصدیق کریں
   `docs/source/sorafs_gateway_dns_design_attendance.md` اور ایجنڈا گردش کرنا
   موجودہ (`docs/source/sorafs_gateway_dns_design_agenda.md`)۔
2. نمونہ کی جڑیں جیسے تیار کریں
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` اور
   `artifacts/soradns_directory/<YYYYMMDD>/`۔
3. ریفریش فکسچر (گار منشور ، ریڈ ٹیسٹ ، گیٹ وے کی تعمیل بنڈل) اور
   اس بات کو یقینی بناتا ہے کہ `git submodule` کی حالت آخری ٹیسٹ ٹیگ سے مماثل ہے۔
4. راز کی تصدیق کریں (KEY ED25519 ، ACME اکاؤنٹ فائل ، پیجریڈی ٹوکن جاری کریں)
   اور یہ والٹ چیکسم میچ کرتا ہے۔
5. تمباکو نوشی کا ٹیسٹ ٹیلی میٹری کے اہداف (پش گیٹ وے اینڈ پوائنٹ ، گار بورڈ Grafana)
   ڈرل سے پہلے

## آٹومیشن ٹیسٹنگ اقدامات

### میزبانوں کا تعصب کا نقشہ اور ریڈ ڈائرکٹری کی رہائی

1. منشور سیٹ کے خلاف ڈٹرمینسٹک میزبان بائی پاس مددگار چلائیں
   مجوزہ اور تصدیق کرتا ہے کہ اس کے سلسلے میں کوئی بہاؤ نہیں ہے
   `docs/source/soradns/deterministic_hosts.md`۔
2. ایک حل کرنے والا ڈائرکٹری بنڈل بنائیں:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. ڈائریکٹری ID ، SHA-256 ، اور اندر کے اندر چھپی ہوئی آؤٹ پٹ راہیں ریکارڈ کریں
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` اور کک آف منٹ میں۔

### DNS ٹیلی میٹری کیپچر

- استعمال کرتے ہوئے ≥10 منٹ کے لئے قطار حل کرنے والے شفافیت لاگ ان
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`۔
- پش گیٹ وے میٹرکس برآمد کریں اور آرکائیو این ڈی جےسن اسنیپ شاٹس کے ساتھ ساتھ
  ID ڈائرکٹری چلائیں۔

### گیٹ وے آٹومیشن مشقیں

1. TLS/ایکیچ تحقیقات چلائیں:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. تعمیل کنٹرول (`ci/check_sorafs_gateway_conformance.sh`) چلائیں اور
   ریفریش کرنے کے لئے سیلف سرٹ ہیلپر (`scripts/sorafs_gateway_self_cert.sh`)
   تصدیق کا بنڈل Norito۔
3. اس آٹومیشن کو ظاہر کرنے کے لئے پیجریڈی/ویب ہک واقعات پر قبضہ کریں
   کام ختم ہونے کے لئے ختم.

### ثبوت پیکیجنگ

- ٹائم اسٹیمپس ، شرکاء اور تحقیقات ہیشوں کے ساتھ `ops/drill-log.md` کو اپ ڈیٹ کریں۔
- رن آئی ڈی ڈائریکٹریوں کے تحت نمونے کو محفوظ کریں اور ایک ایگزیکٹو سمری شائع کریں
  دستاویزات/ڈیوریل منٹ میں۔
- جائزہ سے پہلے گورننس ٹکٹ میں ثبوت کے بنڈل کو لنک کریں
  کک آف کی

## سیشن کی سہولت اور ثبوت کی فراہمی- ** ماڈریٹر ٹائم لائن: **
  - T-24 H- پروگرام مینجمنٹ `#nexus-steering` میں یاد دہانی + ایجنڈا/حاضری اسنیپ شاٹ شائع کرتا ہے۔
  - T-2 H- نیٹ ورکنگ TL GAR ٹیلی میٹری اسنیپ شاٹ کو تازہ دم کرتا ہے اور `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` میں ڈیلٹا کو ریکارڈ کرتا ہے۔
  - T-15 M- OPS آٹومیشن تحقیقات کی تیاری کی تصدیق کرتا ہے اور فعال رن ID کو `artifacts/sorafs_gateway_dns/current` پر لکھتا ہے۔
  - کال کے دوران - ماڈریٹر اس رن بک کو شیئر کرتا ہے اور براہ راست مصنف تفویض کرتا ہے۔ دستاویزات/ڈیوریل ایکشن آئٹمز آن لائن پر قبضہ کرتے ہیں۔
- ** منٹ ٹیمپلیٹ: ** کنکال کے کاپی کریں
  `docs/source/sorafs_gateway_dns_design_minutes.md` (بنڈل میں بھی عکس ہے
  پورٹل کا) اور فی سیشن میں ایک مکمل مثال پیش کرتا ہے۔ کی فہرست شامل ہے
  شرکاء ، فیصلے ، ایکشن آئٹمز ، ثبوت ہیش اور زیر التواء خطرات۔
- ** شواہد اپ لوڈ: ** مقدمے کی `runbook_bundle/` ڈائرکٹری کو دبائیں ،
  منٹ کے مہیا کردہ پی ڈی ایف کو منسلک کریں ، منٹ + میں SHA-256 ہیش ریکارڈ کریں
  جب اپ لوڈ ہوتا ہے تو ایجنڈا اور پھر گورننس کے جائزہ لینے والوں کو عرف کو مطلع کرتا ہے
  `s3://sora-governance/sorafs/gateway_dns/<date>/` پر زمین۔

## ثبوت کے اسنیپ شاٹ (مارچ 2025 کک آف)

روڈ میپ اور منٹ میں تازہ ترین ریہرسل/پروڈکشن نمونے کا حوالہ دیا گیا ہے
وہ بالٹی `s3://sora-governance/sorafs/gateway_dns/` میں رہتے ہیں۔ ذیل میں ہیشس
کیننیکل منشور (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`) کی عکاسی کریں۔

-** خشک رن-2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`) **
  - بنڈل ٹربال: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - منٹ کی پی ڈی ایف: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
-** براہ راست ورکشاپ-2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`) **
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _ (زیر التواء اپ لوڈ: `gateway_dns_minutes_20250303.pdf`- دستاویزات/ڈیورل SHA-256 کو شامل کریں گے جب مہیا کیا گیا PDF بنڈل میں پہنچے گا۔) _

## متعلقہ مواد

- [گیٹ وے آپریشنز پلے بوک] (./operations-playbook.md)
- [SoraFS مشاہداتی منصوبہ] (./observability-plan.md)
- [وکندریقرت DNS ٹریکر اور گیٹ وے] (https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)