---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plan-refactor-nexus
título: خطة اعادة هيكلة دفتر Sora Nexus
descripción: Ajuste el ajuste `docs/source/nexus_refactor_plan.md` y ajuste el ajuste Iroha 3.
---

:::nota المصدر القانوني
Utilice el código `docs/source/nexus_refactor_plan.md`. ابق النسختين متوافقتين حتى تصل النسخة متعددة اللغات الى البوابة.
:::

# خطة اعادة هيكلة دفتر Sora Nexus

توثق هذه الوثيقة خارطة الطريق الفورية لاعادة هيكلة Sora Nexus Ledger ("Iroha 3"). تعكس مخطط المستودع الحالي والانتكاسات في محاسبة genesis/WSV واجماع Sumeragi ومشغلات العقود الذكية واستعلامات اللقطات وربط المضيف puntero-ABI وترميزات Norito. الهدف هو الوصول الى معمارية متماسكة قابلة للاختبار دون محاولة ادخال كل الاصلاحات في تصحيح واحد ضخم.

## 0. مبادئ موجهة
- الحفاظ على سلوك حتمي عبر عتاد متنوع؛ Aquí encontrará indicadores de funciones y opciones de respaldo.
- Norito هي طبقة التسلسل. اي تغيير في الحالة/المخطط يجب ان يتضمن اختبارات لترميز وفك ترميز Norito وتحديث y accesorios.
- يمر التكوين عبر `iroha_config` (usuario -> real -> valores predeterminados). Esta opción alterna entre el menú y el menú.
- سياسة ABI تبقى V1 y قابلة للتفاوض. يجب على المضيفين رفض tipos de puntero/llamadas al sistema غير المعروفة بشكل حتمي.
- `cargo test --workspace` y pruebas doradas (`ivm`, `norito`, `integration_tests`) تبقى بوابة الاساس لكل مرحلة.## 1. لقطة من طوبولوجيا المستودع
- `crates/iroha_core`: ممثلو Sumeragi وWSV ومحمل genesis وخطوط الانابيب (consulta, superposición, carriles zk) y مضيف العقود الذكية.
- `crates/iroha_data_model`: المخطط المرجعي للبيانات والاستعلامات على السلسلة.
- `crates/iroha`: Utilice el CLI y el SDK.
- `crates/iroha_cli`: La CLI se utiliza para configurar las API de `iroha`.
- `crates/ivm`: Haga clic en el botón Kotodama y conecte el puntero ABI.
- `crates/norito`: códec compatible con JSON y AoS/NCB.
- `integration_tests`: تأكيدات عبر المكونات تغطي genesis/bootstrap وSumeragi وtriggers وpagination وغيرها.
- الوثائق تشرح اهداف Sora Nexus Ledger (`nexus.md`, `new_pipeline.md`, `ivm.md`), لكن التنفيذ مجزأ ومتهالك جزئيا مقارنة بالشفرة.

## 2. ركائز اعادة الهيكلة والمراحل### المرحلة A - الاساسات والمراقبة
1. **Telemetria WSV + Instantáneas**
   - Actualización de API para `state` (rasgo `WorldStateSnapshot`), conexión de datos, Sumeragi y CLI.
   - استخدام `scripts/iroha_state_dump.sh` لانتاج instantáneas حتمية عبر `iroha state dump --format norito`.
2. **حتمية Génesis/Bootstrap**
   - اعادة هيكلة ادخال genesis ليمر عبر مسار واحد يعتمد Norito (`iroha_core::genesis`).
   - اضافة تغطية تكامل/ارتداد تعيد تشغيل genesis مع اول كتلة وتؤكد تطابق جذور WSV بين arm64/x86_64 (متابعة في `integration_tests/tests/genesis_replay_determinism.rs`).
3. **اختبارات ثبات عبر الحزم**
   - Utilice `integration_tests/tests/genesis_json.rs` para conectar cables WSV, tuberías y ABI.
   - El andamio `cargo xtask check-shape` permite una deriva del esquema (el trabajo pendiente de DevEx está relacionado con `scripts/xtask/README.md`).### المرحلة B - WSV وسطح الاستعلام
1. **معاملات تخزين الحالة**
   - Haga clic en `state/storage_transactions.rs` para confirmar y confirmar.
   - اختبارات الوحدة تتحقق الان من ان تعديلات resources/world/triggers تتراجع عند الفشل.
2. **اعادة هيكلة نموذج الاستعلام**
   - نقل منطق paginación/cursor الى مكونات قابلة لاعادة الاستخدام تحت `crates/iroha_core/src/query/`. Utilice los ajustes Norito y `iroha_data_model`.
   - Realice consultas de instantáneas sobre activadores, activos y roles relacionados con el archivo (متابعة عبر `crates/iroha_core/tests/snapshot_iterable.rs` للتغطية الحالية).
3. **اتساق اللقطات**
   - La CLI `iroha ledger query` está conectada a la interfaz Sumeragi/fetchers.
   - Regresión de funciones mediante CLI mediante `tests/cli/state_snapshot.rs` (controles controlados por funciones).### المرحلة C - خط انابيب Sumeragi
1. **الطوبولوجيا وادارة الحقبات**
   - استخراج `EpochRosterProvider` الى rasgo مع تطبيقات تعتمد على instantáneas de participación en WSV.
   - `WsvEpochRosterAdapter::from_peer_iter` يوفر منشئا بسيطا وداعما للمحاكاة في bancos/pruebas.
2. **تبسيط تدفق الاجماع**
   - Nombre del producto `crates/iroha_core/src/sumeragi/*` y: `pacemaker`, `aggregation`, `availability`, `witness` de otros títulos `consensus`.
   - استبدال تمرير الرسائل العشوائي بأغلفة Norito نوعية وادخال property tests لتغيير العرض (متابعة في backlog رسائل Sumeragi).
3. **تكامل carril/prueba**
   - مواءمة pruebas de carril مع التزامات DA وضمان ان puerta لـ RBC موحد.
   - اختبار تكامل de extremo a extremo `integration_tests/tests/extra_functional/seven_peer_consistency.rs` يتحقق الان من المسار مع تمكين RBC.### المرحلة D - العقود الذكية ومضيفات puntero-ABI
1. **تدقيق حدود المضيف**
   - توحيد فحوصات tipo puntero (`ivm::pointer_abi`) y محولات المضيف (`iroha_core::smartcontracts::ivm::host`).
   - توقعات جدول المؤشرات وربط manifiesto المضيف مغطاة في `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` و`ivm_host_mapping.rs` التي تمارس خرائط TLV golden.
2. **Disparadores de Sandbox**
   - Los activadores del dispositivo `TriggerExecutor` activan la función de encendido y apagado de la computadora y de la computadora.
   - اضافة اختبارات regresión لمشغلات llamada/tiempo تغطي مسارات الفشل (متابعة عبر `crates/iroha_core/tests/trigger_failure.rs`).
3. **محاذاة CLI y العميل**
   - Utilice la CLI (`audit`, `gov`, `sumeragi`, `ivm`) para actualizar y configurar el dispositivo. `iroha` Aquí está el mensaje.
   - Instalar instantáneas JSON mediante CLI mediante `tests/cli/json_snapshot.rs`; Esta aplicación contiene archivos JSON.### Código E - Codec Norito
1. **سجل المخططات**
   - Utilice el controlador Norito y el `crates/norito/src/schema/` para evitar que se produzcan daños.
   - تمت اضافة doc tests تتحقق من ترميز payloads نموذجية (`norito::schema::SamplePayload`).
2. **تحديث accesorios dorados**
   - تحديث accesorios dorados في `crates/norito/tests/*` لتتطابق مع مخطط WSV الجديد عند اكتمال اعادة الهيكلة.
   - `scripts/norito_regen.sh` Esta es una versión JSON dorada que aparece en el archivo `norito_regen_goldens`.
3. **IVM/Norito**
   - El archivo de manifiesto Kotodama incluye el puntero de metadatos ABI.
   - `crates/ivm/tests/manifest_roundtrip.rs` يحافظ على تساوي Norito codifica/decodifica للمانيفستات.## 3. قضايا مشتركة
- **استراتيجية الاختبارات**: كل مرحلة ترقى pruebas unitarias -> pruebas de cajas -> pruebas de integración. الاختبارات الفاشلة تلتقط الانتكاسات الحالية؛ الاختبارات الجديدة تمنع عودتها.
- **التوثيق**: بعد كل مرحلة, حدث `status.md` وادفع العناصر المفتوحة الى `roadmap.md` مع حذف المهام المكتملة.
- **معايير الاداء**: الحفاظ على bancos الحالية في `iroha_core` و`ivm` و`norito`; اضافة قياسات اساس بعد اعادة الهيكلة للتحقق من عدم وجود regresiones.
- **Marcas de características**: الحفاظ على alterna على مستوى caja فقط للواجهات الخلفية التي تتطلب cadenas de herramientas خارجية (`cuda`, `zk-verify-batch`). Configuración SIMD de la CPU, configuración y configuración de la CPU وفر respaldos قياسية حتمية للعتاد غير المدعوم.## 4. الاجراءات الفورية
- Andamio للمرحلة A (rasgo instantáneo + ربط telemetría) - راجع المهام القابلة للتنفيذ في تحديثات hoja de ruta.
- Haga clic en el enlace `sumeragi`, `state` y `ivm` en el siguiente enlace:
  - `sumeragi`: asignaciones de código muerto, cambios de vista, reproducción de VRF y telemetría EMA. تبقى هذه الاجزاء cerrado حتى تتوفر تبسيطات تدفق الاجماع في المرحلة C وتسليمات تكامل carril/prueba.
  - `state`: Actualización `Cell` y telemetría ينتقلان الى مسار telemetría WSV في المرحلة A, بينما تنضم ملاحظات SoA/parallel-apply La cartera de pedidos de la cartera de proyectos de C.
  - `ivm`: Alterna CUDA y sobres y Halo2/Metal para cambiar la configuración de GPU. العرضي؛ Hay kernels y GPU pendientes de acumulación.
- تحضير RFC عبر الفرق يلخص هذه الخطة من اجل sign-off قبل ادخال تغييرات شيفرة واسعة.

## 5. اسئلة مفتوحة
- Para conectar RBC a P1, use el controlador Nexus. يتطلب قرار اصحاب المصلحة.
- هل نفرض مجموعات componibilidad للـ DS في P1 ام نتركها معطلة حتى تنضج pruebas de carril؟
- ما الموقع القانوني لمعاملات ML-DSA-87؟ المرشح: caja جديد `crates/fastpq_isi` (قيد الانشاء).

---

_اخر تحديث: 2025-09-12_