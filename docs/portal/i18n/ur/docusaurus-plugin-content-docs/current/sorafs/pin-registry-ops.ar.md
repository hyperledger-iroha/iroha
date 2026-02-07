---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-ops.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: پن رجسٹری-او پی ایس
عنوان: پن رجسٹری آپریشنز
سائڈبار_لیبل: پن رجسٹری آپریشنز
تفصیل: SoraFS اور فالتو پن SLA میٹرکس میں پن کی رجسٹری کی نگرانی اور ترتیب دیں۔
---

::: منظور شدہ ماخذ کو نوٹ کریں
`docs/source/sorafs/runbooks/pin_registry_ops.md` کی عکاسی کریں۔ پرانی اسپنکس دستاویزات ریٹائر ہونے تک دونوں ورژن کو مطابقت پذیری میں رکھیں۔
:::

## جائزہ

یہ گائیڈ دستاویز کرتا ہے کہ کس طرح SoraFS اور نقل کی SLAs میں پن رجسٹری کی نگرانی اور ترتیب دیں۔ میٹرکس `iroha_torii` سے ہوتا ہے اور Prometheus کے ذریعے نام کی جگہ `torii_sorafs_*` کے تحت برآمد کیا جاتا ہے۔ Torii پس منظر میں ہر 30 سیکنڈ میں رجسٹری اسٹیٹ کے نمونے لگاتے ہیں ، لہذا ڈیش بورڈز تازہ ترین رہتے ہیں یہاں تک کہ جب آپریٹرز `/v1/sorafs/pin/*` اختتامی مقامات پر کال نہیں کررہے ہیں۔ استعمال شدہ کنٹرول پینل (`docs/source/grafana_sorafs_pin_registry.json`) کو استعمال کرنے کے لئے تیار Grafana لے آؤٹ حاصل کرنے کے لئے درآمد کریں جو نیچے والے حصوں سے مماثل ہے۔

## میٹرکس حوالہ

| اسکیل | لیبل | تفصیل |
| ------ | ------ | ----- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \ | `approved` \ | `retired`) | آن چین انوینٹری لائف سائیکل اسٹیٹ پر مبنی ظاہر ہوتی ہے۔ |
| `torii_sorafs_registry_aliases_total` | - | رجسٹری میں رجسٹرڈ منشور کے لئے فعال عرفی نام کی تعداد۔ |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \ | `completed` \ | `expired`) | اعادہ کے احکامات کا بیک بلاگ حیثیت سے ٹوٹ گیا۔ |
| `torii_sorafs_replication_backlog_total` | - | حوالہ گیج کمانڈز `pending` کی عکاسی کرتا ہے۔ |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \ | `missed` \ | `pending`) | SLA اکاؤنٹنگ: `met` لیڈ ٹائم کے اندر مکمل ہونے والے احکامات ، `missed` دیر سے تکمیل + تکمیل ، `pending` زیر التواء احکامات کی عکاسی کرتا ہے۔ |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \ | `p95` \ | `max` \ | `count`) | مجموعی تکمیل میں تاخیر (رہائی اور تکمیل کے درمیان عہدوں کی تعداد)۔ |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \ | `p95` \ | `max` \ | `count`) | زیر التواء آرڈرز کے لئے مارجن ونڈوز (لیڈ ٹائم مائنس آئی پی او کی رہائی)۔ |

تمام گیجز ہر اسنیپ شاٹ کو دوبارہ ترتیب دیتے ہیں ، لہذا ڈیش بورڈز `1m` پر ہونا چاہئے یا تیز۔

## بورڈ Grafana

JSON ڈیش بورڈ میں آپریٹرز کے ورک فلوز کو ڈھانپنے والے سات اسکیموں پر مشتمل ہے۔ اگر آپ کسٹم چارٹ بنانے کو ترجیح دیتے ہیں تو فوری حوالہ کے لئے سوالات ذیل میں درج ہیں۔

1.
2. ** عرف کیٹلاگ سمت ** - `torii_sorafs_registry_aliases_total`۔
3.
4. ** بیکلاگ بمقابلہ تیار شدہ احکامات ** - سنترپتی کو ظاہر کرنے کے لئے `torii_sorafs_replication_backlog_total` اور `torii_sorafs_registry_orders_total{status="expired"}` کی رقم۔
5. ** ایس ایل اے کامیابی کی شرح ** -

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```

6. جب آپ کو مطلق کم سے کم مارجن کی ضرورت ہو تو `min_over_time` کی پیش کشوں کو شامل کرنے کے لئے Grafana تبادلوں کا استعمال کریں ، مثال کے طور پر:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. ** کھوئے ہوئے احکامات (1h کی شرح) ** -

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## الارم کی دہلیز- ** کامیاب SLA  0 **
  - دہلیز: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - ایکشن: فراہم کنندگان میں منشور کی تصدیق کے لئے گورننس ظاہر ہوتا ہے۔
- ** P95 مکمل ہونے کے لئے> اوسط لیڈ ٹائم مارجن **
  - دہلیز: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - طریقہ کار: تصدیق کریں کہ فراہم کنندگان آخری تاریخ سے پہلے ہی ارتکاب کرتے ہیں۔ دوبارہ تقویت پر غور کریں۔

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
          summary: "هبوط SLA لتكرار SoraFS تحت الهدف"
          description: "ظلت نسبة نجاح SLA اقل من 95% لمدة 15 دقيقة."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "تراكم تكرار SoraFS فوق العتبة"
          description: "تجاوزت اوامر التكرار المعلقة ميزانية التراكم المضبوطة."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "انتهاء اوامر تكرار Prometheus"
          description: "انتهى امر تكرار واحد على الاقل في الخمس دقائق الماضية."
```

## ورک فلو کو چھانٹ رہا ہے

1. ** وجہ کی شناخت کریں **
   - اگر ایس ایل اے کی ناکامیوں میں اضافہ ہوتا ہے جبکہ بیکلاگ کم رہتا ہے تو ، فراہم کنندگان کی کارکردگی (پور کی ناکامی ، تاخیر سے تکمیل) پر توجہ دیں۔
   - اگر بیک بلاگ مستحکم ناکامیوں کے ساتھ بڑھتا ہے تو ، بورڈ کی منظوری کے منتظر ظاہر ہونے کی تصدیق کے لئے قبولیت (`/v1/sorafs/pin/*`) چیک کریں۔
2. ** فراہم کنندگان کی حیثیت کو چیک کریں **
   - `iroha app sorafs providers list` چلائیں اور اس بات کو یقینی بنائیں کہ اعلان کردہ صلاحیتیں فالتو پن کی ضروریات سے ملتی ہیں۔
   - لیس GIB اور POR کامیابی کی تصدیق کے ل g گیجز `torii_sorafs_capacity_*` چیک کریں۔
3. ** دوبارہ تکرار کی بحالی **
   - `sorafs_manifest_stub capacity replication-order` کے ذریعے نئے آرڈر جاری کریں جب جمع مارجن (`stat="avg"`) 5 ای بکس سے نیچے آجاتا ہے (مینی فیسٹ/کار پیکیجنگ `iroha app sorafs toolkit pack` استعمال کرتا ہے)۔
   - گورننس کا خطرہ اگر عرفیت میں فعال مینی فیسٹ بائنڈنگ کی کمی ہے (`torii_sorafs_registry_aliases_total` میں غیر متوقع ڈراپ)۔
4. ** نتیجہ کی دستاویزات **
   - SoraFS عمل میں ٹائم اسٹیمپ اور ہضموں کے ساتھ ریکارڈ واقعے کے نوٹ۔
   - اس گائیڈ کو اپ ڈیٹ کریں جب نئے ناکامی کے طریقوں یا نئے بورڈز نمودار ہوں۔

## لانچ پلان

جب پیداوار میں عرف کے لئے کیشے کی پالیسی کو چالو کرنے یا سخت کرتے ہو تو اس مرحلہ وار طریقہ کار پر عمل کریں:1. ** ترتیبات کی تیاری **
   - `iroha_config` (صارف -> اصل) میں `torii.sorafs_alias_cache` کو اپ ڈیٹ کریں TTLS اور اجازت ونڈوز کا استعمال کرتے ہوئے: `positive_ttl` ، Grafana ، `hard_expiry` ، `negative_ttl` ، `negative_ttl` ، I18000084X ، I180084X ، I18000084X ، I18000084X ، I1800000084X ، `negative_ttl` `successor_grace` ، `governance_grace`۔ پہلے سے طے شدہ اقدار `docs/source/sorafs_alias_policy.md` پر پالیسی سے میچ کرتی ہیں۔
   - ایس ڈی کے کے ل the ، گنتی کی تہوں (`AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)` میں مورچا/نیپی/ازگر بائنڈنگز) میں اسی قدروں کو تقسیم کریں جب تک کہ کلائنٹ کا نفاذ گیٹ وے سے مماثل نہ ہو۔
2. ** اسٹیجنگ میں خشک رن **
   - ترتیبات کو ایک اسٹیجنگ کلسٹر پر تبدیل کریں جو پروڈکشن ٹوپولوجی کی عکاسی کرتا ہے۔
   - `cargo xtask sorafs-pin-fixtures` کو اس بات کی تصدیق کرنے کے لئے چلائیں کہ کیننیکل عرف فکسچر ابھی بھی ڈکرپٹنگ کر رہے ہیں اور راؤنڈ ٹرپ کر رہے ہیں۔ کسی بھی طرح کی مماثلت ، جس کا مطلب ماخذ میں بڑھے ہوئے ہیں ، کو پہلے حل کرنا چاہئے۔
   -ٹیسٹ کے اختتامی مقامات `/v1/sorafs/pin/{digest}` اور `/v1/sorafs/aliases` ایک مصنوعی سابقہ ​​کے ساتھ تازہ ، ریفریش ونڈو ، میعاد ختم ہونے والی ، اور سخت مجبوری ریاستوں کا احاطہ کرتا ہے۔ اس ڈائرکٹری کے خلاف HTTP ہیڈرز (`Sora-Proof-Status` ، `Retry-After` ، `Warning`) اور JSON باڈی فیلڈز چیک کریں۔
3. ** پیداوار میں ایکٹیویشن **
   - معیاری تبدیلی ونڈو میں نئی ​​ترتیبات پوسٹ کریں۔ پہلے اس کا اطلاق Torii پر پہلے کریں ، پھر گیٹ ویز/سروسز SDK کو دوبارہ اسٹارٹ کریں جب نوڈ میں نوڈ میں نئی ​​پالیسی کی تصدیق ہوجاتی ہے۔
   - `docs/source/grafana_sorafs_pin_registry.json` کو Grafana (یا موجودہ بورڈز کو اپ ڈیٹ کریں) میں درآمد کریں اور NOC ورک اسپیس میں عرف کے لئے کیش اپ ڈیٹ بورڈ انسٹال کریں۔
4. ** اشاعت کے بعد توثیق **
   - 30 منٹ کے لئے `torii_sorafs_alias_cache_refresh_total` اور `torii_sorafs_alias_cache_age_seconds` کی نگرانی کریں۔ `error`/`expired` منحنی خطوط میں چوٹیوں کو اپ ڈیٹ ونڈوز کے ساتھ وابستہ ہونا چاہئے۔ غیر متوقع نمو کا مطلب یہ ہے کہ آپریٹرز کو آگے بڑھنے سے پہلے عرف ڈائریکٹریوں اور فراہم کنندگان کی صحت کی جانچ کرنی چاہئے۔
   - اس بات کو یقینی بنائیں کہ کلائنٹ کے نوشتہ ایک ہی پالیسی فیصلے ظاہر کرتے ہیں (جب ڈائریکٹری باسی یا میعاد ختم ہوجاتی ہے تو SDKs غلطیاں دکھائیں گے)۔ کلائنٹ کی انتباہات کی عدم موجودگی سیٹ اپ کی غلطی کی نشاندہی کرتی ہے۔
5. ** فال بیک **
   - اگر عرف کی رہائی میں تاخیر ہوتی ہے اور اپ ڈیٹ ونڈو بار بار حد سے تجاوز کر جاتی ہے تو ، `refresh_window` اور `positive_ttl` کو نمبر میں بڑھا کر عارضی طور پر پالیسی کو آرام کریں اور پھر دوبارہ تعینات کریں۔ `hard_expiry` کو اسی طرح رکھیں تاکہ پرانے شواہد کو حقیقت میں ابھی بھی مسترد کردیا جائے۔
   - `iroha_config` کے پچھلے سنیپ شاٹ کو بحال کرکے پچھلی ترتیب میں واپس آجائیں۔ اگر ٹیلی میٹری میں اعلی `error` کی ترتیبات دکھائی دیتی ہیں تو ، عرف جنریشن کی تاخیر کا سراغ لگانے کے لئے ایک واقعہ کھولیں۔

## متعلقہ مواد

- `docs/source/sorafs/pin_registry_plan.md` - عمل درآمد روڈ میپ اور گورننس سیاق و سباق۔
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` - اسٹوریج ایجنٹ کی کارروائی ، اس گائیڈ کو پورا کرتا ہے۔