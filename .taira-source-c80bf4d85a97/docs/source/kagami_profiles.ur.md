---
lang: ur
direction: rtl
source: docs/source/kagami_profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 061304711d940567ec3c15a75c388085e65aafc6962abc2da6e943fa9a9903fa
source_last_modified: "2026-01-27T18:39:03.379028+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Kagami IROHA3 پروفائلز

Kagami جہاز Iroha 3 نیٹ ورکس کے لئے پریسیٹس تاکہ آپریٹرز ڈٹرمینسٹک پر مہر لگاسکیں
جینیسیس فی نیٹ ورک نوبس کے بغیر جگائے بغیر ظاہر ہوتی ہے۔

- پروفائلز: `iroha3-dev` (چین `iroha3-dev.local` ، جمع کرنے والے K = 1 R = 1 ، جب NPOS منتخب کیا جاتا ہے تو چین ID سے اخذ کردہ VRF بیج) ، `iroha3-taira` ، جب NPOS ، 000012x ، جمع کرنے والوں کے K = 3 R = 3 کی ضرورت ہوتی ہے۔ `iroha3-nexus` (چین `iroha3-nexus` ، جمع کرنے والے K = 5 R = 3 ، جب NPOS منتخب کیا جاتا ہے تو `--vrf-seed-hex` کی ضرورت ہوتی ہے)۔
۔ اجازت شدہ IROHA3 تعیناتیوں کو بغیر SORA پروفائل کے چلانا چاہئے۔
- جنریشن: `cargo run -p iroha_kagami -- genesis generate --profile <profile> --ivm-dir . --genesis-public-key <pk> --consensus-mode <npos|permissioned> [--vrf-seed-hex <hex>]`۔ Nexus کے لئے `--consensus-mode npos` استعمال کریں ؛ `--vrf-seed-hex` صرف NPOs کے لئے موزوں ہے (ٹیسٹوس/گٹھ جوڑ کے لئے ضروری ہے)۔ Kagami پنوں DA/RBC IROHA3 لائن پر اور ایک سمری (چین ، جمع کرنے والے ، DA/RBC ، VRF بیج ، فنگر پرنٹ) خارج کرتا ہے۔
- توثیق: `cargo run -p iroha_kagami -- verify --profile <profile> --genesis <path> [--vrf-seed-hex <hex>]` دوبارہ عمل کی توقعات (چین ID ، DA/RBC ، جمع کرنے والے ، پاپ کوریج ، اتفاق رائے فنگر پرنٹ)۔ سپلائی `--vrf-seed-hex` صرف اس وقت جب کسی NPOs کی تصدیق کی جائے تو Tairas/Nexus کے لئے ظاہر ہوتا ہے۔
-نمونہ بنڈل: پہلے سے تیار کردہ بنڈل `defaults/kagami/iroha3-{dev,taira,nexus}/` (جینیسس.جسن ، config.toml ، docker-compose.yml ، تصدیق. txt ، Readme) کے تحت رہتے ہیں۔ `cargo xtask kagami-profiles [--profile <name>|all] [--out <dir>] [--kagami <bin>]` کے ساتھ دوبارہ تخلیق کریں۔
- موچی: `mochi`/`mochi-genesis` `--genesis-profile <profile>` اور `--vrf-seed-hex <hex>` (صرف NPOS) کو قبول کریں ، انہیں Kagami پر بھیجیں ، اور اسی طرح Kagami کا استعمال STDOUT/STDER پر کریں۔

ٹوپولوجی اندراجات کے ساتھ ساتھ بنڈل ایمبیڈ بی ایل ایس پاپس ہیں لہذا `kagami verify` کامیاب ہوتا ہے
باکس سے باہر ؛ قابل اعتماد ساتھیوں/بندرگاہوں کو ترتیب میں ایڈجسٹ کریں جیسا کہ مقامی کے لئے ضرورت ہے
دھواں چلتا ہے۔