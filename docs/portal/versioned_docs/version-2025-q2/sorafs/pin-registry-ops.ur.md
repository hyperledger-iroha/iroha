---
lang: ur
direction: rtl
source: docs/portal/versioned_docs/version-2025-q2/sorafs/pin-registry-ops.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0dc64bb4067d734250852a74a65a2100bd68e5ff35f9e8e9dbf3bd2b86f00cfa
source_last_modified: "2026-01-22T15:38:30.656337+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
id: pin-registry-ops-ur
slug: /sorafs/pin-registry-ops-ur
---

::: نوٹ کینونیکل ماخذ
آئینے `docs/source/sorafs/runbooks/pin_registry_ops.md`۔ دونوں ورژن کو ریلیز میں جوڑیں۔
:::

## جائزہ

یہ رن بک دستاویز کرتی ہے کہ کس طرح SoraFS پن رجسٹری اور اس کی نقل کی خدمت کی سطح کے معاہدوں (SLAs) کی نگرانی اور ان کی جانچ پڑتال کی جائے۔ میٹرکس `iroha_torii` سے شروع ہوتا ہے اور Prometheus کے ذریعے `torii_sorafs_*` نام کی جگہ کے تحت برآمد کیا جاتا ہے۔ Torii پس منظر میں 30 سیکنڈ کے وقفے پر رجسٹری اسٹیٹ کے نمونے کرتا ہے ، لہذا ڈیش بورڈز موجودہ رہتے ہیں یہاں تک کہ جب کوئی آپریٹر `/v1/sorafs/pin/*` اختتامی نقطہ پر پولنگ نہیں کررہے ہیں۔ تیار شدہ ڈیش بورڈ (`docs/source/grafana_sorafs_pin_registry.json`) کو استعمال کرنے کے لئے تیار Grafana لے آؤٹ کے لئے درآمد کریں جو نیچے والے حصوں میں براہ راست نقشہ بناتا ہے۔

## میٹرک حوالہ

| میٹرک | لیبل | تفصیل |
| ------ | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \ | `approved` \ | `retired`) | لائف سائیکل اسٹیٹ کے ذریعہ آن چین کی منشور انوینٹری۔ |
| `torii_sorafs_registry_aliases_total` | - | رجسٹری میں ریکارڈ شدہ فعال منشور کے عرفی ناموں کی گنتی۔ |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \ | `completed` \ | `expired`) | | ریپلیکشن آرڈر بیکلاگ اسٹیٹس کے ذریعہ منقسم ہے۔ |
| `torii_sorafs_replication_backlog_total` | - | سہولت گیج آئینہ دار `pending` آرڈرز۔ |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \ | `missed` \ | `pending`) | SLA اکاؤنٹنگ: `met` گنتی نے آخری تاریخ کے اندر آرڈرز مکمل کیے ، `missed` دیر سے تکمیل + میعاد ختم ہونے ، `pending` آئینہ بقایا آرڈرز۔ |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \ | `p95` \ | `max` \ | `count`) | مجموعی تکمیل میں تاخیر (اجراء اور تکمیل کے درمیان عہد)۔ |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \ | `p95` \ | `max` \ | `count`) | زیر التواء آرڈر سلیک ونڈوز (ڈیڈ لائن مائنس نے عہد جاری کیا)۔ |

تمام گیجز ہر اسنیپ شاٹ پل پر دوبارہ سیٹ ہوجاتے ہیں ، لہذا ڈیش بورڈز کو `1m` کیڈینس یا تیز تر نمونہ لینا چاہئے۔

## Grafana ڈیش بورڈ

ڈیش بورڈ JSON سات پینل کے ساتھ جہاز چلا رہا ہے جو آپریٹر کے ورک فلوز کا احاطہ کرتے ہیں۔ اگر آپ بیسپوک چارٹ بنانے کو ترجیح دیتے ہیں تو فوری حوالہ کے لئے سوالات ذیل میں درج ہیں۔

1. ** ظاہر لائف سائیکل ** - `torii_sorafs_registry_manifests_total` (`status` کے ذریعہ گروپ کیا گیا)۔
2. ** عرف کیٹلاگ ٹرینڈ ** - `torii_sorafs_registry_aliases_total`۔
3.
4.
5. ** ایس ایل اے کامیابی کا تناسب ** -

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. جب آپ کو مطلق سلیک فلور کی ضرورت ہو تو `min_over_time` خیالات کو شامل کرنے کے لئے Grafana تبدیلیوں کا استعمال کریں ، مثال کے طور پر:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. ** کھوئے ہوئے احکامات (1h کی شرح) ** -

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## انتباہ دہلیز- ** ایس ایل اے کی کامیابی  0 **
  - دہلیز: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - ایکشن: گورننس کا معائنہ کریں تاکہ فراہم کنندہ کے منشور کی تصدیق کی جاسکے۔
- ** تکمیل P95> ڈیڈ لائن سلیک اوسط **
  - دہلیز: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - ایکشن: تصدیق فراہم کرنے والے ڈیڈ لائن سے پہلے کا ارتکاب کر رہے ہیں۔ دوبارہ تفویض جاری کرنے پر غور کریں۔

### مثال Prometheus قواعد

```yaml
groups:
  - name: sorafs-pin-registry
    rules:
      - alert: SorafsReplicationSlaDrop
        expr: sum(torii_sorafs_replication_sla_total{outcome="met"}) /
          clamp_min(sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}), 1) < 0.95
        for: 15m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication SLA below target"
          description: "SLA success ratio stayed under 95% for 15 minutes."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "SoraFS replication backlog above threshold"
          description: "Pending replication orders exceeded the configured backlog budget."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "SoraFS replication orders expired"
          description: "At least one replication order expired in the last five minutes."
```

## ٹریج ورک فلو

1. ** وجہ کی شناخت کریں **
   - اگر ایس ایل اے اسپائک کو یاد کرتا ہے جبکہ بیکلاگ کم رہتا ہے تو ، فراہم کنندہ کی کارکردگی (POR ناکامیوں ، دیر سے تکمیلات) پر توجہ دیں۔
   - اگر بیک بلاگ مستحکم مسوں کے ساتھ بڑھتا ہے تو ، کونسل کی منظوری کے منتظر ظاہر ہونے کی تصدیق کے لئے داخلہ (`/v1/sorafs/pin/*`) کا معائنہ کریں۔
2. ** فراہم کنندہ کی حیثیت کو درست کریں **
   - `iroha app sorafs providers list` چلائیں اور تصدیق کریں کہ اشتہار کی صلاحیتوں سے مماثل نقل کی ضروریات ہیں۔
   - فراہمی شدہ GIB اور POR کامیابی کی تصدیق کے لئے `torii_sorafs_capacity_*` گیجز چیک کریں۔
3. ** دوبارہ نقل کی اصلاح **
   - `sorafs_manifest_stub capacity replication-order` کے ذریعے نئے آرڈر جاری کریں جب بیکلاگ سلیک (`stat="avg"`) 5 عہدوں سے نیچے گرتا ہے (مینی فیسٹ/کار پیکیجنگ `iroha app sorafs toolkit pack` استعمال کرتا ہے)۔
   - گورننس کو مطلع کریں اگر عرفیتوں میں فعال مینی فیسٹ پابندیوں کا فقدان ہے (`torii_sorafs_registry_aliases_total` غیر متوقع طور پر گرتا ہے)۔
4. ** دستاویز کا نتیجہ **
   - SoraFS آپریشنز میں ریکارڈ واقعے کے نوٹ ٹائم اسٹیمپ اور متاثرہ ظاہر ہضموں کے ساتھ لاگ ان کریں۔
   - اس رن بک کو اپ ڈیٹ کریں اگر نئے ناکامی کے طریقوں یا ڈیش بورڈز متعارف کروائے جائیں۔

## رول آؤٹ پلان

جب پیداوار میں عرف کیشے کی پالیسی کو چالو یا سخت کرتے ہو تو اس مرحلے کے طریقہ کار پر عمل کریں:1. ** ترتیب تیار کریں **
   - `iroha_config` (صارف → اصل) میں `torii.sorafs_alias_cache` کو اپ ڈیٹ کریں TTLS اور گریس ونڈوز کے ساتھ: `positive_ttl` ، Prometheus ، `hard_expiry` ، `negative_ttl` ، `negative_ttl` ، I18000083X ، I18000083X ، I18000083X ، `negative_ttl` ، `negative_ttl` ، `negative_ttl` ، `hard_expiry` `successor_grace` ، اور `governance_grace`۔ ڈیفالٹس `docs/source/sorafs_alias_policy.md` میں پالیسی سے ملتے ہیں۔
   - ایس ڈی کے کے ل their ، وہی اقدار کو ان کی کنفیگریشن پرتوں (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` میں مورچا / نیپی / ازگر بائنڈنگ) کے ذریعے تقسیم کریں لہذا کلائنٹ کا نفاذ گیٹ وے سے مماثل ہے۔
2. ** اسٹیجنگ میں خشک رن **
   - کنفیگ چینج کو ایک اسٹیجنگ کلسٹر میں تعینات کریں جو پروڈکشن ٹوپولوجی کو آئینہ دار کرتا ہے۔
   - کیننیکل عرف فکسچر کی تصدیق کے لئے `cargo xtask sorafs-pin-fixtures` چلائیں اب بھی ڈیکوڈ اور راؤنڈ ٹرپ ؛ کسی بھی مماثلت سے upstream ظاہر ہونے والے بڑھے ہوئے بہاؤ کا مطلب ہے جس پر پہلے توجہ دی جانی چاہئے۔
   -`/v1/sorafs/pin/{digest}` اور `/v1/sorafs/aliases` اختتامی نکات کے ساتھ مصنوعی ثبوتوں کے ساتھ تازہ ، ریفریش ونڈو ، میعاد ختم ہونے اور سخت مجاز معاملات کا احاطہ کریں۔ اس رن بک کے خلاف HTTP اسٹیٹس کوڈز ، ہیڈرز (`Sora-Proof-Status` ، `Retry-After` ، `Warning`) ، اور JSON باڈی فیلڈز کی توثیق کریں۔
3. ** پیداوار میں قابل بنائیں **
   - معیاری تبدیلی ونڈو کے ذریعے نئی ترتیب کو رول کریں۔ پہلے اس کا اطلاق Torii پر پہلے کریں ، پھر نوڈ میں نئی ​​پالیسی کی تصدیق ہونے کے بعد گیٹ ویز/ایس ڈی کے خدمات کو دوبارہ شروع کریں۔
   - `docs/source/grafana_sorafs_pin_registry.json` کو Grafana (یا موجودہ ڈیش بورڈز کو اپ ڈیٹ کریں) میں درآمد کریں اور NOC ورک اسپیس میں عرف کیشے ریفریش پینلز کو پن کریں۔
4. ** تعیناتی کی تصدیق **
   - 30 منٹ کے لئے `torii_sorafs_alias_cache_refresh_total` اور `torii_sorafs_alias_cache_age_seconds` کی نگرانی کریں۔ `error`/`expired` منحنی خطوط میں اسپائکس پالیسی ریفریش ونڈوز کے ساتھ منسلک ہونا چاہئے۔ غیر متوقع نمو کا مطلب ہے کہ آپریٹرز کو جاری رکھنے سے پہلے عرف ثبوت اور فراہم کنندہ کی صحت کا معائنہ کرنا چاہئے۔
   - تصدیق کریں کہ کلائنٹ سائیڈ لاگز وہی پالیسی فیصلے دکھاتے ہیں (جب ثبوت باسی یا میعاد ختم ہونے پر SDKs غلطیوں کی سطح کو ظاہر کرے گا)۔ کلائنٹ کی انتباہات کی عدم موجودگی غلط کنفیگریشن کی نشاندہی کرتی ہے۔
5. ** فال بیک **
   - اگر عرفیہ اجراء کے پیچھے پڑ جاتا ہے اور ریفریش ونڈو کثرت سے سفر کرتا ہے تو ، تشکیل میں `refresh_window` اور `positive_ttl` میں اضافہ کرکے عارضی طور پر پالیسی کو آرام کریں ، پھر دوبارہ تعی .ن کریں۔ `hard_expiry` برقرار رکھیں تاکہ واقعی باسی ثبوتوں کو ابھی بھی مسترد کردیا گیا ہے۔
   - پچھلے `iroha_config` اسنیپ شاٹ کو بحال کرکے پہلے ترتیب میں واپس آجائیں اگر ٹیلی میٹری میں بلند `error` گنتی کو ظاہر کرنا جاری ہے تو ، عرف جنریشن میں تاخیر کا سراغ لگانے کے لئے ایک واقعہ کھولیں۔

## متعلقہ مواد

- `docs/source/sorafs/pin_registry_plan.md` - عمل درآمد روڈ میپ اور گورننس سیاق و سباق۔
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` - اسٹوریج ورکر آپریشنز ، اس رجسٹری پلے بک کو پورا کرتا ہے۔
