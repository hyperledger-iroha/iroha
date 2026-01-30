---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-refactor-plan
title: خطة اعادة هيكلة دفتر Sora Nexus
description: نسخة مطابقة لـ `docs/source/nexus_refactor_plan.md` توضح اعمال التنظيف المرحلية لقاعدة شفرة Iroha 3.
---

:::note المصدر القانوني
تعكس هذه الصفحة `docs/source/nexus_refactor_plan.md`. ابق النسختين متوافقتين حتى تصل النسخة متعددة اللغات الى البوابة.
:::

# خطة اعادة هيكلة دفتر Sora Nexus

توثق هذه الوثيقة خارطة الطريق الفورية لاعادة هيكلة Sora Nexus Ledger ("Iroha 3"). تعكس مخطط المستودع الحالي والانتكاسات المرصودة في محاسبة genesis/WSV واجماع Sumeragi ومشغلات العقود الذكية واستعلامات اللقطات وربط المضيف pointer-ABI وترميزات Norito. الهدف هو الوصول الى معمارية متماسكة قابلة للاختبار دون محاولة ادخال كل الاصلاحات في تصحيح واحد ضخم.

## 0. مبادئ موجهة
- الحفاظ على سلوك حتمي عبر عتاد متنوع؛ استخدام التسريع فقط عبر feature flags اختيارية مع fallbacks متطابقة.
- Norito هي طبقة التسلسل. اي تغيير في الحالة/المخطط يجب ان يتضمن اختبارات round-trip لترميز وفك ترميز Norito وتحديث fixtures.
- يمر التكوين عبر `iroha_config` (user -> actual -> defaults). ازل toggles البيئية العشوائية من مسارات الانتاج.
- سياسة ABI تبقى V1 وغير قابلة للتفاوض. يجب على المضيفين رفض pointer types/syscalls غير المعروفة بشكل حتمي.
- `cargo test --workspace` و golden tests (`ivm`, `norito`, `integration_tests`) تبقى بوابة الاساس لكل مرحلة.

## 1. لقطة من طوبولوجيا المستودع
- `crates/iroha_core`: ممثلو Sumeragi وWSV ومحمل genesis وخطوط الانابيب (query, overlay, zk lanes) وغراء مضيف العقود الذكية.
- `crates/iroha_data_model`: المخطط المرجعي للبيانات والاستعلامات على السلسلة.
- `crates/iroha`: واجهة العميل المستخدمة من قبل CLI والاختبارات وSDK.
- `crates/iroha_cli`: واجهة CLI للمشغلين، وتكرر حاليا العديد من APIs الموجودة في `iroha`.
- `crates/ivm`: آلة افتراضية لبايت كود Kotodama ونقاط ادخال تكامل pointer-ABI للمضيف.
- `crates/norito`: codec للتسلسل مع محولات JSON وواجهات خلفية AoS/NCB.
- `integration_tests`: تأكيدات عبر المكونات تغطي genesis/bootstrap وSumeragi وtriggers وpagination وغيرها.
- الوثائق تشرح اهداف Sora Nexus Ledger (`nexus.md`, `new_pipeline.md`, `ivm.md`)، لكن التنفيذ مجزأ ومتهالك جزئيا مقارنة بالشفرة.

## 2. ركائز اعادة الهيكلة والمراحل

### المرحلة A - الاساسات والمراقبة
1. **Telemetria WSV + Snapshots**
   - تأسيس API للقطات في `state` (trait `WorldStateSnapshot`) تستخدمها الاستعلامات وSumeragi وCLI.
   - استخدام `scripts/iroha_state_dump.sh` لانتاج snapshots حتمية عبر `iroha state dump --format norito`.
2. **حتمية Genesis/Bootstrap**
   - اعادة هيكلة ادخال genesis ليمر عبر مسار واحد يعتمد Norito (`iroha_core::genesis`).
   - اضافة تغطية تكامل/ارتداد تعيد تشغيل genesis مع اول كتلة وتؤكد تطابق جذور WSV بين arm64/x86_64 (متابعة في `integration_tests/tests/genesis_replay_determinism.rs`).
3. **اختبارات ثبات عبر الحزم**
   - توسيع `integration_tests/tests/genesis_json.rs` للتحقق من ثوابت WSV وpipeline وABI في harness واحد.
   - تقديم scaffold `cargo xtask check-shape` يفشل عند schema drift (متابعة في backlog ادوات DevEx؛ انظر عنصر العمل في `scripts/xtask/README.md`).

### المرحلة B - WSV وسطح الاستعلام
1. **معاملات تخزين الحالة**
   - دمج `state/storage_transactions.rs` في محول معاملات يفرض ترتيب commit وكشف التعارض.
   - اختبارات الوحدة تتحقق الان من ان تعديلات assets/world/triggers تتراجع عند الفشل.
2. **اعادة هيكلة نموذج الاستعلام**
   - نقل منطق pagination/cursor الى مكونات قابلة لاعادة الاستخدام تحت `crates/iroha_core/src/query/`. مواءمة تمثيلات Norito في `iroha_data_model`.
   - اضافة snapshot queries لل triggers والassets والroles بترتيب حتمي (متابعة عبر `crates/iroha_core/tests/snapshot_iterable.rs` للتغطية الحالية).
3. **اتساق اللقطات**
   - ضمان ان CLI `iroha ledger query` يستخدم نفس مسار اللقطة مثل Sumeragi/fetchers.
   - اختبارات regression للقطات في CLI توجد تحت `tests/cli/state_snapshot.rs` (feature-gated للتشغيلات البطيئة).

### المرحلة C - خط انابيب Sumeragi
1. **الطوبولوجيا وادارة الحقبات**
   - استخراج `EpochRosterProvider` الى trait مع تطبيقات تعتمد على snapshots لل stake في WSV.
   - `WsvEpochRosterAdapter::from_peer_iter` يوفر منشئا بسيطا وداعما للمحاكاة في benches/tests.
2. **تبسيط تدفق الاجماع**
   - اعادة تنظيم `crates/iroha_core/src/sumeragi/*` الى وحدات: `pacemaker`, `aggregation`, `availability`, `witness` مع انواع مشتركة تحت `consensus`.
   - استبدال تمرير الرسائل العشوائي بأغلفة Norito نوعية وادخال property tests لتغيير العرض (متابعة في backlog رسائل Sumeragi).
3. **تكامل lane/proof**
   - مواءمة lane proofs مع التزامات DA وضمان ان gating لـ RBC موحد.
   - اختبار تكامل end-to-end `integration_tests/tests/extra_functional/seven_peer_consistency.rs` يتحقق الان من المسار مع تمكين RBC.

### المرحلة D - العقود الذكية ومضيفات pointer-ABI
1. **تدقيق حدود المضيف**
   - توحيد فحوصات pointer-type (`ivm::pointer_abi`) ومحولات المضيف (`iroha_core::smartcontracts::ivm::host`).
   - توقعات جدول المؤشرات وربط manifest المضيف مغطاة في `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` و`ivm_host_mapping.rs` التي تمارس خرائط TLV golden.
2. **Sandbox تنفيذ triggers**
   - اعادة هيكلة triggers لتعمل عبر `TriggerExecutor` مشترك يفرض الغاز والتحقق من المؤشرات وتسجيل الاحداث.
   - اضافة اختبارات regression لمشغلات call/time تغطي مسارات الفشل (متابعة عبر `crates/iroha_core/tests/trigger_failure.rs`).
3. **محاذاة CLI والعميل**
   - ضمان ان عمليات CLI (`audit`, `gov`, `sumeragi`, `ivm`) تعتمد على وظائف العميل المشتركة في `iroha` لتجنب الانحراف.
   - اختبارات snapshots JSON في CLI توجد في `tests/cli/json_snapshot.rs`; ابقها محدثة لضمان تطابق مخرجات الاوامر مع مرجع JSON القانوني.

### المرحلة E - تعزيز codec Norito
1. **سجل المخططات**
   - انشاء سجل مخطط Norito تحت `crates/norito/src/schema/` لتوفير ترميزات قانونية للانواع الاساسية.
   - تمت اضافة doc tests تتحقق من ترميز payloads نموذجية (`norito::schema::SamplePayload`).
2. **تحديث golden fixtures**
   - تحديث golden fixtures في `crates/norito/tests/*` لتتطابق مع مخطط WSV الجديد عند اكتمال اعادة الهيكلة.
   - `scripts/norito_regen.sh` يعيد توليد golden JSON لنوريتو بشكل حتمي عبر المساعد `norito_regen_goldens`.
3. **تكامل IVM/Norito**
   - التحقق من تسلسل manifests Kotodama من البداية الى النهاية عبر Norito، مع ضمان اتساق metadata pointer ABI.
   - `crates/ivm/tests/manifest_roundtrip.rs` يحافظ على تساوي Norito encode/decode للمانيفستات.

## 3. قضايا مشتركة
- **استراتيجية الاختبارات**: كل مرحلة ترقى unit tests -> crate tests -> integration tests. الاختبارات الفاشلة تلتقط الانتكاسات الحالية؛ الاختبارات الجديدة تمنع عودتها.
- **التوثيق**: بعد كل مرحلة، حدث `status.md` وادفع العناصر المفتوحة الى `roadmap.md` مع حذف المهام المكتملة.
- **معايير الاداء**: الحفاظ على benches الحالية في `iroha_core` و`ivm` و`norito`; اضافة قياسات اساس بعد اعادة الهيكلة للتحقق من عدم وجود regressions.
- **Feature flags**: الحفاظ على toggles على مستوى crate فقط للواجهات الخلفية التي تتطلب toolchains خارجية (`cuda`, `zk-verify-batch`). مسارات SIMD للـ CPU تبنى دائما وتختار في وقت التشغيل؛ وفر fallbacks قياسية حتمية للعتاد غير المدعوم.

## 4. الاجراءات الفورية
- Scaffolding للمرحلة A (snapshot trait + ربط telemetria) - راجع المهام القابلة للتنفيذ في تحديثات roadmap.
- تدقيق العيوب الاخير لـ `sumeragi` و`state` و`ivm` اظهر النقاط التالية:
  - `sumeragi`: allowances ل dead-code تحمي بث اثباتات view-change وحالة replay VRF وتصدير telemetria EMA. تبقى هذه الاجزاء gated حتى تتوفر تبسيطات تدفق الاجماع في المرحلة C وتسليمات تكامل lane/proof.
  - `state`: تنظيف `Cell` ومسار telemetria ينتقلان الى مسار telemetria WSV في المرحلة A، بينما تنضم ملاحظات SoA/parallel-apply الى backlog تحسين pipeline في المرحلة C.
  - `ivm`: عرض toggles CUDA والتحقق من envelopes وتغطية Halo2/Metal تتطابق مع اعمال حدود المضيف في المرحلة D ومع موضوع تسريع GPU العرضي؛ تبقى kernels على backlog GPU المخصص حتى الجاهزية.
- تحضير RFC عبر الفرق يلخص هذه الخطة من اجل sign-off قبل ادخال تغييرات شيفرة واسعة.

## 5. اسئلة مفتوحة
- هل يجب ان يبقى RBC اختياريا بعد P1، ام انه الزامي لخطوط دفتر Nexus؟ يتطلب قرار اصحاب المصلحة.
- هل نفرض مجموعات composability للـ DS في P1 ام نتركها معطلة حتى تنضج lane proofs؟
- ما الموقع القانوني لمعاملات ML-DSA-87؟ المرشح: crate جديد `crates/fastpq_isi` (قيد الانشاء).

---

_اخر تحديث: 2025-09-12_
