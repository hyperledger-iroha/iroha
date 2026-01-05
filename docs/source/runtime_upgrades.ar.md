---
lang: ar
direction: rtl
source: docs/source/runtime_upgrades.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8990e19977e7f3fb370b9c8f66064542135fa171
source_last_modified: "2025-12-04T06:31:08.260928+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/runtime_upgrades.md -->

# ترقيات Runtime (IVM + Host) — بدون توقف، بدون Hardfork

يحدد هذا المستند الية حتمية تتحكم بها الحوكمة لإضافة قدرات IVM/host جديدة (مثل syscalls جديدة
وانواع pointer-ABI) دون ايقاف الشبكة او عمل hardfork للعقد. تقوم العقد بنشر الثنائيات مسبقا؛
ويتم تنسيق التفعيل على السلسلة ضمن نافذة ارتفاع محدودة. العقود القديمة تستمر دون تغيير؛ اما
القدرات الجديدة فمقيدة حسب نسخة ABI والسياسة.

ملاحظة (الاصدار الاول): يتم دعم ABI v1 فقط. يتم رفض manifests الخاصة بترقية runtime لنسخ ABI الاخرى حتى اصدار مستقبلي يضيف ABI جديدة.

الاهداف
- تفعيل حتمي ضمن نافذة ارتفاع مجدولة مع تطبيق idempotent.
- تعايش عدة نسخ ABI بدون كسر الثنائيات الموجودة.
- حواجز قبول وتنفيذ تمنع payloads قبل التفعيل من تمكين سلوك جديد.
- طرح ملائم للمشغلين مع رؤية للقدرات وانماط فشل واضحة.

غير الاهداف
- تغيير ارقام syscalls الحالية او معرفات انواع المؤشرات (محظور).
- ترقيع العقد مباشرة دون نشر ثنائيات محدثة.

التعريفات
- نسخة ABI: عدد صغير يعلن في `ProgramMetadata.abi_version` ويختار `SyscallPolicy` وقائمة allowlist لانواع المؤشرات.
- ABI Hash: digest حتمي لسطح ABI لنسخة معينة: قائمة syscalls (ارقام+اشكال)، معرفات/allowlist لانواع المؤشرات ورايات السياسة؛ يحسب بواسطة `ivm::syscalls::compute_abi_hash`.
- Syscall Policy: تعيين في المضيف يقرر ما اذا كان رقم syscall مسموحا لنسخة ABI وسياسة مضيف محددتين.
- Activation Window: فترة ارتفاع كتلة نصف مفتوحة `[start, end)` حيث يكون التفعيل صالحا مرة واحدة عند `start`.

كائنات الحالة (نموذج البيانات)
<!-- BEGIN RUNTIME UPGRADE TYPES -->
- `RuntimeUpgradeId`: Blake2b-256 لبايتات Norito الكانونية للـ manifest.
- حقول `RuntimeUpgradeManifest`:
  - `name: String` — تسمية مقروءة.
  - `description: String` — وصف قصير للمشغلين.
  - `abi_version: u16` — نسخة ABI المستهدفة للتفعيل.
  - `abi_hash: [u8; 32]` — hash ABI الكانوني للسياسة المستهدفة.
  - `added_syscalls: Vec<u16>` — ارقام syscalls التي تصبح صالحة مع هذه النسخة.
  - `added_pointer_types: Vec<u16>` — معرفات انواع المؤشرات المضافة بالترقية.
  - `start_height: u64` — اول ارتفاع كتلة يسمح فيه بالتفعيل.
  - `end_height: u64` — الحد الاعلى الحصري لنافذة التفعيل.
  - `sbom_digests: Vec<RuntimeUpgradeSbomDigest>` — digests SBOM لادوات الترقية.
  - `slsa_attestation: Vec<u8>` — بايتات SLSA attestation الخام (base64 في JSON).
  - `provenance: Vec<ManifestProvenance>` — تواقيع على الـ payload الكانوني.
- حقول `RuntimeUpgradeRecord`:
  - `manifest: RuntimeUpgradeManifest` — payload الاقتراح الكانوني.
  - `status: RuntimeUpgradeStatus` — حالة دورة حياة الاقتراح.
  - `proposer: AccountId` — الجهة التي قدمت الاقتراح.
  - `created_height: u64` — ارتفاع الكتلة الذي دخل فيه الاقتراح الى الدفتر.
- حقول `RuntimeUpgradeSbomDigest`:
  - `algorithm: String` — معرف خوارزمية digest.
  - `digest: Vec<u8>` — بايتات digest الخام (base64 في JSON).
<!-- END RUNTIME UPGRADE TYPES -->
  - الثوابت: `end_height > start_height`; `abi_version` اكبر بشكل صارم من اي نسخة فعالة; `abi_hash` يجب ان يساوي `ivm::syscalls::compute_abi_hash(policy_for(abi_version))`; `added_*` يجب ان تسرد بالضبط الفرق الاضافي بين سياسة ABI الجديدة والسابقة الفعالة; لا يجوز حذف او اعادة ترقيم الارقام/المعرفات الموجودة.

تخطيط التخزين
- `world.runtime_upgrades`: خريطة MVCC بمفتاح `RuntimeUpgradeId.0` (hash خام بطول 32 بايت) مع قيم مرمزة كـ Norito canonical `RuntimeUpgradeRecord`. السجلات تبقى عبر الكتل؛ وعمليات commit idempotent وآمنة من اعادة التشغيل.

التعليمات (ISI)
- ProposeRuntimeUpgrade { manifest: RuntimeUpgradeManifest }
  - التأثيرات: ادراج `RuntimeUpgradeRecord { status: Proposed }` بمفتاح `RuntimeUpgradeId` اذا لم يوجد.
  - يرفض اذا تداخلت النافذة مع سجل Proposed/Activated اخر او اذا فشلت الثوابت.
  - idempotent: اعادة ارسال نفس البايتات الكانونية للـ manifest لا تغير شيئا.
  - الترميز الكانوني: بايتات الـ manifest يجب ان تطابق `RuntimeUpgradeManifest::canonical_bytes()`؛ الترميزات غير الكانونية مرفوضة.
- ActivateRuntimeUpgrade { id: RuntimeUpgradeId }
  - الشروط المسبقة: سجل Proposed مطابق موجود; `current_height` يجب ان يساوي `manifest.start_height`; `current_height < manifest.end_height`.
  - التأثيرات: تحويل السجل الى `ActivatedAt(current_height)`؛ اضافة `abi_version` الى مجموعة ABI الفعالة.
  - idempotent: اعادة التنفيذ في نفس الارتفاع لا اثر له؛ ارتفاعات اخرى ترفض بشكل حتمي.
- CancelRuntimeUpgrade { id: RuntimeUpgradeId }
  - الشروط المسبقة: الحالة Proposed و `current_height < manifest.start_height`.
  - التأثيرات: تحويل الى `Canceled`.

الاحداث (Data Events)
- RuntimeUpgradeEvent::{Proposed { id, manifest }, Activated { id, abi_version, at_height }, Canceled { id }}

قواعد القبول
- قبول العقود: في الاصدار الاول يقبل فقط `ProgramMetadata.abi_version = 1`؛ القيم الاخرى ترفض مع `IvmAdmissionError::UnsupportedAbiVersion`.
  - لــ ABI v1، يعاد حساب `abi_hash(1)` ويشترط التطابق مع payload/manifest عند توفره؛ والا يرفض بـ `IvmAdmissionError::ManifestAbiHashMismatch`.
- قبول المعاملات: التعليمات `ProposeRuntimeUpgrade`/`ActivateRuntimeUpgrade`/`CancelRuntimeUpgrade` تتطلب صلاحيات مناسبة (root/sudo); يجب ان تلتزم بقيود تداخل النوافذ.

تطبيق provenance
- يمكن لـ manifests الخاصة بالترقية ان تحمل digests SBOM (`sbom_digests`)، وبايتات SLSA attestation (`slsa_attestation`)، وبيانات الموقعين (تواقيع `provenance`). التواقيع تغطي `RuntimeUpgradeManifestSignaturePayload` الكانوني (كل حقول الـ manifest باستثناء قائمة تواقيع `provenance`).
- تهيئة الحوكمة تتحكم بالتطبيق تحت `governance.runtime_upgrade_provenance`:
  - `mode`: `optional` (تقبل غياب provenance وتتحقق اذا كانت موجودة) او `required` (ترفض اذا غابت provenance).
  - `require_sbom`: عندما `true`، يجب توفير digest SBOM واحد على الاقل.
  - `require_slsa`: عندما `true`، يجب توفير SLSA attestation غير فارغ.
  - `trusted_signers`: قائمة مفاتيح عامة للموقعين الموثوقين.
  - `signature_threshold`: الحد الادنى لعدد التواقيع الموثوقة المطلوبة.
- رفضات provenance تظهر كرموز اخطاء مستقرة في فشل التعليمات (بادئة `runtime_upgrade_provenance:`):
  - `missing_provenance`, `missing_sbom`, `invalid_sbom_digest`, `missing_slsa_attestation`
  - `missing_signatures`, `invalid_signature`, `untrusted_signer`, `signature_threshold_not_met`
- Telemetry: `runtime_upgrade_provenance_rejections_total{reason}` يحصي اسباب الرفض.

قواعد التنفيذ
- سياسة مضيف VM: اثناء تنفيذ البرنامج، اشتق `SyscallPolicy` من `ProgramMetadata.abi_version`. syscalls غير المعروفة لتلك النسخة تتحول الى `VMError::UnknownSyscall`.
- Pointer-ABI: allowlist مشتقة من `ProgramMetadata.abi_version`; الانواع خارج allowlist لهذه النسخة ترفض اثناء decode/validation.
- تبديل المضيف: كل كتلة تعيد حساب مجموعة ABI الفعالة؛ عند commit تفعيل، المعاملات اللاحقة في نفس الكتلة ترى السياسة الجديدة (موثق باختبار `runtime_upgrade_admission::activation_allows_new_abi_in_same_block`).
  - ربط سياسة syscalls: `CoreHost` يقرأ نسخة ABI المعلنة بالمعاملة ويطبق `ivm::syscalls::is_syscall_allowed`/`is_type_allowed_for_policy` مقابل `SyscallPolicy` لكل كتلة. المضيف يعيد استخدام مثيل VM المرتبط بالمعاملة، لذا التفعيل في منتصف الكتلة امن - المعاملات اللاحقة ترى السياسة المحدثة بينما السابقة تستمر بنسختها الاصلية.

ثوابت الحتمية والسلامة
- التفعيل يحدث فقط عند `start_height` وهو idempotent؛ اعادة التنظيمات تحت `start_height` تعيد التطبيق بشكل حتمي عند عودة الكتلة.
- نسخ ABI الحالية تبقى فعالة بلا نهاية؛ النسخ الجديدة توسع المجموعة الفعالة فقط.
- لا تفاوض ديناميكي يؤثر على الاجماع او ترتيب التنفيذ؛ نشر القدرات معلوماتي فقط.

طرح المشغل (بدون توقف)
1) نشر ثنائية عقدة تدعم نسخة ABI الجديدة (`v+1`) دون تفعيلها.
2) مراقبة قدرة الاسطول عبر telemetry (نسبة العقد التي تعلن دعم `v+1`).
3) ارسال `ProposeRuntimeUpgrade` بنافذة متقدمة كفاية (مثلا `H+N`).
4) عند `start_height`، تنفذ `ActivateRuntimeUpgrade` تلقائيا كجزء من الكتلة وتبدل المجموعة الفعالة للمضيف؛ العقد التي لم تتحدث ستواصل تشغيل العقود القديمة لكنها سترفض قبول/تنفيذ برامج `v+1`.
5) بعد التفعيل، اعد تجميع/نشر عقود تستهدف `v+1`.

Torii و CLI
- Torii
  - `GET /v1/runtime/abi/active` -> `{ active_versions: [u16], default_compile_target: u16 }` (implemented)
  - `GET /v1/runtime/abi/hash` -> `{ policy: "V1", abi_hash_hex: "<64-hex>" }` (implemented)
  - `GET /v1/runtime/upgrades` -> قائمة السجلات (implemented).
  - `POST /v1/runtime/upgrades/propose` -> يغلف `ProposeRuntimeUpgrade` (يعيد هيكل تعليمات; implemented).
  - `POST /v1/runtime/upgrades/activate/:id` -> يغلف `ActivateRuntimeUpgrade` (يعيد هيكل تعليمات; implemented).
  - `POST /v1/runtime/upgrades/cancel/:id` -> يغلف `CancelRuntimeUpgrade` (يعيد هيكل تعليمات; implemented).
- CLI
  - `iroha runtime abi active` (implemented)
  - `iroha runtime abi hash` (implemented)
  - `iroha runtime upgrade list` (implemented)
  - `iroha runtime upgrade propose --file <manifest.json>` (implemented)
  - `iroha runtime upgrade activate --id <id>` (implemented)
  - `iroha runtime upgrade cancel --id <id>` (implemented)

Core Query API
- استعلام Norito مفرد (موقع):
  - `FindActiveAbiVersions` يعيد بنية Norito `{ active_versions: [u16], default_compile_target: u16 }`.
  - راجع المثال: `docs/source/samples/find_active_abi_versions.md` (النوع/الحقول ومثال JSON).

تغييرات مطلوبة في الكود (حسب crate)
- iroha_data_model
  - اضافة `RuntimeUpgradeManifest` و `RuntimeUpgradeRecord` و enums للتعليمات والاحداث و codecs JSON/Norito مع اختبارات roundtrip.
- iroha_core
  - WSV: اضافة سجل `runtime_upgrades` مع فحوصات التداخل و getters.
  - Executors: تنفيذ معالجات ISI؛ بث الاحداث؛ تطبيق قواعد القبول.
  - Admission: بوابة manifests البرامج عبر نشاط `abi_version` ومطابقة `abi_hash`.
  - ربط سياسة syscalls: تمرير مجموعة ABI الفعالة الى منشئ VM host؛ ضمان الحتمية باستخدام ارتفاع الكتلة عند بداية التنفيذ.
  - Tests: idempotency لنافذة التفعيل، رفضات التداخل، سلوك القبول قبل/بعد.
- ivm
  - تعريف `ABI_V2` (مثال) مع سياسة: توسيع `abi_syscall_list()`؛ mapping `is_syscall_allowed(policy, number)`؛ توسيع سياسة انواع المؤشرات.
  - اعادة حساب وتثبيت اختبارات golden: `abi_syscall_list_golden.rs`, `abi_hash_versions.rs`, `pointer_type_ids_golden.rs`.
- iroha_cli / iroha_torii
  - اضافة endpoints والاوامر المذكورة اعلاه؛ helpers Norito JSON للـ manifests؛ اختبارات تكامل اساسية.
- Kotodama compiler
  - السماح بالاستهداف `abi_version = v+1`؛ تضمين `abi_hash` الصحيح للنسخة المختارة داخل manifests `.to`.

Telemetry
- اضافة gauge `runtime.active_abi_versions` و counter `runtime.upgrade_events_total{kind}`.

اعتبارات الامان
- فقط root/sudo يمكنه الاقتراح/التفعيل/الالغاء؛ يجب توقيع manifests بشكل صحيح.
- نوافذ التفعيل تمنع front-running وتضمن تطبيقا حتميا.
- `abi_hash` يثبت سطح الواجهة لمنع drift صامت بين الثنائيات.

معايير القبول (Conformance)
- قبل التفعيل، العقد ترفض بشكل حتمي الكود مع `abi_version = v+1`.
- بعد التفعيل عند `start_height`، العقد تقبل وتنفذ `v+1`; البرامج القديمة تستمر دون تغيير.
- اختبارات golden لــ ABI hashes وقوائم syscalls تمر على x86-64/ARM64.
- التفعيل idempotent وآمن تحت reorgs.

</div>
