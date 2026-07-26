---
lang: ar
direction: rtl
source: docs/source/nexus_refactor_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44b7100fddd377c97dfcab678ce425ec35edfa4a1276f9b6a22aa2c64135a94d
source_last_modified: "2025-11-02T04:40:40.017979+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/nexus_refactor_plan.md -->

# خطة اعادة هيكلة دفتر Sora Nexus

تلتقط هذه الوثيقة خارطة الطريق الفورية لاعادة هيكلة دفتر Sora Nexus ("Iroha 3"). وتعكس بنية
المستودع الحالية والانحدارات المرصودة في مسك حسابات genesis/WSV، واجماع Sumeragi، ومحفزات
العقود الذكية، واستعلامات snapshot، وروابط مضيف pointer-ABI، وكودكات Norito. الهدف هو الوصول
الى معمارية متماسكة وقابلة للاختبار دون محاولة انزال كل الاصلاحات في رقعة واحدة ضخمة.

## 0. المبادئ الارشادية
- الحفاظ على السلوك الحتمي عبر عتاد متنوع؛ الاستفادة من التسريع فقط عبر feature flags اختيارية
  مع بدائل مطابقة.
- Norito هي طبقة التسلسل. اي تغيير في الحالة/المخطط يجب ان يتضمن اختبارات round-trip للتشفير/فك
  التشفير عبر Norito وتحديثات للـ fixtures.
- التهيئة تمر عبر `iroha_config` (user -> actual -> defaults). ازالة مفاتيح البيئة العشوائية من
  مسارات الانتاج.
- سياسة ABI تبقى V1 وغير قابلة للتفاوض. يجب على المضيفين رفض انواع المؤشرات/الاستدعاءات غير
  المعروفة بشكل حتمي.
- `cargo test --workspace` والاختبارات الذهبية (`ivm`, `norito`, `integration_tests`) تبقى بوابة
  الاساس لكل محطة.

## 1. لقطة لطوبولوجيا المستودع
- `crates/iroha_core`: ممثلو Sumeragi، WSV، محمل genesis، خطوط الانابيب (query, overlay, zk lanes)،
  ربط المضيف للعقود الذكية.
- `crates/iroha_data_model`: المخطط المعتمد للبيانات على السلسلة والاستعلامات.
- `crates/iroha`: واجهة عميل تستخدمها CLI والاختبارات وSDK.
- `crates/iroha_cli`: واجهة سطر اوامر للمشغلين تعكس حاليا عددا كبيرا من واجهات `iroha`.
- `crates/ivm`: VM بايت كود Kotodama ونقاط دمج مضيف pointer-ABI.
- `crates/norito`: كودك تسلسل مع محولات JSON وbackends AoS/NCB.
- `integration_tests`: تحقق عابر للمكونات يغطي genesis/bootstrap وSumeragi وtriggers والترقيم، الخ.
- الوثائق تصف اهداف دفتر Sora Nexus (`nexus.md`, `new_pipeline.md`, `ivm.md`)، لكن التنفيذ مجزأ
  وجزئيا متقادم مقارنة بالكود.

## 2. اعمدة الهيكلة والمحطات

### المرحلة A - الاساسات والرصد
1. **تلِمتري WSV + snapshots**
   - تأسيس واجهة snapshot معيارية في `state` (trait `WorldStateSnapshot`) تستخدمها الاستعلامات
     وSumeragi وCLI.
   - استخدام `scripts/iroha_state_dump.sh` لانتاج snapshots حتمية عبر
     `iroha state dump --format norito`.
2. **حتمية genesis/bootstrap**
   - اعادة هيكلة ابتلاع genesis عبر خط واحد مدعوم بـ Norito (`iroha_core::genesis`).
   - اضافة تغطية تكامل/انحدار تعيد تشغيل genesis مع اول كتلة وتثبت تطابق جذور WSV على
     arm64/x86_64 (متابعة في `integration_tests/tests/genesis_replay_determinism.rs`).
3. **اختبارات الثبات عبر crates**
   - توسيع `integration_tests/tests/genesis_json.rs` للتحقق من ثوابت WSV وخط الانابيب وABI ضمن
     harness واحد.
   - ادخال scaffold `cargo xtask check-shape` يفشل عند انحراف المخطط (متابعة في backlog ادوات
     DevEx؛ راجع action item في `scripts/xtask/README.md`).

### المرحلة B - WSV وسطح الاستعلام
1. **معاملات تخزين الحالة**
   - دمج `state/storage_transactions.rs` في محول معاملات يفرض ترتيب commit وكشف التعارضات.
   - الاختبارات الوحدوية تتحقق الان من ان تعديلات assets/world/triggers تتراجع عند الفشل.
2. **اعادة هيكلة نموذج الاستعلام**
   - نقل منطق الترقيم/cursor الى مكونات قابلة لاعادة الاستخدام تحت `crates/iroha_core/src/query/`.
     مواءمة تمثيلات Norito في `iroha_data_model`.
   - اضافة snapshot queries لـ triggers وassets وroles بترتيب حتمي (متابعة عبر
     `crates/iroha_core/tests/snapshot_iterable.rs` للتغطية الحالية).
3. **اتساق snapshots**
   - ضمان ان CLI `iroha ledger query` يستخدم نفس مسار snapshot مثل Sumeragi/fetchers.
   - اختبارات انحدار snapshot للـ CLI موجودة في `tests/cli/state_snapshot.rs` (feature-gated
     للجولات البطيئة).

### المرحلة C - خط Sumeragi
1. **الطوبولوجيا وادارة الحقب**
   - استخراج `EpochRosterProvider` كـ trait مع تطبيقات تستند الى snapshots رهن WSV.
   - `WsvEpochRosterAdapter::from_peer_iter` يقدم منشئا بسيطا وسهل المحاكاة للـ benches/tests.
2. **تبسيط تدفق الاجماع**
   - اعادة تنظيم `crates/iroha_core/src/sumeragi/*` الى وحدات: `pacemaker`, `aggregation`,
     `availability`, `witness` مع انواع مشتركة تحت `consensus`.
   - استبدال تمرير الرسائل العشوائي بــ Norito envelopes مهيكلة وادخال property tests
     للـ view-change (متابعة في backlog رسائل Sumeragi).
3. **تكامل lanes/proofs**
   - مواءمة اثباتات lanes مع التزامات DA وضمان توحيد RBC gating.
   - اختبار التكامل end-to-end `integration_tests/tests/extra_functional/seven_peer_consistency.rs`
     يتحقق الان من المسار المفعّل بـ RBC.

### المرحلة D - العقود الذكية ومضيفات pointer-ABI
1. **تدقيق حدود المضيف**
   - توحيد فحوص انواع المؤشرات (`ivm::pointer_abi`) ومحولات المضيف
     (`iroha_core::smartcontracts::ivm::host`).
   - توقعات جدول المؤشرات وروابط host manifest مغطاة بواسطة
     `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` و`ivm_host_mapping.rs`، التي تمارس
     mappings TLV الذهبية.
2. **صندوق تنفيذ المحفزات**
   - اعادة هيكلة المحفزات لتعمل عبر `TriggerExecutor` موحد يفرض gas والتحقق من المؤشرات وتسجيل
     الاحداث.
   - اضافة اختبارات انحدار لمحفزات call/time تغطي مسارات الفشل (متابعة عبر
     `crates/iroha_core/tests/trigger_failure.rs`).
3. **مواءمة CLI والعميل**
   - ضمان ان عمليات CLI (`audit`, `gov`, `sumeragi`, `ivm`) تعتمد على وظائف العميل المشتركة في
     `iroha` لتجنب drift.
   - اختبارات snapshot JSON للـ CLI موجودة في `tests/cli/json_snapshot.rs`؛ حافظ عليها محدثة حتى
     تبقى مخرجات اوامر core مطابقة للمرجع JSON القياسي.

### المرحلة E - تقوية كودك Norito
1. **Schema registry**
   - انشاء سجل مخططات Norito تحت `crates/norito/src/schema/` لاستخراج الترميزات القياسية لانواع
     البيانات الاساسية.
   - اضافة doc tests تتحقق من ترميز payloads نموذجية (`norito::schema::SamplePayload`).
2. **تحديث golden fixtures**
   - تحديث `crates/norito/tests/*` golden fixtures لتطابق مخطط WSV الجديد عند هبوط الهيكلة.
   - `scripts/norito_regen.sh` يعيد توليد goldens Norito JSON بشكل حتمي عبر helper
     `norito_regen_goldens`.
3. **تكامل IVM/Norito**
   - التحقق من تسلسل manifest الخاص بـ Kotodama من النهاية الى النهاية عبر Norito مع ضمان
     اتساق metadata الخاصة بـ pointer ABI.
   - `crates/ivm/tests/manifest_roundtrip.rs` يحافظ على تكافؤ Norito encode/decode للـ manifests.

## 3. قضايا عابرة
- **استراتيجية الاختبار**: كل مرحلة ترفع اختبارات وحدوية -> اختبارات crate -> اختبارات تكامل.
  الاختبارات الفاشلة تلتقط الانحدارات الحالية؛ والاختبارات الجديدة تمنع عودتها.
- **التوثيق**: بعد كل مرحلة، حدث `status.md` وادفع العناصر المفتوحة الى `roadmap.md` مع حذف
  المهام المكتملة.
- **مقاييس الاداء**: الحفاظ على benches الحالية في `iroha_core` و`ivm` و`norito`؛ اضافة قياسات
  اساس بعد الهيكلة للتحقق من عدم وجود انحدارات.
- **Feature flags**: ابقاء toggles على مستوى crate فقط للواجهات التي تتطلب toolchains خارجية
  (`cuda`, `zk-verify-batch`). مسارات SIMD للـ CPU تبنى دائما وتختار في runtime؛ قدم بدائل
  scalar حتمية للعتاد غير المدعوم.

## 4. اجراءات فورية
- Scaffolding للمرحلة A (snapshot trait + wiring التلِمتري) - راجع المهام العملية في تحديثات
  roadmap.
- تدقيق العيوب الاخير لـ `sumeragi` و`state` و`ivm` ابرز ما يلي:
  - `sumeragi`: سماحات dead-code تحمي بث proof الخاص بـ view-change وحالة replay الخاصة بـ VRF
    وتصدير تلِمتري EMA. تبقى مقيّدة حتى تهبط تبسيطات التدفق وتكامل lanes/proofs للمرحلة C.
  - `state`: تنظيف `Cell` وتوجيه التلِمتري ينتقلان الى مسار تلِمتري WSV في المرحلة A، بينما
    ملاحظات SoA/parallel-apply تنتقل الى backlog تحسين pipeline في المرحلة C.
  - `ivm`: كشف toggle CUDA والتحقق من envelopes وتغطية Halo2/Metal تتبع عمل حدود المضيف في
    المرحلة D مع محور تسريع GPU العام؛ تبقى kernels في backlog GPU المخصص حتى الجاهزية.
- اعداد RFC عابر للفرق يلخص هذه الخطة من اجل sign-off قبل ادخال تغييرات كود كبيرة.

## 5. اسئلة مفتوحة
- هل تبقى RBC اختيارية بعد P1 ام تصبح الزاميا لخطوط ledger Nexus؟ يحتاج قرار اصحاب المصلحة.
- هل نفرض مجموعات DS composability في P1 ام نبقيها معطلة حتى تنضج proofs الخاصة بالlane؟
- ما الموقع القياسي لمعلمات ML-DSA-87؟ المرشح: crate جديد `crates/fastpq_isi` (قيد الانشاء).

---

_اخر تحديث: 2025-09-12_

</div>
