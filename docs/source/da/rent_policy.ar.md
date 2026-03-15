---
lang: ar
direction: rtl
source: docs/source/da/rent_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7cdc46bcd87af7924817a94900c8fad2c23570607f4065f19d8a42d259fe83f
source_last_modified: "2026-01-22T15:38:30.661606+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# سياسة الإيجار والحوافز المتعلقة بتوفر البيانات (DA-7)

_الحالة: الصياغة — المالكون: مجموعة العمل الاقتصادية / فريق الخزانة / التخزين_

يقدم عنصر خريطة الطريق **DA-7** إيجارًا صريحًا مقومًا بـ XOR لكل كائن ثنائي كبير الحجم
تم إرسالها إلى `/v2/da/ingest`، بالإضافة إلى المكافآت التي تكافئ تنفيذ PDP/PoTR و
خدم الخروج لجلب العملاء. تحدد هذه الوثيقة المعلمات الأولية،
تمثيل نموذج البيانات الخاص بهم، وسير عمل الحساب المستخدم بواسطة Torii،
أدوات تطوير البرامج ولوحات معلومات الخزانة.

## هيكل السياسة

تم ترميز السياسة كـ [`DaRentPolicyV1`](/crates/iroha_data_model/src/da/types.rs)
ضمن نموذج البيانات. Torii وأدوات الإدارة تستمر في السياسة
حمولات Norito بحيث يمكن إعادة حساب عروض أسعار الإيجار ودفاتر الحوافز
حتمية. يكشف المخطط عن خمسة مقابض:

| المجال | الوصف | الافتراضي |
|-------|------------|---------|
| `base_rate_per_gib_month` | XOR يتم تحصيله لكل جيجا بايت لكل شهر من الاحتفاظ. | `250_000` ميكرو-XOR (0.25 XOR) |
| `protocol_reserve_bps` | حصة الإيجار الموجهة إلى احتياطي البروتوكول (نقاط الأساس). | `2_000` (20%) |
| `pdp_bonus_bps` | النسبة المئوية للمكافأة لكل تقييم PDP ناجح. | `500` (5%) |
| `potr_bonus_bps` | نسبة المكافأة لكل تقييم PoTR ناجح. | `250` (2.5%) |
| `egress_credit_per_gib` | يُدفع الائتمان عندما يقدم مقدم الخدمة 1 جيجا بايت من بيانات DA. | `1_500` مايكرو XOR |

يتم التحقق من صحة كافة قيم نقطة الأساس مقابل `BASIS_POINTS_PER_UNIT` (10000).
يجب أن تنتقل تحديثات السياسة عبر الإدارة، وتكشف كل عقدة Torii عن
السياسة النشطة عبر قسم التكوين `torii.da_ingest.rent_policy`
(`iroha_config`). يمكن للمشغلين تجاوز الإعدادات الافتراضية في `config.toml`:

```toml
[torii.da_ingest.rent_policy]
base_rate_per_gib_month_micro = 250000        # 0.25 XOR/GiB-month
protocol_reserve_bps = 2000                   # 20% protocol reserve
pdp_bonus_bps = 500                           # 5% PDP bonus
potr_bonus_bps = 250                          # 2.5% PoTR bonus
egress_credit_per_gib_micro = 1500            # 0.0015 XOR/GiB egress credit
```

تقبل أدوات CLI (`iroha app da rent-quote`) نفس مدخلات سياسة Norito/JSON
وتنبعث منها المصنوعات اليدوية التي تعكس `DaRentPolicyV1` النشط دون الوصول
العودة إلى حالة Torii. قم بتوفير لقطة السياسة المستخدمة في عملية الاستيعاب حتى يتم
يبقى الاقتباس قابلا للتكرار.

### التحف الدائمة للإيجار

قم بتشغيل `iroha app da rent-quote --gib <size> --months <months> --quote-out <path>` إلى
قم بإصدار الملخص الذي يظهر على الشاشة ومصنوعات JSON مطبوعة بشكل جميل. الملف
يسجل `policy_source`، لقطة `DaRentPolicyV1` المضمنة، المحسوبة
`DaRentQuote`، و`ledger_projection` المشتق (متسلسل عبر
[`DaRentLedgerProjection`](/crates/iroha_data_model/src/da/types.rs)) مما يجعلها مناسبة للوحات معلومات الخزانة ودفتر الأستاذ ISI
خطوط الأنابيب. عندما يشير `--quote-out` إلى دليل متداخل، تقوم واجهة سطر الأوامر (CLI) بإنشاء أي دليل
الآباء المفقودون، حتى يتمكن المشغلون من توحيد المواقع مثل
`artifacts/da/rent_quotes/<timestamp>.json` إلى جانب حزم أدلة DA الأخرى.
قم بإرفاق القطعة الأثرية بموافقات الإيجار أو التسوية حتى يتم تشغيل XOR
التوزيع (الإيجار الأساسي، والاحتياطي، ومكافآت PDP/PoTR، وائتمانات الخروج) هو
قابلة للتكرار. قم بتمرير `--policy-label "<text>"` لتجاوز هذا تلقائيًا
وصف `policy_source` المشتق (مسارات الملفات، الافتراضي المضمن، وما إلى ذلك) باستخدام
علامة يمكن قراءتها بواسطة الإنسان مثل تذكرة الحوكمة أو تجزئة البيان؛ الديكورات CLI
هذه القيمة وترفض السلاسل الفارغة/المسافات البيضاء فقط وبالتالي فإن الأدلة المسجلة
يبقى قابلاً للتدقيق

```json
{
  "policy_source": "policy JSON `configs/da/rent_policy.json`",
  "gib": 10,
  "months": 3,
  "policy": { "...": "DaRentPolicyV1 fields elided" },
  "quote": { "...": "DaRentQuote breakdown" },
  "ledger_projection": {
    "rent_due": { "micro": 7500000 },
    "protocol_reserve_due": { "micro": 1500000 },
    "provider_reward_due": { "micro": 6000000 },
    "pdp_bonus_pool": { "micro": 375000 },
    "potr_bonus_pool": { "micro": 187500 },
    "egress_credit_per_gib": { "micro": 1500 }
  }
}
```يتغذى قسم إسقاط دفتر الأستاذ مباشرة في ISIs لدفتر الأستاذ DA: it
يحدد دلتا XOR المخصصة لاحتياطي البروتوكول، ومدفوعات المزود، و
مجموعات المكافآت لكل إثبات دون الحاجة إلى كود تنسيق مخصص.

### إنشاء خطط دفتر الأستاذ الإيجار

قم بتشغيل `iroha app da rent-ledger --quote <path> --payer-account <id> --treasury-account <id> --protocol-reserve-account <id> --provider-account <id> --pdp-bonus-account <id> --potr-bonus-account <id> --asset-definition xor#sora`
لتحويل عرض أسعار الإيجار المستمر إلى عمليات نقل دفتر الأستاذ القابلة للتنفيذ. الأمر
يوزع `ledger_projection` المضمن، ويصدر تعليمات Norito `Transfer`
التي تجمع الإيجار الأساسي في الخزانة، توجه الاحتياطي/المزود
الأجزاء، والتمويل المسبق لمجموعات مكافآت PDP/PoTR مباشرة من الدافع. ال
يعكس إخراج JSON البيانات التعريفية للاقتباس حتى تتمكن أدوات CI والخزانة من التفكير
عن نفس القطعة الأثرية:

```json
{
  "quote_path": "artifacts/da/rent_quotes/2025-12-07/rent.json",
  "rent_due_micro_xor": 7500000,
  "protocol_reserve_due_micro_xor": 1500000,
  "provider_reward_due_micro_xor": 6000000,
  "pdp_bonus_pool_micro_xor": 375000,
  "potr_bonus_pool_micro_xor": 187500,
  "egress_credit_per_gib_micro_xor": 1500,
  "instructions": [
    { "Transfer": { "...": "payer -> treasury base rent instruction elided" }},
    { "Transfer": { "...": "treasury -> reserve" }},
    { "Transfer": { "...": "treasury -> provider payout" }},
    { "Transfer": { "...": "payer -> PDP bonus escrow" }},
    { "Transfer": { "...": "payer -> PoTR bonus escrow" }}
  ]
}
```

يتيح الحقل `egress_credit_per_gib_micro_xor` النهائي للوحات المعلومات والدفع
يقوم القائمون على الجدولة بمحاذاة تعويضات الخروج مع سياسة الإيجار التي أنتجت
الاقتباس دون إعادة حساب رياضيات السياسة في البرمجة النصية الغراء.

## مثال الاقتباس

```rust
use iroha_data_model::da::types::DaRentPolicyV1;

// 10 GiB retained for 3 months.
let policy = DaRentPolicyV1::default();
let quote = policy.quote(10, 3).expect("policy validated");

assert_eq!(quote.base_rent.as_micro(), 7_500_000);      // 7.5 XOR total rent
assert_eq!(quote.protocol_reserve.as_micro(), 1_500_000); // 20% reserve
assert_eq!(quote.provider_reward.as_micro(), 6_000_000);  // Direct provider payout
assert_eq!(quote.pdp_bonus.as_micro(), 375_000);          // PDP success bonus
assert_eq!(quote.potr_bonus.as_micro(), 187_500);         // PoTR success bonus
assert_eq!(quote.egress_credit_per_gib.as_micro(), 1_500);
```

عرض الأسعار قابل للتكرار عبر عقد Torii ومجموعات SDK وتقارير الخزانة نظرًا لأن
يستخدم بنيات Norito الحتمية بدلاً من الرياضيات المخصصة. يمكن للمشغلين
قم بإرفاق JSON/CBOR المشفر `DaRentPolicyV1` بمقترحات الحوكمة أو الإيجار
عمليات التدقيق لإثبات المعلمات التي كانت سارية لأي نقطة معينة.

## المكافآت والاحتياطيات

- **احتياطي البروتوكول:** يقوم `protocol_reserve_bps` بتمويل احتياطي XOR الذي يدعم
  إعادة التكرار في حالات الطوارئ وخفض المبالغ المستردة. وزارة الخزانة تتعقب هذا الدلو
  بشكل منفصل لضمان تطابق أرصدة دفتر الأستاذ مع المعدل الذي تم تكوينه.
- **مكافآت PDP/PoTR:** يحصل كل تقييم إثبات ناجح على مكافأة إضافية
  الدفع مشتق من `base_rent × bonus_bps`. عندما يصدر برنامج جدولة DA دليلاً
  الإيصالات تتضمن علامات نقطة الأساس حتى يمكن إعادة تشغيل الحوافز.
- **رصيد الخروج:** يسجل مقدمو الخدمة حجم GiB الذي يتم تقديمه لكل بيان، مضروبًا في
  `egress_credit_per_gib`، وأرسل الإيصالات عبر `iroha app da prove-availability`.
  تحافظ سياسة الإيجار على تزامن المبلغ لكل جيجا بايت مع الإدارة.

## التدفق التشغيلي

1. **Ingest:** يقوم `/v2/da/ingest` بتحميل `DaRentPolicyV1` النشط، ويقتبس الإيجار
   استنادًا إلى حجم الكائن الثنائي الكبير والاحتفاظ به، ويقوم بتضمين الاقتباس في Norito
   واضح. يوقع مقدم الطلب على بيان يشير إلى تجزئة الإيجار و
   معرف تذكرة التخزين.
2. **المحاسبة:** تقوم نصوص الخزانة بفك تشفير البيان، اتصل
   `DaRentPolicyV1::quote`، وملء دفاتر الإيجار (الإيجار الأساسي، الاحتياطي،
   المكافآت، وائتمانات الخروج المتوقعة). أي تناقض بين الإيجار المسجل
   والاقتباسات المعاد حسابها تفشل CI.
3. **مكافآت الإثبات:** عندما تشير برامج جدولة PDP/PoTR إلى النجاح، فإنها ترسل إيصالاً
   يحتوي على ملخص البيان ونوع الإثبات ومكافأة XOR المستمدة من
   السياسة. يمكن للحوكمة مراجعة المدفوعات عن طريق إعادة حساب نفس عرض الأسعار.
4. **تعويض الخروج:** يقوم منسقو الجلب بإرسال ملخصات الخروج الموقعة.
   يقوم Torii بضرب عدد GiB في `egress_credit_per_gib` ويصدر الدفع
   تعليمات ضد ضمان الإيجار.

## القياس عن بعدتعرض عقد Torii استخدام الإيجار عبر مقاييس Prometheus التالية (التسميات:
`cluster`، `storage_class`):

- `torii_da_rent_gib_months_total` — أشهر GiB مقتبسة بواسطة `/v2/da/ingest`.
- `torii_da_rent_base_micro_total` — الإيجار الأساسي (micro XOR) المستحق عند الاستيعاب.
- `torii_da_protocol_reserve_micro_total` — مساهمات البروتوكول الاحتياطية.
- `torii_da_provider_reward_micro_total` — دفعات الإيجار من جانب المزود.
- `torii_da_pdp_bonus_micro_total` و`torii_da_potr_bonus_micro_total` —
  يتم الحصول على مجموعات مكافآت PDP/PoTR من عرض الأسعار المستوعب.

تعتمد لوحات المعلومات الاقتصادية على هذه العدادات لضمان وجود معلومات استخباراتية لدفتر الأستاذ، والصنابير الاحتياطية،
وجداول مكافآت PDP/PoTR جميعها تتوافق مع معايير السياسة المعمول بها لكل منها
فئة الكتلة والتخزين. لوحة SoraFS لصحة السعة Grafana
(`dashboards/grafana/sorafs_capacity_health.json`) يعرض الآن لوحات مخصصة
لتوزيع الإيجار، واستحقاق مكافأة PDP/PoTR، والتقاط GiB شهرًا، مما يسمح بذلك
الخزانة للتصفية حسب مجموعة Torii أو فئة التخزين عند مراجعة الاستيعاب
الحجم والدفعات.

## الخطوات التالية

- ✅ تقوم إيصالات `/v2/da/ingest` الآن بتضمين `rent_quote` وتعرض أسطح CLI/SDK المقتبسة
  الإيجار الأساسي والحصة الاحتياطية ومكافآت PDP/PoTR حتى يتمكن مقدمو الطلبات من مراجعة التزامات XOR من قبل
  ارتكاب الحمولات.
- قم بدمج سجل الإيجار مع خلاصات سجل الطلبات/سجل الطلبات القادمة
  لإثبات أن مقدمي الخدمة ذوي التوفر العالي يتلقون الدفعات الصحيحة.