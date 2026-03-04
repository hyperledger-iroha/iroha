---
lang: ar
direction: rtl
source: docs/bindings/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb91ce03aee552c65d15ed1c019da4b3b3db9d48d299b3374ca78b4a8c6c1781
source_last_modified: "2025-11-15T13:38:36.954059+00:00"
translation_last_reviewed: 2026-01-21
---

# حوكمة روابط SDK والـ fixtures

<div dir="rtl">

يشير WP1-E في خارطة الطريق إلى “docs/bindings” باعتباره المكان المعتمد لتتبع حالة
الروابط متعددة اللغات. يوثّق هذا المستند مخزون الروابط وأوامر إعادة التوليد
وحواجز الانحراف ومواقع الأدلة حتى تمتلك بوابات تماثل GPU (WP1-E/F/G) ومجلس
إيقاع SDK مرجعًا واحدًا.

## ضوابط مشتركة
- **الدليل المعتمد:** يوضح `docs/source/norito_binding_regen_playbook.md` سياسة
  التدوير والأدلة المتوقعة ومسار التصعيد لـ Android وSwift وPython والروابط
  المستقبلية.
- **تماثل مخطط Norito:** `scripts/check_norito_bindings_sync.py` (يُستدعى عبر
  `scripts/check_norito_bindings_sync.sh` ومقيّد في CI بواسطة
  `ci/check_norito_bindings_sync.sh`) يوقف البناء عندما تنحرف مخططات Rust أو Java
  أو Python.
- **مراقب الإيقاع:** يقرأ `scripts/check_fixture_cadence.py` ملفات
  `artifacts/*_fixture_regen_state.json` ويطبّق نوافذ الثلاثاء/الجمعة (Android,
  Python) والأربعاء (Swift) حتى تمتلك بوابات خارطة الطريق طوابع زمنية قابلة
  للتدقيق.

## مصفوفة الروابط

| الرابط | نقاط الدخول | أمر fixtures/إعادة التوليد | حواجز الانحراف | الأدلة |
|--------|-------------|-----------------------------|----------------|--------|
| Android (Java) | `java/iroha_android/` (`java/iroha_android/README.md`) | `scripts/android_fixture_regen.sh` → `artifacts/android_fixture_regen_state.json` | `scripts/check_android_fixtures.py`, `ci/check_android_fixtures.sh`, `java/iroha_android/run_tests.sh` | `artifacts/android/fixture_runs/` |
| Swift (iOS/macOS) | `IrohaSwift/` (`IrohaSwift/README.md`) | `scripts/swift_fixture_regen.sh` (اختياريًا `SWIFT_FIXTURE_ARCHIVE`) → `artifacts/swift_fixture_regen_state.json` | `scripts/check_swift_fixtures.py`, `ci/check_swift_fixtures.sh`, `scripts/swift_fixture_archive.py` | `docs/source/swift_parity_triage.md`, `docs/source/sdk/swift/ios2_fixture_cadence_brief.md` |
| Python | `python/iroha_python/` (`python/iroha_python/README.md`) | `scripts/python_fixture_regen.sh` → `artifacts/python_fixture_regen_state.json` | `scripts/check_python_fixtures.py`, `python/iroha_python/scripts/run_checks.sh` | `docs/source/norito_binding_regen_playbook.md`, `docs/source/sdk/python/connect_end_to_end.md` |
| JavaScript | `javascript/iroha_js/` (`docs/source/sdk/js/publishing.md`) | `npm run release:provenance`, `scripts/js_sbom_provenance.sh`, `scripts/js_signed_staging.sh` | `npm run test`, `javascript/iroha_js/scripts/verify-release-tarball.mjs`, `javascript/iroha_js/scripts/record-release-provenance.mjs` | `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`, `artifacts/js/verification/`, `artifacts/js/sbom/` |

## تفاصيل الروابط

### Android (Java)
يوجد SDK الخاص بـ Android في `java/iroha_android/` ويستهلك fixtures Norito
المعتمدة التي ينتجها `scripts/android_fixture_regen.sh`. يقوم هذا المساعد
بتصدير كتل `.norito` جديدة من أدوات Rust، ويحدّث
`artifacts/android_fixture_regen_state.json` ويسجّل بيانات الإيقاع التي تستهلكها
`scripts/check_fixture_cadence.py` ولوحات الحوكمة. يتم اكتشاف الانحراف عبر
`scripts/check_android_fixtures.py` (والموصل بـ `ci/check_android_fixtures.sh`)
وبواسطة `java/iroha_android/run_tests.sh` الذي يختبر روابط JNI وإعادة تشغيل
طوابير WorkManager وحالات StrongBox البديلة. تُخزن أدلة التدوير وملاحظات الفشل
ونصوص إعادة التشغيل تحت `artifacts/android/fixture_runs/`.

### Swift (macOS/iOS)
يعكس `IrohaSwift/` نفس حمولات Norito عبر `scripts/swift_fixture_regen.sh`. يسجل
السكربت مالك التدوير ووسم الإيقاع والمصدر (`live` مقابل `archive`) داخل
`artifacts/swift_fixture_regen_state.json` ويغذي مدقق الإيقاع. يتيح
`scripts/swift_fixture_archive.py` للمتابعين استيراد أرشيفات ناتجة عن Rust؛
يفرض `scripts/check_swift_fixtures.py` و `ci/check_swift_fixtures.sh` تماثلًا
بمستوى البايت وحدود العمر وفق SLA، بينما يدعم
`scripts/swift_fixture_regen.sh` الخيار `SWIFT_FIXTURE_EVENT_TRIGGER` للتدوير
اليدوي. توثّق خطة التصعيد ومؤشرات الأداء ولوحات المراقبة في
`docs/source/swift_parity_triage.md` وملخصات الإيقاع ضمن `docs/source/sdk/swift/`.

### Python
يشترك عميل Python (`python/iroha_python/`) في fixtures Android. تشغيل
`scripts/python_fixture_regen.sh` يجلب أحدث حمولات `.norito`، ويحدّث
`python/iroha_python/tests/fixtures/`، وسيصدر بيانات إيقاع في
`artifacts/python_fixture_regen_state.json` بعد أول تدوير بعد خارطة الطريق.
يقوم `scripts/check_python_fixtures.py` و`python/iroha_python/scripts/run_checks.sh`
بمنع pytest وmypy وruff وتماثل fixtures محليًا وفي CI. توضح وثائق end-to-end
(`docs/source/sdk/python/…`) ودليل إعادة التوليد كيفية تنسيق التدوير مع مالكي
Android.

### JavaScript
لا يعتمد `javascript/iroha_js/` على ملفات `.norito` محلية، لكن WP1-E يتابع أدلة
الإصدار حتى ترث مسارات CI الخاصة بـ GPU provenance كاملة. كل إصدار يلتقط
provenance عبر `npm run release:provenance` (بدعم
`javascript/iroha_js/scripts/record-release-provenance.mjs`)، وينشئ ويوقّع حزم
SBOM عبر `scripts/js_sbom_provenance.sh`، ويشغّل التجهيز الموقّع
(`scripts/js_signed_staging.sh`)، ويتحقق من عنصر السجل عبر
`javascript/iroha_js/scripts/verify-release-tarball.mjs`. تنتهي البيانات الوصفية
الناتجة في `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`,
`artifacts/js/sbom/`, و`artifacts/js/verification/`, لتوفير أدلة حتمية لتشغيل
خارطة الطريق JS5/JS6 و WP1-F. يربط دليل النشر في `docs/source/sdk/js/` الأتمتة
بأكملها.

</div>
