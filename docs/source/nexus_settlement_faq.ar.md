<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: ar
direction: rtl
source: docs/source/nexus_settlement_faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9ccbc52b2d34a410c5b724b6421fc91bd403cd40d0a03360315a2ae2e3e504ec
source_last_modified: "2025-11-21T14:07:47.531238+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/nexus_settlement_faq.md -->

# اسئلة شائعة عن settlement في Nexus

**رابط خارطة الطريق:** NX-14 - توثيق Nexus وrunbooks للمشغلين  
**الحالة:** مسودة 2026-03-24 (يعكس مواصفات settlement router و playbook الخاص بـ CBDC)  
**الجمهور:** مشغلون، مؤلفو SDK، ومراجعو الحوكمة الذين يستعدون لاطلاق Nexus (Iroha 3).

هذا الـ FAQ يجيب عن الاسئلة التي ظهرت خلال مراجعة NX-14 حول توجيه settlement، وتحويل XOR،
والتلِمتري، وادلة التدقيق. ارجع الى `docs/source/settlement_router.md` للمواصفة الكاملة و
`docs/source/cbdc_lane_playbook.md` لاعدادات السياسة الخاصة بـ CBDC.

> **TL;DR:** كل تدفقات settlement تمر عبر Settlement Router الذي يخصم buffers XOR في lanes العامة
> ويطبق رسوما خاصة بكل lane. يجب على المشغلين ابقاء اعدادات التوجيه (`config/config.toml`)
> ولوحات التلِمتري وسجلات التدقيق متزامنة مع manifests المنشورة.

## الاسئلة الشائعة

### اي lanes تتعامل مع settlement وكيف اعرف اين يقع DS الخاص بي؟

- كل dataspace يعلن `settlement_handle` في manifest الخاص به. التعيينات الافتراضية كما يلي:
  - `xor_global` للـ lanes العامة الافتراضية.
  - `xor_lane_weighted` للـ lanes العامة المخصصة التي تستمد السيولة من مكان اخر.
  - `xor_hosted_custody` للـ lanes الخاصة/CBDC (buffer XOR في escrow).
  - `xor_dual_fund` للـ lanes الهجينة/السرية التي تمزج تدفقات shielded + public.
- راجع `docs/source/nexus_lanes.md` لفئات lane و
  `docs/source/project_tracker/nexus_config_deltas/*.md` لاحدث موافقات الكتالوج. امر
  `irohad --sora --config ... --trace-config` يطبع الكتالوج الفعلي في runtime للاغراض التدقيقية.

### كيف يحدد Settlement Router اسعار التحويل؟

- يفرض router مسارا واحدا بتسعير حتمي:
  - في lanes العامة نستخدم مجمع سيولة XOR على السلسلة (DEX عام). اوكلاء الاسعار يعودون الى TWAP
    المعتمد من الحوكمة عندما تكون السيولة ضعيفة.
  - الـ lanes الخاصة تمول buffers XOR مسبقا. عند خصم settlement، يسجل router tuple التحويل
    `{lane_id, source_token, xor_amount, haircut}` ويطبق haircuts المعتمدة من الحوكمة
    (`haircut.rs`) اذا انحرفت buffers.
- الاعدادات تقع تحت `[settlement]` في `config/config.toml`. تجنب التعديلات المخصصة الا اذا طلبت
  الحوكمة ذلك. راجع `docs/source/settlement_router.md` لوصف الحقول.

### كيف يتم تطبيق الرسوم والخصومات؟

- الرسوم معرّفة لكل lane في manifest:
  - `base_fee_bps` - تطبق على كل خصم settlement.
  - `liquidity_haircut_bps` - تعوض مزودي السيولة المشتركة.
  - `rebate_policy` - اختياري (مثل خصومات CBDC الترويجية).
- يصدر router احداث `SettlementApplied` (صيغة Norito) مع تفاصيل الرسوم لكي تتمكن SDKs والمدققون
  من مطابقة قيود الـ ledger.

### اي تلِمتري يثبت ان settlements بحالة جيدة؟

- مقاييس Prometheus (تُصدر عبر `iroha_telemetry` و settlement router):
  - `nexus_settlement_latency_seconds{lane_id}` - يجب ان يبقى P99 اقل من 900 ms للـ lanes العامة /
    1200 ms للـ lanes الخاصة.
  - `settlement_router_conversion_total{source_token}` - يؤكد احجام التحويل لكل token.
  - `settlement_router_haircut_total{lane_id}` - تنبيه عند عدم كونه صفرا دون ملاحظة حوكمة مرتبطة.
  - `iroha_settlement_buffer_xor{lane_id,dataspace_id}` - يعرض خصومات XOR الحية لكل lane (micro units).
    تنبيه عندما يكون <25 %/10 % من Bmin.
  - `iroha_settlement_pnl_xor{lane_id,dataspace_id}` - تباين haircut المحقق لمطابقته مع P&L للخزينة.
  - `iroha_settlement_haircut_bp{lane_id,dataspace_id}` - قيمة epsilon الفعلية المطبقة في اخر بلوك؛
    المدققون يقارنونها مع سياسة router.
  - `iroha_settlement_swapline_utilisation{lane_id,dataspace_id,profile}` - استخدام خط الائتمان
    sponsor/MM؛ تنبيه فوق 80 %.
- `nexus_lane_block_height{lane,dataspace}` - اخر ارتفاع بلوك لوحدة lane/dataspace؛ ابقِ الاقران
  المجاورين ضمن بضعة slots من بعضهم.
- `nexus_lane_finality_lag_slots{lane,dataspace}` - عدد الـ slots بين الرأس العالمي واحدث بلوك للـ
  lane؛ تنبيه عندما >12 خارج التدريبات.
- `nexus_lane_settlement_backlog_xor{lane,dataspace}` - backlog ينتظر settlement في XOR؛ بوّب اعمال
  CBDC/الخاصة قبل تجاوز الحدود التنظيمية.

يحمل `settlement_router_conversion_total` تسميات `lane_id` و`dataspace_id` و`source_token` حتى
تثبت اي اصل gas قاد كل تحويل. ويجمع `settlement_router_haircut_total` وحدات XOR (وليس micro amounts
الخام)، ما يسمح للخزينة بمطابقة سجل haircut مباشرة من Prometheus.
- `lane_settlement_commitments[*].swap_metadata.volatility_class` يوضح ما اذا طبق router فئة الهامش
  `stable` او `elevated` او `dislocated`. يجب ربط قيود elevated/dislocated بسجل حادث او ملاحظة
  حوكمة.
- لوحات التحكم: `dashboards/grafana/nexus_settlement.json` بالاضافة الى نظرة `nexus_lanes.json`.
  اربط التنبيهات بـ `dashboards/alerts/nexus_audit_rules.yml`.
- عند تدهور تلِمتري settlement، سجّل الحادث وفقا للـ runbook في `docs/source/nexus_operations.md`.

### كيف اصدّر تلِمتري lane للجهات التنظيمية؟

شغّل المساعد التالي عندما يطلب المنظم جدول الـ lanes:

```
cargo xtask nexus-lane-audit \
  --status artifacts/status.json \
  --json-out artifacts/nexus_lane_audit.json \
  --parquet-out artifacts/nexus_lane_audit.parquet \
  --markdown-out artifacts/nexus_lane_audit.md \
  --captured-at 2026-02-12T09:00:00Z \
  --lane-compliance artifacts/lane_compliance_evidence.json
```

* `--status` يقبل JSON blob المعاد من `iroha status --format json`.
* `--json-out` يلتقط array JSON قانونيا لكل lane (aliases، dataspace، ارتفاع البلوك، finality lag،
  سعة/استغلال TEU، عدادات scheduler trigger + utilization، RBC throughput، backlog، بيانات الحوكمة، الخ.).
* `--parquet-out` يكتب نفس الحمولة كملف Parquet (schema Arrow) جاهز للجهات التنظيمية التي تتطلب
  ادلة عمودية.
* `--markdown-out` يصدر ملخصا مقروءا يميز lanes المتاخرة و backlog غير الصفري وادلة الامتثال
  المفقودة و manifests المعلقة؛ الافتراضي `artifacts/nexus_lane_audit.md`.
* `--lane-compliance` اختياري؛ عند تمريره يجب ان يشير الى manifest JSON الموصوف في وثيقة الامتثال
  حتى تدمج الصفوف المصدرة سياسة lane المطابقة وتواقيع المراجعين ولقطة المقاييس ومقتطفات audit log.

ارشِف كلا المخرجين تحت `artifacts/` مع ادلة routed-trace (لقطات من `nexus_lanes.json`، حالة
Alertmanager، و `nexus_lane_rules.yml`).

### ما الادلة التي يتوقعها المدققون؟

1. **Snapshot للconfig** — التقط `config/config.toml` مع قسم `[settlement]` وكتالوج lanes المشار
   اليه في manifest الحالي.
2. **سجلات router** — ارشف `settlement_router.log` يوميا؛ يتضمن IDs settlement المهاشَة وخصومات
   XOR واماكن تطبيق haircuts.
3. **Exports للتلِمتري** — snapshot اسبوعي للمقاييس المذكورة اعلاه.
4. **تقرير مطابقة** — اختياري لكنه موصى به: صدّر قيود `SettlementRecordV1`
   (انظر `docs/source/cbdc_lane_playbook.md`) وقارنها مع ledger الخزينة.

### هل تحتاج SDK الى معالجة خاصة للـ settlement؟

- يجب على SDKs:
  - توفير helpers للاستعلام عن احداث settlement (`/v2/settlement/records`) وتفسير سجلات
    `SettlementApplied`.
  - اظهار IDs الخاصة بالـ lane و settlement handles في اعدادات العميل حتى يوجه المشغلون
    المعاملات بشكل صحيح.
  - مطابقة payloads Norito المعرفة في `docs/source/settlement_router.md` (مثل
    `SettlementInstructionV1`) مع اختبارات end-to-end.
- يوضح quickstart الخاص بـ Nexus SDK (القسم التالي) مقتطفات لكل لغة للانضمام الى الشبكة العامة.

### كيف يتفاعل settlement مع الحوكمة او مكابح الطوارئ؟

- يمكن للحوكمة ايقاف handles تسوية محددة عبر تحديثات manifest. يحترم router العلم `paused` ويرفض
  settlements الجديدة بخطأ حتمي (`ERR_SETTLEMENT_PAUSED`).
- "haircut clamps" الطارئة تحد من خصومات XOR القصوى لكل بلوك لمنع استنزاف buffers المشتركة.
- يجب على المشغلين مراقبة `governance.settlement_pause_total` واتباع قالب الحادث في
  `docs/source/nexus_operations.md`.

### اين ابلّغ عن اخطاء او اطلب تغييرات؟

- فجوات الميزات -> افتح issue بعلامة `NX-14` واربطها بخارطة الطريق.
- حوادث settlement العاجلة -> نادِ Nexus primary (انظر `docs/source/nexus_operations.md`) وارفق
  سجلات router.
- تصحيحات الوثائق -> قدم PRs لهذا الملف ونظائره في البوابة
  (`docs/portal/docs/nexus/overview.md`, `docs/portal/docs/nexus/operations.md`).

### هل يمكنك عرض امثلة لتدفقات settlement؟

الامثلة التالية تبين ما يتوقعه المدققون لاكثر انواع lanes شيوعا. التقط سجل router، و hashes
الـ ledger، وتصدير التلِمتري المطابق لكل سيناريو حتى يتمكن المراجعون من اعادة تشغيل الدليل.

#### Lane CBDC خاصة (`xor_hosted_custody`)

فيما يلي سجل router مقتطع لـ lane CBDC خاصة تستخدم handle hosted custody. يثبت السجل خصومات XOR
الحتمية وتكوين الرسوم ومعرفات التلِمتري:

```text
2026-03-24T11:42:07Z settlement_router lane=3 dataspace=ds::cbdc::jp
    handle=xor_hosted_custody settlement_id=0x9c2f...a413
    source_token=JPYCBDC amount=125000.00
    xor_debited=312.500000 xor_rate=400.000000 haircut_bps=25 base_fee_bps=15
    fee_breakdown={base=0.046875, haircut=0.078125}
    ledger_tx=0x7ab1...ff11 telemetry_trace=nexus-settle-20260324T1142Z-lane3
```

في Prometheus يجب ان ترى المقاييس المقابلة:

```text
nexus_settlement_latency_seconds{lane_id="3"} 0.842
settlement_router_conversion_total{lane_id="3",source_token="JPYCBDC"} += 1
settlement_router_haircut_total{lane_id="3"} += 0.078125
```

ارشِف مقتطف السجل و hash معاملة ledger وتصدير المقاييس معا حتى يتمكن المدققون من اعادة بناء
التدفق. توضح الامثلة التالية كيفية تسجيل الادلة للـ lanes العامة والهجينة/السرية.

#### Lane عامة (`xor_global`)

تقوم data spaces العامة بالتوجيه عبر `xor_global`، لذا يخصم router buffer DEX المشترك ويسجل TWAP
الحي الذي سعر التحويل. ارفق سجل TWAP او ملاحظة حوكمة عندما يعود الـ oracle الى قيمة مخزنة.

```text
2026-03-25T08:11:04Z settlement_router lane=0 dataspace=ds::public::creator
    handle=xor_global settlement_id=0x81cc...991c
    source_token=XOR amount=42.000000
    xor_debited=42.000000 xor_rate=1.000000 haircut_bps=0 base_fee_bps=10
    fee_breakdown={base=0.004200, liquidity=0.000000}
    dex_twap_id=twap-20260325T0810Z ledger_tx=0x319e...dd72 telemetry_trace=nexus-settle-20260325T0811Z-lane0
```

المقاييس تثبت نفس التدفق:

```text
nexus_settlement_latency_seconds{lane_id="0"} 0.224
settlement_router_conversion_total{lane_id="0",source_token="XOR"} += 1
settlement_router_haircut_total{lane_id="0"} += 0
```

احفظ سجل TWAP وسجل router ولقطة التلِمتري و hash الـ ledger في نفس bundle الادلة. عند اطلاق
تنبيهات تاخر lane 0 او حداثة TWAP، اربط تذكرة الحادث بهذه الحزمة.

#### Lane هجينة/سرية (`xor_dual_fund`)

تمزج الـ lanes الهجينة buffers shielded مع احتياطيات XOR العامة. يجب ان يوضح كل settlement اي
bucket زود XOR وكيف قسمت سياسة haircut الرسوم. يكشف سجل router تلك التفاصيل عبر كتلة metadata
الخاصة بالـ dual-fund:

```text
2026-03-26T19:54:31Z settlement_router lane=9 dataspace=ds::hybrid::art
    handle=xor_dual_fund settlement_id=0x55d2...c0ab
    source_token=ARTCREDIT amount=9800.00
    xor_debited_public=12.450000 xor_debited_shielded=11.300000
    xor_rate_public=780.000000 xor_rate_shielded=820.000000
    haircut_bps=35 base_fee_bps=20 dual_fund_ratio=0.52
    fee_breakdown={base=0.239000, haircut=0.418750}
    ledger_tx=0xa924...1104 telemetry_trace=nexus-settle-20260326T1954Z-lane9
```

```text
nexus_settlement_latency_seconds{lane_id="9"} 0.973
settlement_router_conversion_total{lane_id="9",source_token="ARTCREDIT"} += 1
settlement_router_haircut_total{lane_id="9"} += 0.418750
```

ارشِف سجل router مع سياسة dual-fund (مقتطف من كتالوج الحوكمة) وتصدير `SettlementRecordV1` للـ lane
ومقتطف التلِمتري حتى يتمكن المدققون من تاكيد ان التقسيم shielded/public احترم حدود الحوكمة.

حافظ على تحديث هذا FAQ عند تغير سلوك settlement router او عند ادخال الحوكمة فئات جديدة من lanes
وسياسات الرسوم.

</div>
