---
lang: ur
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-ops.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: پن رجسٹری-او پی ایس
عنوان: پن رجسٹری آپریشنز
سائڈبار_لیبل: پن رجسٹری آپریشنز
تفصیل: مانیٹر اور ٹریج SoraFS پن رجسٹری اور نقل SLA میٹرکس۔
---

::: نوٹ کینونیکل ماخذ
آئینہ `docs/source/sorafs/runbooks/pin_registry_ops.md`۔ جب تک میراثی اسفنکس دستاویزات ریٹائر نہیں ہوجاتے ہیں تب تک دونوں ورژن کو ہم آہنگ رکھیں۔
:::

## جائزہ

یہ رن بک دستاویز کرتی ہے کہ کس طرح SoraFS پن رجسٹری اور اس کی نقل کی خدمت کی سطح کے معاہدوں (SLA) کی نگرانی اور اس کی ترجمانی کی جائے۔ میٹرکس `iroha_torii` سے شروع ہوتا ہے اور `torii_sorafs_*` نام کی جگہ کے تحت Prometheus کے ذریعے برآمد کیا جاتا ہے۔ Torii پس منظر میں 30 سیکنڈ کے وقفے پر رجسٹری کی حالت کا نمونہ پیش کرتا ہے ، لہذا ڈیش بورڈز اپ ڈیٹ رہتے ہیں یہاں تک کہ جب کوئی آپریٹر `/v1/sorafs/pin/*` اختتامی نقطہ نظر سے استفسار نہیں کررہا ہے۔ کیوریٹڈ ڈیش بورڈ (`docs/source/grafana_sorafs_pin_registry.json`) کو استعمال کرنے کے لئے تیار Grafana لے آؤٹ میں درآمد کریں جو نیچے والے حصوں میں براہ راست نقشہ بناتا ہے۔

## میٹرکس حوالہ

| میٹرک | لیبل | تفصیل |
| ------- | ------ | --------- |
| `torii_sorafs_registry_manifests_total` | `status` (`pending` \ | `approved` \ | `retired`) | لائف سائیکل اسٹیٹ کے ذریعہ آن چین کی انوینٹری۔ |
| `torii_sorafs_registry_aliases_total` | - | رجسٹری میں رجسٹرڈ فعال منشور عرفی ناموں کی گنتی۔ |
| `torii_sorafs_registry_orders_total` | `status` (`pending` \ | `completed` \ | `expired`) | حیثیت کے ذریعہ منقسم نقل کے احکامات کا بیکلاگ۔ |
| `torii_sorafs_replication_backlog_total` | - | سہولت گیج جو `pending` آرڈرز کی آئینہ دار ہے۔ |
| `torii_sorafs_replication_sla_total` | `outcome` (`met` \ | `missed` \ | `pending`) | SLA اکاؤنٹنگ: `met` ڈیڈ لائن کے اندر مکمل ہونے والے احکامات ، `missed` دیر سے تکمیل + میعاد ختم ہونے ، `pending` آئینے زیر التواء آرڈرز۔ |
| `torii_sorafs_replication_completion_latency_epochs` | `stat` (`avg` \ | `p95` \ | `max` \ | `count`) | مجموعی تکمیل میں تاخیر (اجراء اور تکمیل کے درمیان عہد)۔ |
| `torii_sorafs_replication_deadline_slack_epochs` | `stat` (`avg` \ | `p95` \ | `max` \ | `count`) | زیر التواء آرڈرز کے لئے کلیئرنس ونڈوز (ڈیڈ لائن مائنس جاری کرنے کا وقت)۔ |

تمام گیجز ہر اسنیپ شاٹ کے ساتھ دوبارہ سیٹ ہوجاتے ہیں ، لہذا ڈیش بورڈز کو `1m` یا تیز تر کیڈینس پر نمونہ لینا چاہئے۔

## Grafana ڈیش بورڈ

ڈیش بورڈ JSON میں سات پینل شامل ہیں جو آپریٹر کے ورک فلوز کا احاطہ کرتے ہیں۔ اگر آپ کسٹم گرافکس بنانے کو ترجیح دیتے ہیں تو نیچے دیئے گئے سوالات فوری حوالہ کے طور پر کام کرتے ہیں۔

1.
2. ** عرف کیٹلاگ ٹرینڈ ** - `torii_sorafs_registry_aliases_total`۔
3.
4. ** بیکلاگ بمقابلہ میعاد ختم ہونے والے احکامات ** - سنترپتی کو ظاہر کرنے کے لئے `torii_sorafs_replication_backlog_total` اور `torii_sorafs_registry_orders_total{status="expired"}` کو جوڑتا ہے۔
5. ** ایس ایل اے کامیابی کا تناسب ** -

   ```promql
   sum(torii_sorafs_replication_sla_total{outcome="met"})
   /
   clamp_min(
     sum(torii_sorafs_replication_sla_total{outcome=~"met|missed"}),
     1
   )
   ```6. جب آپ کو مطلق منزل کی کلیئرنس کی ضرورت ہو تو `min_over_time` خیالات کو شامل کرنے کے لئے Grafana تبدیلیوں کا استعمال کریں ، مثال کے طور پر:

   ```promql
   min_over_time(torii_sorafs_replication_deadline_slack_epochs{stat="avg"}[15m])
   ```

7. ** کھوئے ہوئے احکامات (1H فیس) ​​** -

   ```promql
   sum(increase(torii_sorafs_replication_sla_total{outcome="missed"}[1h]))
   ```

## انتباہ دہلیز

- ** ایس ایل اے کامیابی  0 **
  - دہلیز: `increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0`
  - ایکشن: گورننس کا معائنہ کریں تاکہ فراہم کنندہ کے منشور کی تصدیق کی جاسکے۔
- ** P95 تکمیل> اوسط ڈیڈ لائن سلیک **
  - دہلیز: `torii_sorafs_replication_completion_latency_epochs{stat="p95"} > torii_sorafs_replication_deadline_slack_epochs{stat="avg"}`
  - ایکشن: چیک کریں کہ آیا فراہم کنندہ ڈیڈ لائن سے پہلے تعمیل کر رہے ہیں۔ دوبارہ تفویض پر غور کریں۔

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
          summary: "SLA de replicacao SoraFS abaixo da meta"
          description: "A razao de sucesso do SLA ficou abaixo de 95% por 15 minutos."

      - alert: SorafsReplicationBacklogGrowing
        expr: torii_sorafs_replication_backlog_total > 10
        for: 10m
        labels:
          severity: page
        annotations:
          summary: "Backlog de replicacao SoraFS acima do limiar"
          description: "Ordens de replicacao pendentes excederam o budget configurado."

      - alert: SorafsReplicationExpiredOrders
        expr: increase(torii_sorafs_registry_orders_total{status="expired"}[5m]) > 0
        for: 0m
        labels:
          severity: ticket
        annotations:
          summary: "Ordens de replicacao Prometheus expiradas"
          description: "Pelo menos uma ordem de replicacao expirou nos ultimos cinco minutos."
```

## اسکریننگ فلو

1. ** وجہ کی شناخت کریں **
   - اگر ایس ایل اے مشنوں میں اضافہ ہوتا ہے جبکہ بیک بلاگ کم رہتا ہے تو ، فراہم کنندہ کی کارکردگی (POR ناکامیوں ، دیر سے تکمیلات) پر توجہ دیں۔
   اگر بیک بلاگ مستحکم مشنوں کے ساتھ بڑھتا ہے تو ، بورڈ کی منظوری کے منتظر ظاہر ہونے کی تصدیق کے لئے انٹیک (`/v1/sorafs/pin/*`) کا معائنہ کریں۔
2. ** فراہم کنندہ کی حیثیت کو درست کریں **
   - `iroha app sorafs providers list` چلائیں اور تصدیق کریں کہ اشتہاری صلاحیتیں نقل کی ضروریات کو پورا کرتی ہیں۔
   - فراہمی شدہ GIB اور POR کامیابی کی تصدیق کے لئے `torii_sorafs_capacity_*` گیجز چیک کریں۔
3. ** دوبارہ نقل کی اصلاح **
   - `sorafs_manifest_stub capacity replication-order` کے ذریعے نئے آرڈر جاری کریں جب بیکلاگ سلیک (`stat="avg"`) 5 عہدوں سے نیچے گرتا ہے (مینی فیسٹ/کار پیکیجنگ `iroha app sorafs toolkit pack` استعمال کرتا ہے)۔
   - گورننس کو مطلع کریں اگر عرفی ناموں کے پاس فعال مینی فیسٹ پابندی نہیں ہے (`torii_sorafs_registry_aliases_total` میں غیر متوقع قطرے)۔
4. ** دستاویز کا نتیجہ **
   - SoraFS آپریشنز میں ریکارڈ واقعے کے نوٹس متاثرہ ٹائم اسٹیمپ اور ظاہر ہضموں کے ساتھ لاگ ان۔
   - اس رن بک کو اپ ڈیٹ کریں اگر نئے ناکامی کے طریقوں یا ڈیش بورڈز متعارف کروائے جائیں۔

## رول آؤٹ پلان

جب پیداوار میں عرف کیشے کی پالیسی کو چالو یا سخت کرتے ہو تو اس مرحلہ وار طریقہ کار پر عمل کریں:1. ** ترتیب تیار کریں **
   - `torii.sorafs_alias_cache` کو `iroha_config` (صارف -> موجودہ) میں متفقہ TTLS اور گریس ونڈوز کے ساتھ اپ ڈیٹ کریں: `positive_ttl` ، Grafana ، `hard_expiry` ، `negative_ttl` ، `negative_ttl` ، I180084X ، I180084X ، I180084X ، I18000084X `successor_grace` اور `governance_grace`۔ پہلے سے طے شدہ `docs/source/sorafs_alias_policy.md` میں پالیسی کے مطابق ہے۔
   - ایس ڈی کے کے لئے ، اپنی ترتیب پرتوں میں ایک ہی اقدار کو تقسیم کریں (مورچا / نیپی / پیتھون بائنڈنگ میں `AliasCachePolicy::new(positive, refresh, hard, negative, revocation, rotation, successor, governance)`) تاکہ کلائنٹ کا نفاذ گیٹ وے سے مماثل ہو۔
2. ** اسٹیجنگ میں خشک رن **
   - کنفیگریشن چینج کو ایک اسٹیجنگ کلسٹر میں تعینات کریں جو پروڈکشن ٹوپولوجی کی آئینہ دار ہے۔
   - `cargo xtask sorafs-pin-fixtures` چلائیں اس بات کی تصدیق کے لئے کہ کیننیکل عرف فکسچر اب بھی ڈی کوڈ اور راؤنڈ ٹرپ ؛ کسی بھی طرح کی مماثلت کا مطلب اپ اسٹریم ڈرفٹ کا مطلب ہے جسے پہلے حل کرنا چاہئے۔
   -`/v1/sorafs/pin/{digest}` اور `/v1/sorafs/aliases` اختتامی نکات کو مصنوعی ٹیسٹوں کے ساتھ تازہ ، تروتازہ ونڈو ، میعاد ختم ہونے اور سخت مبتلا معاملات کا احاطہ کرتا ہے۔ اس رن بک کے خلاف HTTP کوڈز ، ہیڈرز (`Sora-Proof-Status` ، `Retry-After` ، `Warning`) اور JSON باڈی فیلڈز کی توثیق کریں۔
3. ** پیداوار میں قابل بنائیں **
   - معیاری تبدیلیوں والی ونڈو میں نئی ​​ترتیب کو تعینات کریں۔ پہلے Torii پر درخواست دیں اور پھر جیسے ہی نوڈ میں نوڈ میں نئی ​​پالیسی کی تصدیق ہوتی ہے گیٹ ویز/ایس ڈی کے خدمات کو دوبارہ شروع کریں۔
   - `docs/source/grafana_sorafs_pin_registry.json` کو Grafana (یا موجودہ ڈیش بورڈز کو اپ ڈیٹ کریں) میں درآمد کریں اور NOC ورک اسپیس میں عرف کیشے ریفریش ڈیش بورڈز کو پن کریں۔
4. ** پوسٹ کی تصدیق **
   - 30 منٹ کے لئے `torii_sorafs_alias_cache_refresh_total` اور `torii_sorafs_alias_cache_age_seconds` کی نگرانی کریں۔ `error`/`expired` منحنی خطوط میں ریفریش ونڈوز سے وابستہ ہونا چاہئے۔ غیر متوقع نمو کا مطلب یہ ہے کہ آپریٹرز کو آگے بڑھنے سے پہلے عرف ثبوت اور فراہم کنندگان کی صحت کا معائنہ کرنا ہوگا۔
   - اس بات کی تصدیق کریں کہ کلائنٹ سائیڈ لاگز وہی پالیسی فیصلے دکھاتے ہیں (جب ثبوت باسی یا میعاد ختم ہونے پر SDKs غلطیاں پھینک دیں گے)۔ کلائنٹ کی انتباہات کی عدم موجودگی غلط ترتیب کی نشاندہی کرتی ہے۔
5. ** فال بیک **
   - اگر عرفیہ کے اجراء میں تاخیر ہوتی ہے اور ریفریش ونڈو کثرت سے متحرک ہوجاتی ہے تو ، تشکیل میں `refresh_window` اور `positive_ttl` میں اضافہ کرکے عارضی طور پر پالیسی کو آرام کریں ، پھر دوبارہ تعی .ن کریں۔ `hard_expiry` برقرار رکھیں تاکہ واقعی باسی ثبوت کو ابھی بھی مسترد کردیا جائے۔
   - Grafana کے پچھلے اسنیپ شاٹ کو بحال کرکے پچھلی ترتیب پر واپس جائیں اگر ٹیلی میٹری میں بلند `error` گنتی کو ظاہر کرنا جاری ہے تو ، پھر عرف جنریشن میں تاخیر کو ٹریک کرنے کے لئے ایک واقعہ کھولیں۔

## متعلقہ مواد

- `docs/source/sorafs/pin_registry_plan.md` - عمل درآمد روڈ میپ اور گورننس سیاق و سباق۔
- `docs/source/sorafs/runbooks/sorafs_node_ops.md` - اسٹوریج ورکر آپریشنز ، اس رجسٹری پلے بک کو پورا کرتا ہے۔