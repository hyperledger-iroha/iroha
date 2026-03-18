---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-ops.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: پن رجسٹری-او پی ایس
عنوان: پن رجسٹری آپریشنز
سائڈبار_لیبل: پن رجسٹری آپریشنز
تفصیل: نگرانی اور ٹریج پن رجسٹری SoraFS اور SLA نقل کی پیمائش۔
---

::: نوٹ کینونیکل ماخذ
`docs/source/sorafs/runbooks/pin_registry_ops.md` کی عکاسی کرتا ہے۔ جب تک میراثی اسفنکس دستاویزات ریٹائر نہ ہو تب تک دونوں ورژن کو مطابقت پذیری میں رکھیں۔
:::

## جائزہ

اس رن بک میں بتایا گیا ہے کہ کس طرح ٹریج پن رجسٹری SoraFS اور نقل کے لئے اس کی خدمت کی سطح کے معاہدے (SLA) کی نگرانی اور ان پر عمل درآمد کیا جائے۔ میٹرکس `iroha_torii` سے آتے ہیں اور Prometheus کے ذریعے نام کی جگہ `torii_sorafs_*` کے تحت برآمد ہوتے ہیں۔ Torii پس منظر میں ہر 30 سیکنڈ میں رجسٹری کی حیثیت کا انتخاب کرتا ہے ، لہذا ڈیش بورڈز تازہ ترین رہیں یہاں تک کہ جب کوئی آپریٹر `/v1/sorafs/pin/*` اختتامی نقطہ کی درخواست نہیں کرتا ہے۔ تیار کردہ ترتیب Grafana کے لئے تیار ڈیش بورڈ (`docs/source/grafana_sorafs_pin_registry.json`) درآمد کریں ، جو نیچے والے حصوں سے براہ راست مطابقت رکھتا ہے۔

## میٹرکس حوالہ

| میٹرک | لیبل | تفصیل |
| ------ | ------ | -------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \ | `approved` \ | `retired`) | لائف سائیکل ریاستوں کے ذریعہ انچین پر انوینٹری۔ |
| `torii_sorafs_registry_aliases_total` | - | رجسٹری میں ریکارڈ شدہ فعال عرف کی تعداد ظاہر ہوتی ہے۔ |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \ | `completed` \ | `expired`) | نقل کے احکامات کا بیک بلاگ ، حیثیت کے ذریعہ منقسم۔ |
| `torii_sorafs_replication_backlog_total` | - | آسان گیج `pending` آرڈرز کی عکاسی کرتا ہے۔ |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \ | `missed` \ | `pending`) | SLA اکاؤنٹنگ: `met` وقت پر مکمل ہونے والے آرڈرز ، `missed` دیر سے تکمیلات + میعاد ختم ہونے کی میعاد ، `pending` نامکمل احکامات کی عکاسی کرتا ہے۔ |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \ | `p95` \ | `max` \ | `count`) | مجموعی تکمیل میں تاخیر (رہائی اور تکمیل کے درمیان عہد)۔ |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \ | `p95` \ | `max` \ | `count`) | نامکمل احکامات کے لئے انوینٹری ونڈوز (ڈیڈ لائن مائنس ایشو ایرا)۔ |

تمام گیجز ہر پل اسنیپ شاٹ کے ساتھ دوبارہ ترتیب دیئے جاتے ہیں ، لہذا ڈیش بورڈز کو `1m` یا تیز تر تعدد پر پول کیا جانا چاہئے۔

## ڈیش بورڈ Grafana

JSON ڈیش بورڈ میں آپریٹر کے ورک فلوز کو ڈھانپنے والے سات پینل شامل ہیں۔ اگر آپ اپنے گراف بنانے کو ترجیح دیتے ہیں تو فوری حوالہ کے لئے سوالات ذیل میں درج ہیں۔

1.
2. ** کیٹلاگ عرف رجحان ** - `torii_sorafs_registry_aliases_total`۔
3. ** حیثیت کے ذریعہ احکامات کی قطار ** - `torii_sorafs_registry_orders_total` (گروپنگ برائے `status`)۔
4. ** بیکلاگ بمقابلہ میعاد ختم ہونے والے احکامات ** - سنترپتی کا پتہ لگانے کے لئے `torii_sorafs_replication_backlog_total` اور `torii_sorafs_registry_orders_total{status="expired"}` کو جوڑتا ہے۔
5. ** ایس ایل اے کامیابی کی شرح ** -

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```6. جب آپ کو مطلق نچلے مارجن کی ضرورت ہو تو `min_over_time` نمائندگی شامل کرنے کے لئے Grafana تبدیلیوں کا استعمال کریں ، مثال کے طور پر:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. ** کھوئے ہوئے احکامات (شرح 1H) ** -

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## انتباہ دہلیز

- ** کامیابی SLA  0 **
  - دہلیز: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - ایکشن: چیک گورننس ظاہر ہوتا ہے کہ منشور فراہم کرنے والوں کی تصدیق کریں۔
- ** P95 تکمیل> ڈیڈ لائن پر اوسط اسٹاک **
  - دہلیز: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - ایکشن: اس بات کو یقینی بنائیں کہ فراہم کنندگان آخری تاریخ سے پہلے مکمل کریں۔ دوبارہ تقسیم پر غور کریں۔

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
          summary: "SLA репликации SoraFS ниже целевого"
          description: "Коэффициент успеха SLA оставался ниже 95% в течение 15 минут."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "Backlog репликации SoraFS выше порога"
          description: "Ожидающие заказы репликации превысили настроенный бюджет backlog."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "Заказы репликации Prometheus истекли"
          description: "По крайней мере один заказ репликации истек за последние пять минут."
```

## ورک فلو ٹریج

1. ** وجہ کا تعین کریں **
   - اگر ایس ایل اے کی کمی محسوس ہوتی ہے اور بیک بلاگ کم رہتا ہے تو ، فراہم کنندگان کی کارکردگی (PUR ناکامیوں ، دیر سے تکمیل) پر توجہ دیں۔
   اگر بیک بلاگ مستحکم پاسوں کے ساتھ بڑھ رہا ہے تو ، بورڈ کی منظوری کے منتظر ظاہر ہونے کی تصدیق کے لئے داخلہ (`/v1/sorafs/pin/*`) چیک کریں۔
2. ** فراہم کرنے والوں کی حیثیت چیک کریں **
   - `iroha app sorafs providers list` چلائیں اور تصدیق کریں کہ اعلان کردہ صلاحیتیں نقل کی ضروریات کو پورا کرتی ہیں۔
   - فراہمی گیب اور پور کامیابی کی تصدیق کے لئے گیجز `torii_sorafs_capacity_*` چیک کریں۔
3. ** دوبارہ تقسیم کی نقل **
   - `sorafs_manifest_stub capacity replication-order` کے ذریعے نئے آرڈر جاری کریں جب بیکلاگ اسٹاک (`stat="avg"`) 5 عہدوں سے نیچے گرتا ہے (مینی فیسٹ/کار پیکیجنگ `iroha app sorafs toolkit pack` استعمال کرتا ہے)۔
   - گورننس کو مطلع کریں اگر عرفی ناموں کے پاس فعال پابند ظاہر نہیں ہوتا ہے (غیر متوقع حادثہ `torii_sorafs_registry_aliases_total`)۔
4. ** نتیجہ کو دستاویز کریں **
   - ٹائم اسٹیمپ اور ڈائجسٹ مینیفنس کے ساتھ SoraFS آپریشن لاگ پر واقعہ کے نوٹ لکھیں۔
   - اس رن بک کو اپ ڈیٹ کریں اگر نئی ناکامی کے طریقوں یا ڈیش بورڈز ظاہر ہوں۔

## تعیناتی کا منصوبہ

پیداوار میں عرف کیشے کی پالیسی کو چالو کرنے یا بڑھانے کے وقت اس مرحلہ وار عمل پر عمل کریں:1. ** ترتیب تیار کریں **
   - `torii.sorafs_alias_cache` کو `iroha_config` (صارف -> اصل) کے ساتھ مستقل ٹی ٹی ایل اور گریس ونڈوز کے ساتھ اپ ڈیٹ کریں: `positive_ttl` ، `refresh_window` ، `hard_expiry` ، `negative_ttl` ، i18000084x ، i1800000000000000000000000000000000000000000000000000000000000000000000000000 `successor_grace` ، `governance_grace`۔ پہلے سے طے شدہ اقدار `docs/source/sorafs_alias_policy.md` میں پالیسی کے مطابق ہیں۔
   - ایس ڈی کے کے ل their ، ان کی ترتیب پرتوں (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` میں مورچا/نیپی/پیتھون بائنڈنگز) کے ذریعے ایک ہی اقدار کو پروپیگنڈا کریں تاکہ کلائنٹ کا نفاذ گیٹ وے سے مماثل ہو۔
2. ** اسٹیجنگ میں خشک رن **
   - تشکیل کی تبدیلی کو ایک اسٹیجنگ کلسٹر میں تعینات کریں جو پروڈکشن ٹوپولوجی کی عکاسی کرتا ہے۔
   - `cargo xtask sorafs-pin-fixtures` چلائیں اس بات کا یقین کرنے کے لئے کہ کیننیکل عرف فکسچر ابھی بھی ضابطہ کشائی اور گول ٹرپڈ ہیں۔ کسی بھی تضاد کا مطلب ہے upstream بڑھے ، جس کو پہلے ختم کیا جانا چاہئے۔
   -اختتامی مقامات `/v1/sorafs/pin/{digest}` اور `/v1/sorafs/aliases` مصنوعی ثبوتوں کے ساتھ تازہ ، ریفریش ونڈو ، میعاد ختم ہونے اور مشکل سے متاثرہ معاملات کا احاطہ کرتے ہیں۔ اس رن بک کے خلاف HTTP کوڈز ، ہیڈر (`Sora-Proof-Status` ، `Retry-After` ، `Warning`) اور باڈی JSON فیلڈز چیک کریں۔
3. ** پیداوار میں قابل بنائیں **
   - نئی ترتیب کو معیاری تبدیلیوں والی ونڈو میں بڑھا دیں۔ پہلے Torii پر درخواست دیں ، پھر نوڈ لاگز میں نئی ​​پالیسی کی تصدیق کے بعد گیٹ ویز/ایس ڈی کے خدمات کو دوبارہ شروع کریں۔
   - `docs/source/grafana_sorafs_pin_registry.json` کو Grafana (یا موجودہ ڈیش بورڈز کو اپ ڈیٹ کریں) میں درآمد کریں اور NOC ورک اسپیس میں ریفریش عرف کیشے پینلز کو پن کریں۔
4. ** تعیناتی کے بعد چیک کریں **
   - 30 منٹ کے لئے `torii_sorafs_alias_cache_refresh_total` اور `torii_sorafs_alias_cache_age_seconds` کی نگرانی کریں۔ `error`/`expired` منحنی خطوط میں چوٹیوں کو ریفریش ونڈوز کے ساتھ منسلک کرنا چاہئے۔ غیر متوقع نمو کا مطلب ہے کہ آپریٹرز کو آگے بڑھنے سے پہلے پروف عرفی اور فراہم کنندگان کی حیثیت کی جانچ کرنی چاہئے۔
   - اس بات کو یقینی بنائیں کہ کلائنٹ کے نوشتہ جات وہی پالیسی فیصلے دکھاتے ہیں (جب ثبوت تاریخ سے باہر ہو یا میعاد ختم ہوجائے تو ایس ڈی کے غلطیاں پھینک دیں گے)۔ کوئی کلائنٹ کی انتباہ غلط ترتیب کی نشاندہی نہیں کرتا ہے۔
5. ** فال بیک **
   - اگر عرف رہتا ہے اور ریفریش ونڈو میں کثرت سے فائر ہوتا ہے تو ، تشکیل میں `refresh_window` اور `positive_ttl` میں اضافہ کرکے عارضی طور پر پالیسی کو آرام کریں ، پھر دوبارہ تعی .ن کریں۔ `hard_expiry` کو کوئی تبدیلی نہیں چھوڑیں تاکہ واقعی فرسودہ ثبوتوں کو مسترد کردیا جائے۔
   - پچھلی اسنیپ شاٹ `iroha_config` کو بحال کرکے پچھلی ترتیب کی طرف لوٹائیں اگر ٹیلی میٹری بلند `error` اقدار کو ظاہر کرتی رہتی ہے تو ، پھر عرف جنریشن میں تاخیر کی تحقیقات کے لئے ایک واقعہ کھولیں۔

## متعلقہ مواد

- `docs/source/sorafs/pin_registry_plan.md` - عمل درآمد روڈ میپ اور مینجمنٹ سیاق و سباق۔
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` - اسٹوریج ورکر آپریشنز ، اس پلے بک رجسٹری کو پورا کرتا ہے۔