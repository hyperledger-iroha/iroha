---
lang: ur
direction: rtl
source: docs/portal/docs/sns/address-checksum-runbook.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

<div dir="rtl">

:::note اصل نسخہ
یہ صفحہ `docs/source/sns/address_checksum_failure_runbook.md` کا خلاصہ ہے۔ پہلے
ماخذ فائل اپ ڈیٹ کریں، پھر یہ کاپی sync کریں۔
:::

Torii، SDKs، اور والٹ/ایکسپلورر کلائنٹس پر `ERR_CHECKSUM_MISMATCH`
(`ChecksumMismatch`) ظاہر ہوتے ہی ADDR-6/ADDR-7 رن بک فعال کرنا لازم ہے۔

## یہ play کب چلائیں

- **الرٹس:** `AddressInvalidRatioSlo` (`dashboards/alerts/address_ingest_rules.yml`)
  میں `reason="ERR_CHECKSUM_MISMATCH"` ظاہر ہو۔
- **فکسچر ڈرفٹ:** `account_address_fixture_status` Prometheus فائل یا Grafana
  ڈیش بورڈ کسی SDK کے لیے mismatch رپورٹ کرے۔
- **سپورٹ اسکیلیشنز:** والٹ/ایکسپلورر/SDK ٹیمیں چیک سم ایررز، IME کرپشن، یا
  clipboard اسکین ناکامیوں کی اطلاع دیں۔
- **ہاتھوں کا مشاہدہ:** پروڈکشن Torii لاگز میں مسلسل
  `address_parse_error=checksum_mismatch` نظر آئے۔

	Local-8/Local-12 کولیشنز کے لیے متعلقہ playbooks استعمال کریں۔

## شواہد کی چیک لسٹ

| ثبوت | کمانڈ / مقام | نوٹس |
|------|--------------|-------|
| Grafana اسنیپ شاٹ | `dashboards/grafana/address_ingest.json` | invalid وجوہ اور متاثرہ endpoints محفوظ کریں۔ |
| Alert payload | PagerDuty/Slack + `dashboards/alerts/address_ingest_rules.yml` | context لیبل اور ٹائم اسٹیمپ شامل کریں۔ |
| فکسچر صحت | `artifacts/account_fixture/address_fixture.prom` + Grafana | ثابت کرتا ہے کہ SDK کی کاپیاں canonical JSON سے نہیں ہٹیں۔ |
| PromQL | `sum by (context) (increase(torii_address_invalid_total{reason="ERR_CHECKSUM_MISMATCH"}[5m]))` | انسیڈنٹ رپورٹ کے لیے CSV ایکسپورٹ کریں۔ |
| لاگز | `journalctl -u iroha_torii --since -30m | rg 'checksum_mismatch'` | PII ہٹانے کے بعد شیئر کریں۔ |
| فکسچر ویری فیکیشن | `cargo xtask address-vectors --verify` | جنریٹر اور کمیٹیڈ JSON کی یکسانیت۔ |
| SDK parity | `python3 scripts/account_fixture_helper.py check ...` | الرٹ میں مذکور ہر SDK کے لیے چلائیں۔ |
| Clipboard/IME چیک | `iroha tools address inspect <literal>` | چھپے حروف یا IME تبدیلیوں کی شناخت۔ |

## فوری ردِعمل

1. الرٹ کا اعتراف کریں، Grafana + PromQL لنکس انسیڈنٹ تھریڈ میں دیں، اور
   متاثرہ Torii contexts نوٹ کریں۔
2. manifest promotions اور address parsing والے SDK ریلیز روک دیں۔
3. ڈیش بورڈ اسنیپ شاٹس اور Prometheus artefacts کو
   `docs/source/sns/incidents/YYYY-MM/<ticket>/` میں محفوظ کریں۔
4. `checksum_mismatch` دکھانے والے لاگ نمونے جمع کریں۔
5. `#sdk-parity` میں SDK مالکان کو اطلاع دیں تاکہ فوری triage ہو سکے۔

## وجہ کی علیحدگی

### فکسچر یا جنریٹر ڈرفٹ
- `cargo xtask address-vectors --verify` دوبارہ چلائیں؛ ناکامی پر ری جنریٹ کریں۔
- ہر SDK کے لیے `ci/account_fixture_metrics.sh` یا `scripts/account_fixture_helper.py check`
  چلائیں تاکہ بنڈل canonical JSON سے میچ کرے۔

### کلائنٹ انکوڈر / IME ریگریشن
- `iroha tools address inspect` کے ذریعے زیرو وِد، kana یا truncate شدہ strings پکڑیں۔
- `docs/source/sns/address_display_guidelines.md` کے مطابق UX بہاؤ دوبارہ چیک کریں۔

### Manifest یا رجسٹری مسائل
- `address_manifest_ops.md` فالو کریں تاکہ تازہ ترین manifest bundle validate ہو
  اور Local-8 selectors واپس نہ آئیں۔

### بدنیتی پر مبنی/خراب ٹریفک
- Torii لاگز اور `torii_http_requests_total` کے ذریعے حملہ آور IPs/app IDs
  شناخت کریں اور کم از کم 24 گھنٹے کے لاگز محفوظ رکھیں۔

## اصلاح اور بحالی

| منظر | اقدامات |
|------|---------|
| فکسچر ڈرفٹ | `fixtures/account/address_vectors.json` ری جنریٹ کریں، `cargo xtask address-vectors --verify` دوبارہ چلائیں، SDK بنڈلز اپ ڈیٹ کریں، اور `address_fixture.prom` ٹکٹ میں شامل کریں۔ |
| SDK/کلائنٹ ریگریشن | canonical فکسچر اور `iroha tools address inspect` آؤٹ پٹ کے ساتھ ایشوز فائل کریں اور ریلیزز کو `ci/check_address_normalize.sh` جیسی parity CI کے پیچھے گیٹ کریں۔ |
| بدنیتی سبمشنز | مسئلہ پیدا کرنے والے principals کو ریٹ لمٹ/بلاک کریں، اور selector tombstone کی ضرورت ہو تو گورننس کو escalate کریں۔ |

اصلاحات کے بعد PromQL دوبارہ چلائیں تاکہ `ERR_CHECKSUM_MISMATCH`
کم از کم 30 منٹ تک صفر رہے (`/tests/*` مستثنیٰ)۔

## اختتام

1. Grafana اسنیپ شاٹس، PromQL CSV، لاگ اقتباسات، اور `address_fixture.prom` آرکائیو کریں۔
2. `status.md` (ADDR سیکشن) اور روڈ میپ قطار اپ ڈیٹ کریں۔
3. `docs/source/sns/incidents/` کے تحت پوسٹ انسیڈنٹ نوٹس شامل کریں۔
4. SDK ریلیز نوٹس میں checksum فکسز کا ذکر کریں جہاں موزوں ہو۔
5. الرٹ صرف تب بند کریں جب 24 گھنٹے تک الرٹ سبز رہے اور تمام فکسچر چیکس پاس ہوں۔

</div>
