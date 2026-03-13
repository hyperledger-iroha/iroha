---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-ops.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: پن رجسٹری-او پی ایس
عنوان: پن رجسٹری آپریشنز
سائڈبار_لیبل: پن رجسٹری آپریشنز
تفصیل: پن رجسٹری SoraFS اور نقل SLA میٹرکس کی نگرانی اور ترتیب دیں۔
---

::: نوٹ کینونیکل ماخذ
`docs/source/sorafs/runbooks/pin_registry_ops.md` کی عکاسی کرتا ہے۔ جب تک لیگیسی اسپنکس دستاویزات ریٹائر نہ ہو تب تک دونوں ورژن کو مطابقت پذیری میں رکھیں۔
:::

## جائزہ

یہ رن بک بیان کرتی ہے کہ کس طرح پن رجسٹری SoraFS اور اس کی نقل کی خدمت کی سطح کے معاہدوں (SLAs) کی نگرانی اور ترتیب دی جائے۔ میٹرکس `iroha_torii` سے آتی ہے اور Prometheus کے ذریعے نام کی جگہ `torii_sorafs_*` کے تحت برآمد کی جاتی ہے۔ Torii نمونے رجسٹری کی حیثیت ہر 30 سیکنڈ کے پس منظر میں ، لہذا ڈیش بورڈز تازہ ترین رہتے ہیں یہاں تک کہ جب کوئی آپریٹر `/v2/sorafs/pin/*` اختتامی نقطہ نظر سے استفسار نہیں کررہا ہے۔ تیار شدہ ڈیش بورڈ (`docs/source/grafana_sorafs_pin_registry.json`) کو استعمال کرنے کے لئے تیار Grafana لے آؤٹ کے لئے درآمد کریں جو نیچے والے حصوں سے براہ راست مطابقت رکھتا ہے۔

## میٹرکس حوالہ

| میٹرک | لیبل | تفصیل |
| ------- | ------ | ----------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \ | `approved` \ | `retired`) | لائف سائیکل اسٹیٹ کے ذریعہ آن چین کی انوینٹری۔ |
| `torii_sorafs_registry_aliases_total` | - | رجسٹری میں رجسٹرڈ فعال منشور عرفی ناموں کی تعداد۔ |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \ | `completed` \ | `expired`) | حیثیت کے ذریعہ منقسم نقل کے احکامات کا بیکلاگ۔ |
| `torii_sorafs_replication_backlog_total` | - | سہولت گیج کی عکاسی کرنے والے احکامات `pending`۔ |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \ | `missed` \ | `pending`) | ایس ایل اے اکاؤنٹنگ: `met` وقت پر مکمل ہونے والے آرڈرز ، `missed` دیر سے تکمیلات + میعاد ختم ہونے ، `pending` زیر التواء احکامات کی عکاسی کرتا ہے۔ |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \ | `p95` \ | `max` \ | `count`) | مجموعی تکمیل میں تاخیر (اجراء اور تکمیل کے درمیان عہد)۔ |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \ | `p95` \ | `max` \ | `count`) | زیر التواء احکامات کے لئے مارجن ونڈوز (ڈیڈ لائن مائنس ایشو ای پیچ)۔ |

جب بھی اسنیپ شاٹ کو بازیافت کیا جاتا ہے تو تمام گیجز ری سیٹ ہوجاتے ہیں ، لہذا ڈیش بورڈز کو `1m` یا تیز کی شرح سے نمونہ لینا چاہئے۔

## ڈیش بورڈ Grafana

ڈیش بورڈ JSON آپریٹر ورک فلوز کو ڈھانپنے والے سات پینل پر مشتمل ہے۔ اگر آپ کسٹم چارٹ بنانے کو ترجیح دیتے ہیں تو فوری حوالہ کے لئے سوالات ذیل میں درج ہیں۔

1. ** ظاہر لائف سائیکل ** - `torii_sorafs_registry_manifests_total` (`status` کے ذریعہ گروپ کیا گیا)۔
2. ** عرف کیٹلاگ ٹرینڈ ** - `torii_sorafs_registry_aliases_total`۔
3.
4. ** بیکلاگ بمقابلہ میعاد ختم ہونے والے احکامات ** - سنترپتی کو اجاگر کرنے کے لئے `torii_sorafs_replication_backlog_total` اور `torii_sorafs_registry_orders_total{status="expired"}` کو جوڑتا ہے۔
5. ** ایس ایل اے کامیابی کا تناسب ** -

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```6. جب آپ کو مطلق مارجن فلور کی ضرورت ہو تو `min_over_time` خیالات کو شامل کرنے کے لئے Grafana تبدیلیوں کا استعمال کریں ، مثال کے طور پر:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. ** کھوئے ہوئے احکامات (1 گھنٹے کی شرح) ** -

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## انتباہ دہلیز

- ** ایس ایل اے کامیابی  0 **
  - دہلیز: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - ایکشن: گورننس کا معائنہ کریں تاکہ فراہم کنندہ کے منشور کی تصدیق کی جاسکے۔
- ** P95 تکمیل> اوسط ڈیڈ لائن مارجن **
  - دہلیز: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - ایکشن: چیک کریں کہ فراہم کنندگان ڈیڈ لائن سے پہلے توثیق کرتے ہیں۔ دوبارہ تفویض پر غور کریں۔

### مثال کے اصول Prometheus

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
          summary: "SLA de réplication SoraFS sous la cible"
          description: "Le ratio de succès SLA est resté sous 95% pendant 15 minutes."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "Backlog de réplication SoraFS au-dessus du seuil"
          description: "Les ordres de réplication en attente ont dépassé le budget de backlog configuré."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "Ordres de réplication Prometheus expirés"
          description: "Au moins un ordre de réplication a expiré au cours des cinq dernières minutes."
```

## ٹریج ورک فلو

1. ** وجہ کی شناخت کریں **
   - اگر ایس ایل اے کی ناکامیوں میں اضافہ ہوتا ہے جبکہ بیکلاگ کم رہتا ہے تو ، فراہم کرنے والوں کی کارکردگی (POR ناکامیوں ، دیر سے تکمیل) کی کارکردگی پر تجزیہ پر توجہ دیں۔
   اگر بیک بلاگ مستحکم ناکامیوں کے ساتھ بڑھ رہا ہے تو ، بورڈ کی منظوری کے منتظر ظاہر ہونے کی تصدیق کے لئے انٹیک (`/v2/sorafs/pin/*`) کا معائنہ کریں۔
2. ** فراہم کنندگان کی حیثیت کی توثیق کریں **
   - `iroha app sorafs providers list` چلائیں اور تصدیق کریں کہ اشتہار بازی کی صلاحیتیں نقل کی ضروریات سے ملتی ہیں۔
   - فراہمی شدہ گیبس اور پور کامیابی کی تصدیق کے ل g گیجز `torii_sorafs_capacity_*` چیک کریں۔
3. ** دوبارہ نقل کی اصلاح **
   - `sorafs_manifest_stub capacity replication-order` کے ذریعے نئے آرڈر جاری کریں جب بیکلاگ مارجن (`stat="avg"`) 5 عہدوں سے نیچے گرتا ہے (مینی فیسٹ/کار پیکیجنگ `iroha app sorafs toolkit pack` استعمال کرتا ہے)۔
   - گورننس کو مطلع کریں اگر عرفی ناموں کے پاس فعال مینی فیسٹ پابند نہیں ہے (`torii_sorafs_registry_aliases_total` کا غیر متوقع قطرہ)۔
4. ** نتیجہ کو دستاویز کریں **
   - آپریشنز لاگ SoraFS میں ٹائم اسٹیمپ اور متعلقہ ہضموں کے ساتھ متعلقہ نوٹوں کو ریکارڈ کریں۔
   - اس رن بک کو اپ ڈیٹ کریں اگر نئے ناکامی کے طریقوں یا ڈیش بورڈز متعارف کروائے جائیں۔

## تعیناتی کا منصوبہ

جب پیداوار میں عرف کیشے کی پالیسی کو چالو یا سخت کرتے ہو تو اس مرحلہ وار طریقہ کار پر عمل کریں:1. ** ترتیب تیار کریں **
   - `iroha_config` (صارف -> اصل) میں `torii.sorafs_alias_cache` کو اپ ڈیٹ کریں TTLS اور گریس ونڈوز کے ساتھ: Prometheus ، Grafana ، `hard_expiry` ، `negative_ttl` ، `negative_ttl` ، I18000084X ، I18000084X ، I18000084X ، I18000084X ، I1800000084X ، `negative_ttl` ، `negative_ttl` `successor_grace` اور `governance_grace`۔ پہلے سے طے شدہ اقدار `docs/source/sorafs_alias_policy.md` کی پالیسی کے مطابق ہیں۔
   - ایس ڈی کے کے لئے ، ان کی تشکیل پرتوں (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` میں مورچا / نیپی / ازگر بائنڈنگز) کے ذریعہ وہی اقدار نشر کریں تاکہ کلائنٹ کی درخواست گیٹ وے کی پیروی کرے۔
2. ** اسٹیجنگ میں خشک رن **
   - تشکیل کی تبدیلی کو ایک اسٹیجنگ کلسٹر میں تعینات کریں جو پروڈکشن ٹوپولوجی کی عکاسی کرتا ہے۔
   - `cargo xtask sorafs-pin-fixtures` چلائیں اس بات کی تصدیق کرنے کے لئے کہ کیننیکل عرف فکسچر ہمیشہ ڈیکوڈ اور راؤنڈ ٹرپ ؛ کسی بھی تضاد میں پہلے درست ہونے کے لئے ایک upstream بڑھے شامل ہوتا ہے۔
   -ورزش کے اختتامی مقامات `/v2/sorafs/pin/{digest}` اور `/v2/sorafs/aliases` مصنوعی ثبوتوں کے ساتھ تازہ ، ریفریش ونڈو ، میعاد ختم ہونے اور مشکل سے متاثرہ۔ اس رن بک کے خلاف HTTP کوڈز ، ہیڈرز (`Sora-Proof-Status` ، `Retry-After` ، `Warning`) اور JSON باڈی فیلڈز کی توثیق کریں۔
3. ** پیداوار میں چالو کریں **
   - معیاری تبدیلی ونڈو کے دوران نئی ترتیب کو تعینات کریں۔ پہلے اس کا اطلاق Torii پر کریں ، پھر نوڈ میں نئی ​​پالیسی کی تصدیق کرنے کے بعد گیٹ ویز/ایس ڈی کے خدمات کو دوبارہ شروع کریں۔
   - `docs/source/grafana_sorafs_pin_registry.json` کو Grafana (یا موجودہ ڈیش بورڈز کو اپ ڈیٹ کریں) میں درآمد کریں اور NOC ورک اسپیس پر عرف کیشے ریفریش پینلز کو پن کریں۔
4. ** تعیناتی کی تصدیق **
   - 30 منٹ کے لئے `torii_sorafs_alias_cache_refresh_total` اور `torii_sorafs_alias_cache_age_seconds` کی نگرانی کریں۔ `error`/`expired` منحنی خطوط میں چوٹیوں کو ریفریش ونڈوز کے ساتھ منسلک کرنا ہوگا۔ غیر متوقع نمو کا مطلب ہے کہ آپریٹرز کو آگے بڑھنے سے پہلے عرف شواہد اور فراہم کنندہ کی صحت کا معائنہ کرنا ہوگا۔
   - اس بات کی تصدیق کریں کہ کلائنٹ سائیڈ لاگز وہی پالیسی فیصلے دکھاتے ہیں (جب ثبوت باسی یا میعاد ختم ہونے پر SDKs کی غلطیوں کی اطلاع دی جاتی ہے)۔ کلائنٹ سائیڈ وارننگ کی عدم موجودگی غلط کنفیگریشن کی نشاندہی کرتی ہے۔
5. ** فال بیک **
   - اگر عرف کے اجراء میں تاخیر ہوتی ہے اور ریفریش ونڈو کو کثرت سے متحرک کیا جاتا ہے تو ، تشکیل میں `refresh_window` اور `positive_ttl` میں اضافہ کرکے عارضی طور پر پالیسی کو آرام کریں ، پھر دوبارہ تعی .ن کریں۔ `hard_expiry` برقرار رکھیں تاکہ واقعی پرانی ثبوت ہمیشہ مسترد ہوجائیں۔
   - پچھلی `iroha_config` اسنیپ شاٹ کو بحال کرکے پچھلی ترتیب میں واپس آجائیں اگر ٹیلی میٹری میں اعلی `error` گنتی دکھائی دیتی ہے تو ، پھر عرف نسل کے اوقات کا سراغ لگانے کے لئے ایک واقعہ کھولیں۔

## متعلقہ مواد

- `docs/source/sorafs/pin_registry_plan.md` - عمل درآمد روڈ میپ اور گورننس سیاق و سباق۔
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` - اسٹوریج ورکر آپریشنز ، اس رجسٹری پلے بک کو مکمل کرتا ہے۔