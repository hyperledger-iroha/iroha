---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-ops.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: پن رجسٹری-او پی ایس
عنوان: رجسٹری پن آپریشنز
سائڈبار_لیبل: رجسٹری پن آپریشنز
تفصیل: مانیٹر اور ٹریجز SoraFS رجسٹری پن اور نقل SLA میٹرکس۔
---

::: نوٹ کینونیکل ماخذ
`docs/source/sorafs/runbooks/pin_registry_ops.md` کی عکاسی کرتا ہے۔ جب تک میراثی اسفنکس دستاویزات ریٹائر نہ ہو تب تک دونوں ورژن کو مطابقت پذیری میں رکھیں۔
:::

## خلاصہ

اس رن بک نے دستاویز کیا ہے کہ کس طرح SoraFS پن رجسٹری اور اس کی نقل کی خدمت کی سطح کے معاہدوں (SLAs) کی نگرانی اور اس کی آزمائش کی جائے۔ میٹرکس `iroha_torii` سے آتی ہے اور Prometheus کے ذریعے نام کی جگہ `torii_sorafs_*` کے تحت برآمد کی جاتی ہے۔ Torii پس منظر میں ہر 30 سیکنڈ میں رجسٹری اسٹیٹ کے نمونے لگاتے ہیں ، لہذا ڈیش بورڈز تازہ ترین رہتے ہیں یہاں تک کہ جب کوئی آپریٹر `/v1/sorafs/pin/*` اختتامی مقامات سے استفسار نہیں کررہا ہے۔ استعمال کرنے میں تیار Grafana لے آؤٹ کے لئے کیوریٹڈ ڈیش بورڈ (`docs/source/grafana_sorafs_pin_registry.json`) درآمد کریں جو براہ راست مندرجہ ذیل حصوں میں نقشہ بناتا ہے۔

## میٹرکس حوالہ

| میٹرک | لیبل | تفصیل |
| ------ | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \ | `approved` \ | `retired`) | لائف سائیکل اسٹیٹ کے ذریعہ آن چین کی انوینٹری۔ |
| `torii_sorafs_registry_aliases_total` | - | رجسٹری میں رجسٹرڈ فعال منشور عرفی ناموں کی گنتی۔ |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \ | `completed` \ | `expired`) | حیثیت کے ذریعہ منقسم نقل کے احکامات کا بیکلاگ۔ |
| `torii_sorafs_replication_backlog_total` | - | سہولت گیج کی عکاسی کرنے والے احکامات `pending`۔ |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \ | `missed` \ | `pending`) | SLA اکاؤنٹنگ: `met` گنتی کے احکامات آخری تاریخ کے اندر مکمل ہوئے ، `missed` دیر سے تکمیل + میعاد ختمیاں شامل کرتا ہے ، `pending` زیر التواء احکامات کی عکاسی کرتا ہے۔ |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \ | `p95` \ | `max` \ | `count`) | مجموعی تکمیل میں تاخیر (مسئلے اور تکمیل کے درمیان عہد)۔ |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \ | `p95` \ | `max` \ | `count`) | زیر التواء آرڈر سلیک ونڈوز (ڈیڈ لائن مائنس جاری کرنے کی مدت)۔ |

تمام گیجز ہر اسنیپ شاٹ پل پر دوبارہ ترتیب دیئے جاتے ہیں ، لہذا ڈیش بورڈز کو `1m` کی شرح یا تیز تر نمونہ لینا چاہئے۔

## Grafana ڈیش بورڈ

JSON ڈیش بورڈ میں سات پینل شامل ہیں جو آپریٹر کے ورک فلوز کا احاطہ کرتے ہیں۔ اگر آپ کسٹم چارٹ بنانے کو ترجیح دیتے ہیں تو فوری حوالہ کے لئے سوالات ذیل میں درج ہیں۔

1. ** ظاہر زندگی سائیکل ** - `torii_sorafs_registry_manifests_total` (`status` کے ذریعہ گروپ کیا گیا)۔
2. ** عرف کیٹلاگ ٹرینڈ ** - `torii_sorafs_registry_aliases_total`۔
3.
4.
5. ** ایس ایل اے کامیابی کی شرح ** -

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```6. جب آپ کو مطلق سلیک فلور کی ضرورت ہو تو `min_over_time` خیالات کو شامل کرنے کے لئے Grafana تبدیلیوں کا استعمال کریں ، مثال کے طور پر:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. ** ناکام احکامات (1h کی شرح) ** -

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## انتباہ دہلیز

- ** ایس ایل اے کامیابی  0 **
  - دہلیز: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - ایکشن: فراہم کنندگان کے منشور کی تصدیق کے لئے گورننس کے ظاہر ہونے کا معائنہ کریں۔
- ** P95 تکمیل> اوسط ڈیڈ لائن کلیئرنس **
  - دہلیز: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - ایکشن: تصدیق کریں کہ فراہم کنندگان آخری تاریخ سے پہلے تعمیل کرتے ہیں۔ دوبارہ تفویض پر غور کریں۔

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
          summary: "SLA de replicación de SoraFS por debajo del objetivo"
          description: "El ratio de éxito del SLA se mantuvo por debajo de 95% durante 15 minutos."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "Backlog de replicación de SoraFS por encima del umbral"
          description: "Las órdenes pendientes excedieron el presupuesto de backlog configurado."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "Órdenes de replicación de Prometheus expiradas"
          description: "Al menos una orden de replicación expiró en los últimos cinco minutos."
```

## ٹریج فلو

1. ** وجہ کی شناخت کریں **
   - اگر ایس ایل اے کی کمی محسوس ہوتی ہے جبکہ بیک بلاگ کم رہتا ہے تو ، فراہم کنندہ کی کارکردگی (PUR ناکامیوں ، دیر سے تکمیلات) پر توجہ دیں۔
   - اگر بیک بلاگ مستحکم مسوں کے ساتھ بڑھتا ہے تو ، بورڈ کی منظوری کے زیر التواء ظاہر ہونے کی تصدیق کے لئے داخلہ (`/v1/sorafs/pin/*`) کا معائنہ کریں۔
2. ** فراہم کنندہ کی حیثیت کو درست کریں **
   - `iroha app sorafs providers list` چلائیں اور تصدیق کریں کہ اشتہاری صلاحیتیں نقل کی ضروریات کو پورا کرتی ہیں۔
   - فراہمی گیب اور پور کامیابی کی تصدیق کے لئے گیجز `torii_sorafs_capacity_*` چیک کریں۔
3. ** دوبارہ نقل کی اصلاح **
   - `sorafs_manifest_stub capacity replication-order` کے ذریعے نئے آرڈر جاری کریں جب بیکلاگ سلیک (`stat="avg"`) 5 عہدوں سے نیچے گرتا ہے (مینی فیسٹ/کار پیکیجنگ `iroha app sorafs toolkit pack` استعمال کرتا ہے)۔
   - گورننس کو مطلع کریں اگر عرفیتوں میں فعال مینی فیسٹ پابندیوں کی کمی ہے (غیر متوقع `torii_sorafs_registry_aliases_total` قطرے)۔
4. ** دستاویز کا نتیجہ **
   - متاثرہ مینی فیسٹ ٹائم اسٹیمپ اور ہضموں کے ساتھ SoraFS کے آپریشنز لاگ میں واقعے کے ریکارڈ نوٹ۔
   - اس رن بک کو اپ ڈیٹ کریں اگر نئی ناکامی کے طریقوں یا ڈیش بورڈز ظاہر ہوں۔

## تعیناتی کا منصوبہ

اس مرحلہ وار طریقہ کار پر عمل کریں جب پیداوار میں عرف کیچنگ پالیسی کو چالو یا سخت کرتے ہو:1. ** ترتیب تیار کریں **
   - `iroha_config` (صارف -> موجودہ) میں `torii.sorafs_alias_cache` کو اپ ڈیٹ کریں TTL اور گریس ونڈوز کے ساتھ: `positive_ttl` ، `refresh_window` ، `hard_expiry` ، `negative_ttl` ، `negative_ttl` ، I180084X ، I180000000000000000000000000000000000000000 `successor_grace` اور `governance_grace`۔ ڈیفالٹس `docs/source/sorafs_alias_policy.md` میں پالیسی سے ملتے ہیں۔
   - ایس ڈی کے کے ل their ، وہی اقدار کو ان کی کنفیگریشن پرتوں (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` میں مورچا / نیپی / ازگر بائنڈنگ) کے ذریعے تقسیم کریں تاکہ کلائنٹ کی درخواست گیٹ وے سے مماثل ہو۔
2. ** اسٹیجنگ میں خشک رن **
   - تشکیل کی تبدیلی کو ایک اسٹیجنگ کلسٹر میں تعینات کریں جو پروڈکشن ٹوپولوجی کی عکاسی کرتا ہے۔
   - `cargo xtask sorafs-pin-fixtures` کو چلائیں اس بات کی تصدیق کے لئے کہ عرف کیننیکل فکسچر اب بھی ڈیکوڈ اور راؤنڈ ٹرپ ؛ کسی بھی مماثلت کا مطلب اپ اسٹریم ڈرفٹ کا مطلب ہے جسے پہلے حل کرنا چاہئے۔
   -ورزش کے اختتامی مقامات `/v1/sorafs/pin/{digest}` اور `/v1/sorafs/aliases` مصنوعی ٹیسٹوں کے ساتھ تازہ ، ریفریش ونڈو ، میعاد ختم ہونے اور سخت برآمد ہونے والے معاملات کا احاطہ کرتے ہیں۔ اس رن بک کے خلاف HTTP کوڈز ، ہیڈر (`Sora-Proof-Status` ، `Retry-After` ، `Warning`) اور JSON باڈی فیلڈز کی توثیق کرتا ہے۔
3. ** پیداوار میں قابل بنائیں **
   - معیاری تبدیلیوں والی ونڈو میں نئی ​​ترتیب دکھاتا ہے۔ پہلے اس کا اطلاق Torii پر کریں اور پھر نوڈ میں نئی ​​پالیسی کی تصدیق کرنے کے بعد گیٹ ویز/SDK خدمات کو دوبارہ شروع کریں۔
   - `docs/source/grafana_sorafs_pin_registry.json` کو Grafana (یا موجودہ ڈیش بورڈز کو اپ ڈیٹ کریں) میں درآمد کریں اور عرف کیچ ریفریش پینلز کو NOC ورک اسپیس پر سیٹ کریں۔
4. ** تعیناتی کی تصدیق **
   - 30 منٹ کے لئے `torii_sorafs_alias_cache_refresh_total` اور `torii_sorafs_alias_cache_age_seconds` پر نگرانی کرتا ہے۔ `error`/`expired` منحنی خطوط میں ریفریش ونڈوز سے وابستہ ہونا چاہئے۔ غیر متوقع نمو کا مطلب یہ ہے کہ آپریٹرز کو جاری رکھنے سے پہلے عرف ٹیسٹ اور فراہم کنندہ کی صحت کا معائنہ کرنا ہوگا۔
   - اس بات کی تصدیق کریں کہ کلائنٹ سائیڈ لاگز وہی پالیسی فیصلے دکھاتے ہیں (جب ٹیسٹ باسی یا میعاد ختم ہوجاتا ہے تو SDKs غلطیاں دکھائیں گے)۔ مؤکل سے انتباہات کی عدم موجودگی خراب ترتیب کی نشاندہی کرتی ہے۔
5. ** فال بیک **
   - اگر عرفیہ کے اجراء میں تاخیر ہوتی ہے اور ریفریش ونڈو کثرت سے فائر ہوتی ہے تو ، تشکیل میں `refresh_window` اور `positive_ttl` میں اضافہ کرکے اور دوبارہ عمل میں `positive_ttl` کو عارضی طور پر آرام کرتا ہے۔ `hard_expiry` برقرار رکھیں تاکہ واقعی باسی ٹیسٹوں کو مسترد کردیا جائے۔
   - پچھلی `iroha_config` اسنیپ شاٹ کو بحال کرکے پچھلی ترتیب کی طرف لوٹائیں اگر ٹیلی میٹری اب بھی اعلی `error` گنتی دکھاتا ہے تو ، پھر عرف جنریشن میں تاخیر کو ٹریک کرنے کے لئے ایک واقعہ کھولیں۔

## متعلقہ مواد

- `docs/source/sorafs/pin_registry_plan.md` - عمل درآمد روڈ میپ اور گورننس سیاق و سباق۔
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` - اسٹوریج ورکر آپریشنز ، اس رجسٹری پلے بک کو پورا کرتا ہے۔