<!-- Auto-generated stub for Urdu (ur) translation. Replace this content with the full translation. -->

---
lang: ur
direction: rtl
source: docs/source/nexus_cross_dataspace_localnet.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2324cfc7b086ceb96317eb2260abe41101f17e5c0749d0a1d28ffbf4cb5e8e45
source_last_modified: "2026-02-19T18:33:20.275472+00:00"
translation_last_reviewed: 2026-04-02
translator: machine-google-reviewed
---

# Nexus کراس ڈیٹا اسپیس لوکل نیٹ پروف

یہ رن بک Nexus انضمام کے ثبوت کو انجام دیتی ہے کہ:

- دو محدود نجی ڈیٹا اسپیس (`ds1`، `ds2`) کے ساتھ 4-پیر لوکل نیٹ کو بوٹ کرتا ہے،
- ہر ڈیٹا اسپیس میں ٹریفک کا حساب لگاتا ہے،
- ہر ڈیٹا اسپیس میں ایک اثاثہ بناتا ہے،
- دونوں سمتوں میں ڈیٹا اسپیس میں ایٹم سویپ سیٹلمنٹ کو انجام دیتا ہے،
- انڈر فنڈڈ ٹانگ جمع کر کے اور بیلنس چیک کر کے رول بیک سیمنٹکس کو ثابت کرتا ہے۔

کینونیکل ٹیسٹ یہ ہے:
`nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing`۔

## فوری رن

ریپوزٹری روٹ سے ریپر اسکرپٹ کا استعمال کریں:

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh
```

پہلے سے طے شدہ سلوک:

- صرف کراس ڈیٹا اسپیس پروف ٹیسٹ چلاتا ہے،
- `NORITO_SKIP_BINDINGS_SYNC=1` سیٹ کرتا ہے،
- `IROHA_TEST_SKIP_BUILD=1` سیٹ کرتا ہے،
- `--test-threads=1` استعمال کرتا ہے،
- `--nocapture` پاس کرتا ہے۔

## مفید اختیارات

```bash
scripts/run_nexus_cross_dataspace_atomic_swap.sh --keep-dirs
scripts/run_nexus_cross_dataspace_atomic_swap.sh --no-skip-build
scripts/run_nexus_cross_dataspace_atomic_swap.sh --release
scripts/run_nexus_cross_dataspace_atomic_swap.sh --all-nexus
```

- `--keep-dirs` فرانزک کے لیے عارضی ہم مرتبہ ڈائریکٹریز (`IROHA_TEST_NETWORK_KEEP_DIRS=1`) رکھتا ہے۔
- `--all-nexus` `mod nexus::` (مکمل Nexus انٹیگریشن سبسیٹ) چلاتا ہے، نہ صرف ثبوت ٹیسٹ۔

## سی آئی گیٹ

CI مددگار:

```bash
ci/check_nexus_cross_dataspace_localnet.sh
```

ہدف بنائیں:

```bash
make check-nexus-cross-dataspace
```

یہ گیٹ ڈیٹرمنسٹک پروف ریپر کو چلاتا ہے اور کام کو ناکام کرتا ہے اگر کراس ڈیٹا اسپیس ایٹم
تبادلہ کا منظر نامہ واپس آتا ہے۔

## دستی مساوی کمانڈز

ھدف شدہ ثبوت ٹیسٹ:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod \
  nexus::cross_dataspace_localnet::cross_dataspace_atomic_swap_is_all_or_nothing \
  -- --nocapture --test-threads=1
```

مکمل Nexus سب سیٹ:

```bash
IROHA_TEST_SKIP_BUILD=1 NORITO_SKIP_BINDINGS_SYNC=1 \
  cargo test -p integration_tests --test mod nexus:: -- --nocapture --test-threads=1
```

## متوقع ثبوت سگنل- امتحان پاس ہوتا ہے۔
- ایک متوقع انتباہ دانستہ طور پر ناکام ہونے والے کم فنڈ والے سیٹلمنٹ ٹانگ کے لیے ظاہر ہوتا ہے:
  `settlement leg requires 10000 but only ... is available`۔
- حتمی بیلنس کے دعوے اس کے بعد کامیاب ہوتے ہیں:
  - کامیاب فارورڈ سویپ،
  - کامیاب ریورس سویپ،
  - ناکام انڈر فنڈڈ سویپ (رول بیک غیر تبدیل شدہ بیلنس)۔

## موجودہ توثیق کا سنیپ شاٹ

**19 فروری 2026** تک، یہ ورک فلو اس کے ساتھ گزر گیا:

- ھدف بنائے گئے ٹیسٹ: `1 passed; 0 failed`،
- مکمل Nexus سب سیٹ: `24 passed; 0 failed`۔