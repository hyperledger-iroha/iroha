---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/gateway-dns-runbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# گیٹ وے اور ڈی این ایس لانچ رن بک SoraFS

پورٹل کی یہ کاپی کیننیکل رن بک میں آئینہ کرتی ہے
[`docs/source/sorafs_gateway_dns_design_runbook.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md)۔
یہ विकेंद्रीकृत DNS اور گیٹ وے ورک اسٹریم کے آپریشنل محافظوں کو اپنی گرفت میں لے لیتا ہے
تاکہ وہ نیٹ ورک ، او پی ایس اور دستاویزات کے مینیجر اسٹیک کو دہرا سکتے ہیں
لانچ 2025-03 سے پہلے آٹومیشن۔

## دائرہ کار اور فراہمی

-DNS (SF-4) اور گیٹ وے (SF-5) سنگ میل کو عین مطابق مشتق کو دہرا کر لنک کریں
  میزبان ، حل کرنے والے ڈائرکٹری ریلیز ، TLS/GAR آٹومیشن اور
  ثبوت کی گرفتاری.
- کک آف اندراجات رکھیں (کیلنڈر ، دعوت نامہ ، حاضری سے باخبر رہنا ، اسنیپ شاٹ
  گار ٹیلی میٹری) تازہ ترین مالک اسائنمنٹس کے ساتھ ہم آہنگ۔
- گورننس جائزہ لینے والوں کے لئے نمونے کا ایک قابل آڈٹ بنڈل تیار کریں: نوٹ
  حل کرنے والے ڈائرکٹری کی رہائی ، گیٹ وے تحقیقات لاگز ، استعمال کی آؤٹ پٹ
  تعمیل اور دستاویزات/ڈیوریل سمری۔

## کردار اور ذمہ داریاں

| ورک اسٹریم | ذمہ داریاں | مطلوبہ نمونے |
| ------------ | ------------------- | ------------------- |
| نیٹ ورکنگ TL (DNS اسٹیک) | ڈٹرمینسٹک میزبان منصوبہ کو برقرار رکھیں ، ریڈ ڈائرکٹری ریلیز پر عملدرآمد کریں ، حل کرنے والے ٹیلی میٹری کے آدانوں کو شائع کریں۔ | `artifacts/soradns_directory/<ts>/` ، `docs/source/soradns/deterministic_hosts.md` ، RAD میٹا ڈیٹا سے مختلف ہے۔ |
| اوپس آٹومیشن لیڈ (گیٹ وے) | TLS/ECH/GAR آٹومیشن ڈرل چلائیں ، `sorafs-gateway-probe` چلائیں ، پیجریڈی ہکس کو اپ ڈیٹ کریں۔ | `artifacts/sorafs_gateway_probe/<ts>/` ، تحقیقات JSON ، اندراجات `ops/drill-log.md`۔ |
| QA گلڈ اور ٹولنگ WG | `ci/check_sorafs_gateway_conformance.sh` لانچ کریں ، فکسچر صاف کریں ، خود کار سرٹ بنڈل Norito کو محفوظ کریں۔ | `artifacts/sorafs_gateway_conformance/<ts>/` ، `artifacts/sorafs_gateway_attest/<ts>/`۔ |
| دستاویزات/ڈیوریل | منٹ پر قبضہ کریں ، ڈیزائن پری ریڈ + منسلکات کو اپ ڈیٹ کریں ، اس پورٹل میں شواہد کی ترکیب شائع کریں۔ | `docs/source/sorafs_gateway_dns_design_*.md` فائلوں اور رول آؤٹ نوٹ کو اپ ڈیٹ کیا گیا۔ |

## اندراجات اور شرائط

- ڈٹرمینسٹک میزبان تفصیلات (`docs/source/soradns/deterministic_hosts.md`) اور
  حل کرنے والے تصدیقی سہاروں (`docs/source/soradns/resolver_attestation_directory.md`)۔
- گیٹ وے نمونے: آپریٹر دستی ، TLS/ECH آٹومیشن مددگار ،
  براہ راست موڈ رہنمائی ، `docs/source/sorafs_gateway_*` کے تحت سیلف سرٹ ورک فلو۔
- ٹولنگ: `cargo xtask soradns-directory-release` ،
  `cargo xtask sorafs-gateway-probe` ، `scripts/telemetry/run_soradns_transparency_tail.sh` ،
  `scripts/sorafs_gateway_self_cert.sh` ، اور CI مددگار
  (`ci/check_sorafs_gateway_conformance.sh` ، `ci/check_sorafs_gateway_probe.sh`)۔
- راز: GAR ریلیز کلید ، ACME DNS/TLS اسناد ، پیجریڈی روٹنگ کلید ،
  حل کرنے والے بازیافت کے لئے Auth ٹوکن Torii۔

## پری فلائٹ چیک لسٹ1. تازہ کاری کرکے شرکاء اور ایجنڈے کی تصدیق کریں
   `docs/source/sorafs_gateway_dns_design_attendance.md` اور ایجنڈا نشر کرنا
   موجودہ (`docs/source/sorafs_gateway_dns_design_agenda.md`)۔
2. نمونے کی جڑیں تیار کریں جیسے
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` اور
   `artifacts/soradns_directory/<YYYYMMDD>/`۔
3. فکسچر کو تازہ کریں (گار منشور ، ریڈ پروف ، گیٹ وے کی تعمیل بنڈل) اور
   اس بات کو یقینی بنائیں کہ ریاست `git submodule` آخری ریہرسل ٹیگ کے مساوی ہے۔
4. راز چیک کریں (ED25519 ریلیز کلید ، ACME اکاؤنٹ فائل ، پیجریڈی ٹوکن)
   اور والٹ چیکسمس کے ساتھ ان کا خط و کتابت۔
5. ٹیلی میٹری کے اہداف کا تمباکو نوشی ٹیسٹ کریں (پش گیٹ وے اینڈ پوائنٹ ، گار بورڈ Grafana)
   ڈرل سے پہلے

## آٹومیشن ریہرسل اقدامات

### عین مطابق میزبان نقشہ اور ریڈ ڈائرکٹری کی رہائی

1. منشور سیٹ پر ڈٹرمینسٹک میزبان مشتق مددگار چلائیں
   اس کے مقابلے میں بڑھنے کی عدم موجودگی کی تجویز اور تصدیق کریں
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

3. ڈائریکٹری ID ، SHA-256 اور چھپی ہوئی آؤٹ پٹ راہوں کو محفوظ کریں
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` اور کک آف منٹ میں۔

### DNS ٹیلی میٹری کیپچر

- ٹیلر ریزولور شفافیت کے ساتھ ≥10 منٹ کے ساتھ لاگ ان کریں
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
   بنڈل کو تازہ دم کرنے کے لئے سیلف سرٹ ہیلپر (`scripts/sorafs_gateway_self_cert.sh`)
   سرٹیفکیٹ Norito۔
3. پیجرڈیٹی/ویب ہک کے واقعات پر قبضہ کریں تاکہ یہ ثابت کریں کہ آٹومیشن چین
   کام ختم ہونے کے لئے ختم.

### ثبوت کی پیکیجنگ

- ٹائم اسٹیمپس ، شرکاء اور تحقیقات ہیشوں کے ساتھ `ops/drill-log.md` کو اپ ڈیٹ کریں۔
- رن آئی ڈی ڈائریکٹریوں میں نمونے اسٹور کریں اور ایک ایگزیکٹو سمری شائع کریں
  دستاویزات/ڈیوریل منٹ میں۔
- کک آف جائزہ سے پہلے گورننس ٹکٹ میں ثبوت کے بنڈل کو لنک کریں۔

## سیشن حرکت پذیری اور ثبوت پیش کرنا- ** ماڈریٹر ٹائم لائن: **
  - T-24 H- پروگرام مینجمنٹ `#nexus-steering` میں یاد دہانی + کیلنڈر/موجودگی اسنیپ شاٹ پوسٹ کرتا ہے۔
  - T-2 H- نیٹ ورکنگ TL GAR ٹیلی میٹری اسنیپ شاٹ کو تازہ دم کرتا ہے اور `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` میں ڈیلٹا کو لاگ کرتا ہے۔
  - T-15 M- OPS آٹومیشن تحقیقات کی تیاری کی جانچ پڑتال کرتا ہے اور `artifacts/sorafs_gateway_dns/current` پر فعال رن ID لکھتا ہے۔
  - کال کے دوران - ماڈریٹر اس رن بک کو شیئر کرتا ہے اور براہ راست مصنف تفویض کرتا ہے۔ دستاویزات/ڈیوریل آن لائن کارروائیوں کو اپنی گرفت میں لیتے ہیں۔
- ** منٹ ٹیمپلیٹ: ** کنکال کے کاپی کریں
  `docs/source/sorafs_gateway_dns_design_minutes.md` (بنڈل میں بھی آئینہ دار
  پورٹل کا) اور فی سیشن میں ایک مکمل مثال کا ارتکاب کریں۔ کی فہرست شامل کریں
  شرکاء ، فیصلے ، اقدامات ، پروف ہیش اور کھلے خطرات۔
- ** ثبوت اپ لوڈ کریں: ** زپ `runbook_bundle/` ریہرسل ڈائرکٹری ،
  مہی .ا منٹ پی ڈی ایف منسلک کریں ، منٹ + میں SHA-256 ہیش کو بچائیں
  ایجنڈا ، پھر اپ لوڈ ہونے کے بعد گورننس کے جائزہ لینے والوں کے عرفی ناموں کو پنگ
  `s3://sora-governance/sorafs/gateway_dns/<date>/` میں دستیاب ہے۔

## ثبوت کے اسنیپ شاٹ (کک آف مارچ 2025)

روڈ میپ اور منٹ میں تازہ ترین ریہرسل/براہ راست نمونے کا حوالہ دیا گیا ہے
بالٹی `s3://sora-governance/sorafs/gateway_dns/` میں محفوظ ہیں۔ ہیشس
ذیل میں کیننیکل منشور (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`) کی عکاسی کریں۔

-** خشک رن-2025-03-02 (`artifacts/sorafs_gateway_dns/20250302/`) **
  - ٹربال بنڈل: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - منٹ کی پی ڈی ایف: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
-** براہ راست ورکشاپ-2025-03-03 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`) **
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _ (اپلوڈ زیر التواء: `gateway_dns_minutes_20250303.pdf`- دستاویزات/ڈیوریل جیسے ہی مہی .ا پی ڈی ایف بنڈل میں ہیں SHA-256 شامل کریں گے۔) _

## متعلقہ مواد

- [گیٹ وے آپریشنز پلے بوک] (./operations-playbook.md)
- [مشاہدہ کا منصوبہ SoraFS] (./observability-plan.md)
- [وکندریقرت DNS ٹریکر اور گیٹ وے] (https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)