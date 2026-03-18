---
id: constant-rate-profiles
lang: ar
direction: rtl
source: docs/portal/docs/soranet/constant-rate-profiles.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note المصدر القياسي
تعكس هذه الصفحة `docs/source/soranet/constant_rate_profiles.md`. حافظ على النسختين متطابقتين حتى يتم تقاعد مجموعة الوثائق القديمة.
:::

يقدم SNNet-17B مسارات نقل ذات معدل ثابت بحيث تنقل المرحلات حركة المرور في خلايا 1,024 B بغض النظر عن حجم payload. يختار المشغلون من ثلاثة presets:

- **core** - مرحلات مراكز البيانات او الاستضافة الاحترافية التي يمكنها تخصيص >=30 Mbps لتغطية الحركة.
- **home** - مشغلو السكن او uplink المنخفض الذين لا يزالون يحتاجون fetches مجهولة للدوائر الحساسة للخصوصية.
- **null** - preset dogfood لـ SNNet-17A2. يحتفظ بنفس TLVs/envelope لكنه يطيل tick وceiling لبيئات staging ذات النطاق المنخفض.

## ملخص presets

| الملف | Tick (ms) | الخلية (B) | سقف lanes | ارضية dummy | Payload لكل lane (Mb/s) | Payload سقف (Mb/s) | % من uplink للسقف | Uplink موصى به (Mb/s) | سقف الجيران | محفز التعطيل التلقائي (%) |
|---------|-----------|----------|----------|-------------|-------------------------|------------------------|---------------------|----------------------------|--------------|--------------------------|
| core    | 5.0       | 1024     | 12       | 4           | 1.64                    | 19.50                  | 65                  | 30.0                       | 8            | 85                       |
| home    | 10.0      | 1024     | 4        | 2           | 0.82                    | 4.00                   | 40                  | 10.0                       | 2            | 70                       |
| null    | 20.0      | 1024     | 2        | 1           | 0.41                    | 0.75                   | 15                  | 5.0                        | 1            | 55                       |

- **سقف lanes** - الحد الاقصى لعدد الجيران المتزامنين بمعدل ثابت. ترفض المرحلات الدوائر الاضافية عند بلوغ السقف وتزيد `soranet_handshake_capacity_reject_total`.
- **ارضية dummy** - الحد الادنى لعدد lanes التي تبقى حية مع حركة dummy حتى عندما يكون الطلب الفعلي اقل.
- **Payload سقف** - ميزانية uplink المخصصة للمسارات ذات المعدل الثابت بعد تطبيق نسبة السقف. يجب الا يتجاوزها المشغلون حتى لو توفرت سعة اضافية.
- **محفز التعطيل التلقائي** - نسبة تشبع مستمرة (متوسط لكل preset) تجعل runtime يهبط الى ارضية dummy. تستعاد السعة بعد حد الاستعادة (75% لـ `core`، 60% لـ `home`، 45% لـ `null`).

**مهم:** preset `null` مخصص للتجريب وdogfooding فقط؛ ولا يحقق ضمانات الخصوصية المطلوبة للدوائر الانتاجية.

## جدول tick -> bandwidth

تحمل كل خلية payload مقدار 1,024 B، لذا يعادل عمود KiB/sec عدد الخلايا المرسلة في الثانية. استخدم helper لتوسيع الجدول بقيم tick مخصصة.

| Tick (ms) | خلايا/ثانية | Payload KiB/sec | Payload Mb/s |
|-----------|-----------|-----------------|--------------|
| 5.0       | 200.00    | 200.00          | 1.64         |
| 7.5       | 133.33    | 133.33          | 1.09         |
| 10.0      | 100.00    | 100.00          | 0.82         |
| 15.0      | 66.67     | 66.67           | 0.55         |
| 20.0      | 50.00     | 50.00           | 0.41         |

المعادلة:

```
payload_mbps = (cell_bytes x 8 / 1_000_000) x (1000 / tick_ms)
```

مساعد CLI:

```bash
# Markdown table output for all presets plus default tick table
cargo xtask soranet-constant-rate-profile --tick-table --format markdown --json-out artifacts/soranet/constant_rate/report.json

# Restrict to a preset and emit JSON
cargo xtask soranet-constant-rate-profile --profile core --format json

# Custom tick series
cargo xtask soranet-constant-rate-profile --tick-table --tick-values 5,7.5,12,18 --format markdown
```

خيار `--format markdown` يصدر جداول بنمط GitHub لكل من ملخص presets وجدول ticks الاختياري حتى يمكن لصق المخرجات الحتمية في البوابة. اربطه مع `--json-out` لارجاع البيانات المولدة كدليل حوكمة.

## الاعدادات وoverrides

يعرض `tools/soranet-relay` presets في ملفات التهيئة وكذلك في overrides وقت التشغيل:

```bash
# Persisted in relay.json
"constant_rate_profile": "home"

# One-off override during rollout or maintenance
soranet-relay --config relay.json --constant-rate-profile core
```

مفتاح التهيئة يقبل `core` او `home` او `null` (الافتراضي `core`). تعد overrides عبر CLI مفيدة لتمارين staging او طلبات SOC التي تقلل duty cycle مؤقتا دون تعديل configs.

## حواجز MTU

- تستخدم خلايا payload مقدار 1,024 B بالاضافة الى ~96 B من Norito+Noise framing واصغر رؤوس QUIC/UDP، مما يبقي كل datagram اقل من MTU IPv6 الادنى 1,280 B.
- عند اضافة طبقات تغليف عبر الانفاق (WireGuard/IPsec) يجب **حتما** خفض `padding.cell_size` بحيث `cell_size + framing <= 1,280 B`. يفرض مدقق relay `padding.cell_size <= 1,136 B` (1,280 B - 48 B overhead UDP/IPv6 - 96 B framing).
- يجب على ملفات `core` تثبيت >=4 neighbors حتى في وضع idle كي تغطي lanes dummy دائما جزءا من guards PQ. يمكن لملفات `home` حصر دوائر المعدل الثابت في wallets/aggregators لكن يجب تطبيق back-pressure عندما تتجاوز نسبة التشبع 70% عبر ثلاث نوافذ telemetry.

## القياس والتنبيهات

تقوم relays بتصدير المقاييس التالية لكل preset:

- `soranet_constant_rate_active_neighbors`
- `soranet_constant_rate_queue_depth`
- `soranet_constant_rate_saturation_percent`
- `soranet_constant_rate_dummy_lanes` / `soranet_constant_rate_dummy_ratio`
- `soranet_constant_rate_slot_rate_hz`
- `soranet_constant_rate_ceiling_hits_total`
- `soranet_constant_rate_degraded`

نبه عندما:

1. ينخفض dummy ratio تحت ارضية preset (`core >= 4/8`, `home >= 2/2`, `null >= 1/1`) لاكثر من نافذتين.
2. ينمو `soranet_constant_rate_ceiling_hits_total` بوتيرة اسرع من hit واحد كل خمس دقائق.
3. يتحول `soranet_constant_rate_degraded` الى `1` خارج تمرين مخطط.

سجل تسمية preset وقائمة neighbors في تقارير الحوادث حتى يتمكن المدققون من اثبات ان سياسات المعدل الثابت طابقت متطلبات خارطة الطريق.
