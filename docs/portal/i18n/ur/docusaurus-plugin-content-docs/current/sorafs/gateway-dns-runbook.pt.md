---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/gateway-dns-runbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS گیٹ وے اور DNS کک آف رن بک

پورٹل کی یہ کاپی کیننیکل رن بک میں آئینہ کرتی ہے
[`docs/source/sorafs_gateway_dns_design_runbook.md`] (https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md)۔
یہ विकेंद्रीकृत ڈی این ایس اور گیٹ وے ورک اسٹریم کے آپریشنل محافظوں کو اپنی گرفت میں لے لیتا ہے
تاکہ نیٹ ورکنگ ، او پی ایس اور دستاویزات کے رہنما اسٹیک کی مشق کرسکیں
کک آف 2025-03 سے پہلے آٹومیشن۔

## دائرہ کار اور فراہمی

-مشتق جانچ کے ذریعہ DNS (SF-4) اور گیٹ وے (SF-5) کے نشانات سے رابطہ کریں
  تعی .ن میزبان ، حل کرنے والے ڈائرکٹری ریلیز ، TLS/GAR آٹومیشن
  اور ثبوت پر قبضہ کرنا۔
- کک آف ان پٹ کو برقرار رکھیں (ایجنڈا ، دعوت نامہ ، حاضری ٹریکر ، اسنیپ شاٹ
  گار ٹیلی میٹری) تازہ ترین مالک اسائنمنٹس کے ساتھ ہم آہنگ۔
- گورننس جائزہ لینے والوں کے لئے نمونے کا ایک قابل آڈٹ بنڈل تیار کریں: نوٹ
  حل کرنے والے ڈائرکٹری ، گیٹ وے تحقیقات کے نوشتہ جات ، آؤٹ پٹ کی رہائی
  تعمیل اور دستاویزات/ڈیوریل سمری۔

## کردار اور ذمہ داریاں

| ورک اسٹریم | ذمہ داریاں | مطلوبہ نمونے |
| ------------ |
| TL نیٹ ورکنگ (DNS اسٹیک) | ڈٹرمینسٹک میزبان منصوبہ کو برقرار رکھیں ، ریڈ ڈائرکٹری ریلیز انجام دیں ، حل کرنے والوں سے ٹیلی میٹری ان پٹ شائع کریں۔ | `artifacts/soradns_directory/<ts>/` ، `docs/source/soradns/deterministic_hosts.md` ، RAD میٹا ڈیٹا سے مختلف ہے۔ |
| اوپس آٹومیشن لیڈ (گیٹ وے) | TLS/ECH/GAR آٹومیشن ڈرل چلائیں ، `sorafs-gateway-probe` چلائیں ، پیجریڈی ہکس کو اپ ڈیٹ کریں۔ | `artifacts/sorafs_gateway_probe/<ts>/` ، JSON تحقیقات ، `ops/drill-log.md` اندراجات۔ |
| QA گلڈ اور ٹولنگ WG | `ci/check_sorafs_gateway_conformance.sh` ، کیور فکسچر ، آرکائیو Norito سیلف سرٹ بنڈل چلائیں۔ | `artifacts/sorafs_gateway_conformance/<ts>/` ، `artifacts/sorafs_gateway_attest/<ts>/`۔ |
| دستاویزات/ڈیوریل | منٹ پر قبضہ کریں ، پری ریڈ ریڈ + ضمیموں کو ڈیزائن کریں ، اور اس پورٹل پر ثبوت کا خلاصہ شائع کریں۔ | `docs/source/sorafs_gateway_dns_design_*.md` فائلوں اور رول آؤٹ نوٹ کو اپ ڈیٹ کیا گیا۔ |

## اندراجات اور شرائط

- عزم میزبانوں کی تفصیلات (`docs/source/soradns/deterministic_hosts.md`) اور
  حل کرنے والے تصدیق (`docs/source/soradns/resolver_attestation_directory.md`) کی سہاروں۔
- گیٹ وے نمونے: آپریٹر دستی ، TLS/ECH آٹومیشن مددگار ،
  `docs/source/sorafs_gateway_*` میں براہ راست موڈ رہنمائی اور سیلف سرٹ ورک فلو۔
- ٹولنگ: `cargo xtask soradns-directory-release` ،
  `cargo xtask sorafs-gateway-probe` ، `scripts/telemetry/run_soradns_transparency_tail.sh` ،
  `scripts/sorafs_gateway_self_cert.sh` ، اور CI مددگار
  (`ci/check_sorafs_gateway_conformance.sh` ، `ci/check_sorafs_gateway_probe.sh`)۔
- راز: GAR ریلیز کلید ، ACME DNS/TLS اسناد ، پیجریڈی روٹنگ کلید ،
  Torii AUTH ٹوکن برائے حل کرنے والے بازیافت کے لئے۔

## پری فلائٹ چیک لسٹ

1. تازہ کاری کرکے شرکاء اور ایجنڈے کی تصدیق کریں
   `docs/source/sorafs_gateway_dns_design_attendance.md` اور ایجنڈا گردش کرنا
   موجودہ (`docs/source/sorafs_gateway_dns_design_agenda.md`)۔
2. نمونہ کی جڑیں جیسے تیار کریں
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` اور
   `artifacts/soradns_directory/<YYYYMMDD>/`۔
3. اپ ڈیٹ فکسچر (گار منشور ، ریڈ پروف ، گیٹ وے کی تعمیل بنڈل) اور
   اس بات کو یقینی بنائیں کہ `git submodule` کی حالت آخری ٹیسٹ ٹیگ کے ساتھ منسلک ہے۔
4. راز چیک کریں (KEY ED25519 ، ACME اکاؤنٹ فائل ، پیجریڈی ٹوکن جاری کریں)
   اور ایک دوسرے کو والٹ چیکسم سے مارا۔
5. تمباکو نوشی کے اہداف کی جانچ (پش گیٹ وے اینڈ پوائنٹ ، گار بورڈ Grafana)
   سوراخ کرنے سے پہلے## آٹومیشن ٹیسٹ اقدامات

### عین مطابق میزبان کا نقشہ اور ریڈ ڈائرکٹری کی رہائی

1. منشور سیٹ کے خلاف ڈٹرمینسٹک میزبان مشتق مددگار چلائیں
   مجوزہ اور تصدیق کریں کہ اس کے سلسلے میں کوئی بہاؤ نہیں ہے
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

3. ڈائریکٹری ID ، SHA-256 ، اور اندر چھپی ہوئی آؤٹ پٹ راہیں ریکارڈ کریں
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` اور کک آف منٹ میں۔

### DNS ٹیلی میٹری کیپچر

- till 10 منٹ کا استعمال کرتے ہوئے شفافیت کے نوشتہ جات کو حل کریں
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`۔
- پش گیٹ وے سے میٹرکس برآمد کریں اور آرکائیو این ڈی جےسن اسنیپ شاٹس کے ساتھ ساتھ
  ID ڈائرکٹری چلائیں۔

### گیٹ وے آٹومیشن مشقیں

1. TLS/ایکیچ تحقیقات چلائیں:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. تعمیل کنٹرول (`ci/check_sorafs_gateway_conformance.sh`) کو گھمائیں اور
   اپ ڈیٹ کرنے کے لئے سیلف سرٹ ہیلپر (`scripts/sorafs_gateway_self_cert.sh`)
   تصدیق کا بنڈل Norito۔
3. آٹومیشن کے راستے کو ثابت کرنے کے لئے پیجریڈی/ویب ہک واقعات پر قبضہ کریں
   کام ختم ہونے کے لئے ختم.

### ثبوت کی پیکیجنگ

- ٹائم اسٹیمپس ، شرکاء اور تحقیقات ہیشوں کے ساتھ `ops/drill-log.md` کو اپ ڈیٹ کریں۔
- رن آئی ڈی ڈائریکٹریوں میں نمونے اسٹور کریں اور ایک ایگزیکٹو سمری شائع کریں
  دستاویزات/ڈیوریل منٹ میں۔
- کک آف جائزہ سے پہلے ثبوت کے بنڈل کو گورننس ٹکٹ سے لنک کریں۔

## سیشن کی سہولت اور ثبوت ہینڈ آف

- ** ماڈریٹر ٹائم لائن: **
  - T-24 H- پروگرام مینجمنٹ نے یاد دہانی + شیڈول/حاضری سنیپ شاٹ کو `#nexus-steering` پر پوسٹ کیا۔
  - T-2 H- نیٹ ورکنگ TL GAR ٹیلی میٹری اسنیپ شاٹ کو اپ ڈیٹ کرتا ہے اور `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` میں ڈیلٹا کو ریکارڈ کرتا ہے۔
  - T-15 M- OPS آٹومیشن کی جانچ پڑتال کی تیاری کی تیاری اور `artifacts/sorafs_gateway_dns/current` پر فعال رن ID لکھتی ہے۔
  - کال کے دوران - ماڈریٹر اس رن بک کو شیئر کرتا ہے اور براہ راست مصنف تفویض کرتا ہے۔ دستاویزات/ڈیوریل ایکشن آئٹمز ان لائن پر قبضہ کرتے ہیں۔
- ** منٹ ٹیمپلیٹ: ** کنکال کے کاپی کریں
  `docs/source/sorafs_gateway_dns_design_minutes.md` (بنڈل میں بھی عکس ہے
  پورٹل کا) اور فی سیشن میں ایک آبادی والی مثال کا ارتکاب کرتا ہے۔ کی فہرست شامل کریں
  شرکاء ، فیصلے ، ایکشن آئٹمز ، ثبوت ہیش اور بقایا خطرات۔
- ** شواہد اپ لوڈ: ** زپ `runbook_bundle/` ٹیسٹ کی ڈائرکٹری ،
  مہی .ا منٹ پی ڈی ایف ، لاگ ان SHA-256 ہیشوں کو منٹ + پر منسلک کریں
  ایجنڈا ، اور پھر اپ لوڈ کرنے پر گورننس کے جائزہ لینے والوں کو عرف کو مطلع کریں
  `s3://sora-governance/sorafs/gateway_dns/<date>/` پر پہنچیں۔

## ثبوت سنیپ شاٹ (مارچ 2025 کک آف)

روڈ میپ اور منٹ میں تازہ ترین ریہرسل/براہ راست نمونے کا حوالہ دیا گیا ہے
وہ بالٹی `s3://sora-governance/sorafs/gateway_dns/` میں ہیں۔ ذیل میں ہیشس
کیننیکل منشور (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`) کی عکسبندی کریں۔

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
  - _ (زیر التواء اپ لوڈ: `gateway_dns_minutes_20250303.pdf`- جب پیش کردہ پی ڈی ایف بنڈل میں پہنچے گا تو دستاویزات/ڈیوریل SHA-256 کو شامل کریں گے۔) _

## متعلقہ مواد- [گیٹ وے آپریشنز پلے بوک] (./operations-playbook.md)
- [SoraFS مشاہداتی منصوبہ] (./observability-plan.md)
- [وکندریقرت DNS ٹریکر اور گیٹ وے] (https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)