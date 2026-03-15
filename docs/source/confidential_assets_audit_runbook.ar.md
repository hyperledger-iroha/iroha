---
lang: ar
direction: rtl
source: docs/source/confidential_assets_audit_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8691a94d23e589f46d8e8cf2359d6d9a31f7c38c5b7bf0def69c88d2dd081765
source_last_modified: "2026-01-22T15:38:30.658489+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! دليل عمليات تدقيق الأصول السرية والعمليات المشار إليه بواسطة `roadmap.md:M4`.

# دليل تدقيق الأصول والعمليات السرية

يعمل هذا الدليل على توحيد الأدلة التي يعتمد عليها المدققون والمشغلون
عند التحقق من تدفقات الأصول السرية. وهو يكمل قواعد اللعب التناوب
(`docs/source/confidential_assets_rotation.md`) ودفتر الأستاذ المعايرة
(`docs/source/confidential_assets_calibration.md`).

## 1. الإفصاح الانتقائي وخلاصات الأحداث

- تصدر كل تعليمات سرية حمولة منظمة `ConfidentialEvent`
  (`Shielded`، `Transferred`، `Unshielded`) تم التقاطها في
  `crates/iroha_data_model/src/events/data/events.rs:198` وتسلسلها بواسطة
  المنفذون (`crates/iroha_core/src/smartcontracts/isi/world.rs:3699` –`4021`).
  تمارس مجموعة الانحدار الحمولات الملموسة حتى يتمكن المدققون من الاعتماد عليها
  تخطيطات JSON الحتمية (`crates/iroha_core/tests/zk_confidential_events.rs:19` – `299`).
- يعرض Torii هذه الأحداث عبر مسار SSE/WebSocket القياسي؛ مراجعي الحسابات
  الاشتراك باستخدام `ConfidentialEventFilter` (`crates/iroha_data_model/src/events/data/filters.rs:82`)،
  تحديد النطاق بشكل اختياري لتعريف أصل واحد. مثال سطر الأوامر:

  ```bash
  iroha ledger events data watch --filter '{ "confidential": { "asset_definition_id": "rose#wonderland" } }'
  ```

- بيانات تعريف السياسة والانتقالات المعلقة متاحة من خلال
  `GET /v2/confidential/assets/{definition_id}/transitions`
  (`crates/iroha_torii/src/routing.rs:15205`)، معكوسة بواسطة Swift SDK
  (`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:3245`) وموثقة في
  كل من تصميم الأصول السرية وأدلة SDK
  (`docs/source/confidential_assets.md:70`، `docs/source/sdk/swift/index.md:334`).

## 2. القياس عن بعد ولوحات المعلومات وأدلة المعايرة

- مقاييس وقت التشغيل لعمق الشجرة، وتاريخ الالتزام/الحدود، وإخلاء الجذر
  العدادات، ونسب نتائج التحقق من ذاكرة التخزين المؤقت
  (`crates/iroha_telemetry/src/metrics.rs:5760` –`5815`). لوحات المعلومات Grafana في
  يقوم `dashboards/grafana/confidential_assets.json` بشحن اللوحات و
  التنبيهات، مع سير العمل الموثق في `docs/source/confidential_assets.md:401`.
- عمليات المعايرة (NS/op، Gas/op، ns/gas) مع وجود سجلات موقعة مباشرة
  `docs/source/confidential_assets_calibration.md`. أحدث سيليكون أبل
  تم أرشفة تشغيل NEON في
  `docs/source/confidential_assets_calibration_neon_20260428.log`، ونفس الشيء
  يسجل دفتر الأستاذ التنازلات المؤقتة لملفات تعريف SIMD المحايدة وAVX2 حتى
  يأتي مضيفو x86 عبر الإنترنت.

## 3. الاستجابة للحوادث ومهام المشغل

- إجراءات التناوب/الترقية موجودة
  `docs/source/confidential_assets_rotation.md`، يغطي كيفية العرض الجديد
  حزم المعلمات، وجدولة ترقيات السياسة، وإخطار المحافظ/المراجعين. ال
  قوائم التعقب (`docs/source/project_tracker/confidential_assets_phase_c.md`).
  أصحاب كتاب التشغيل وتوقعات التدريب.
- بالنسبة لبروفات الإنتاج أو نوافذ الطوارئ، يرفق المشغلون الأدلة بها
  إدخالات `status.md` (على سبيل المثال، سجل التدريب متعدد المسارات) وتتضمن:
  إثبات `curl` لانتقالات السياسة ولقطات Grafana والحدث ذي الصلة
  ملخصات حتى يتمكن المدققون من إعادة بناء الجداول الزمنية للنعناع → النقل → الكشف.

## 4. إيقاع المراجعة الخارجية

- نطاق المراجعة الأمنية: الدوائر السرية، وسجلات المعلمات، والسياسة
  التحولات والقياس عن بعد. هذه الوثيقة بالإضافة إلى نماذج دفتر الأستاذ المعايرة
  وحزمة الأدلة المرسلة إلى البائعين؛ يتم تتبع جدولة المراجعة عبر
  M4 في `docs/source/project_tracker/confidential_assets_phase_c.md`.
- يجب على المشغلين إبقاء `status.md` محدثًا بأي نتائج أو متابعة خاصة بالبائع
  عناصر العمل. وإلى أن تكتمل المراجعة الخارجية، يكون دليل التشغيل هذا بمثابة
  يمكن لمدققي خط الأساس التشغيلي اختبارها.