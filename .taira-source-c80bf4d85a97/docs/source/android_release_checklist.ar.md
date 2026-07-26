---
lang: ar
direction: rtl
source: docs/source/android_release_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5ee3613b544a847953f5ec152092cb2fe1da35279c5482486513d6b8d6dddf02
source_last_modified: "2026-01-04T11:42:43.398592+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# قائمة التحقق لإصدار Android (AND6)

تلتقط قائمة المراجعة هذه بوابات **AND6 — CI وتقوية الامتثال** من
`roadmap.md` (§الأولوية 5). فهو يعمل على محاذاة إصدارات Android SDK مع Rust
قم بإصدار توقعات RFC من خلال توضيح وظائف CI، وعناصر الامتثال،
أدلة مختبر الأجهزة، وحزم المصدر التي يجب إرفاقها قبل GA،
LTS، أو قطار الإصلاح العاجل يتحرك للأمام.

استخدم هذا المستند مع:

- `docs/source/android_support_playbook.md` - تقويم الإصدار، واتفاقيات مستوى الخدمة، و
  شجرة التصعيد
- `docs/source/android_runbook.md` - دفاتر التشغيل اليومية.
- `docs/source/compliance/android/and6_compliance_checklist.md` — المنظم
  جرد قطعة أثرية.
- `docs/source/release_dual_track_runbook.md` - إدارة الإصدار ثنائي المسار.

## 1. لمحة سريعة عن بوابات المسرح

| المرحلة | البوابات المطلوبة | الأدلة |
|-------|----------------|---------|
| **T−7 أيام (ما قبل التجميد)** | ليلاً `ci/run_android_tests.sh` باللون الأخضر لمدة 14 يومًا؛ تمرير `ci/check_android_fixtures.sh` و`ci/check_android_samples.sh` و`ci/check_android_docs_i18n.sh`؛ تم وضع عمليات فحص الوبر/التبعية في قائمة الانتظار. | لوحات معلومات Buildkite، وتقرير فرق التركيبات، وعينة من لقطات الشاشة. |
| **T−3 أيام (ترويج RC)** | تم تأكيد حجز معمل الأجهزة؛ تشغيل CI لشهادة StrongBox (`scripts/android_strongbox_attestation_ci.sh`)؛ يتم ممارسة الأجنحة الكهربائية/الروبوتية على الأجهزة المجدولة؛ `./gradlew lintRelease ktlintCheck detekt dependencyGuard` نظيف. | مصفوفة الجهاز CSV، بيان حزمة التصديق، تقارير Gradle المؤرشفة تحت `artifacts/android/lint/<version>/`. |
| **T−1 يوم (ذهاب/عدم الانطلاق)** | تم تحديث حزمة حالة تنقيح القياس عن بعد (`scripts/telemetry/check_redaction_status.py --write-cache`)؛ تم تحديث عناصر الامتثال وفقًا لـ `and6_compliance_checklist.md`؛ اكتملت بروفة المصدر (`scripts/android_sbom_provenance.sh --dry-run`). | `docs/source/compliance/android/evidence_log.csv`، حالة القياس عن بعد JSON، سجل التشغيل الجاف للمصدر. |
| **T0 (تحويل GA/LTS)** | اكتمل `scripts/publish_android_sdk.sh --dry-run`؛ المصدر + توقيع SBOM؛ تم تصدير قائمة مراجعة الإصدار وإرفاقها بدقائق الذهاب/عدم الذهاب؛ `ci/sdk_sorafs_orchestrator.sh` وظيفة الدخان باللون الأخضر. | قم بتحرير مرفقات RFC، وحزمة Sigstore، وعناصر الاعتماد تحت `artifacts/android/`. |
| **T+1 يوم (ما بعد التحويل)** | تم التحقق من جاهزية الإصلاح العاجل (`scripts/publish_android_sdk.sh --validate-bundle`)؛ تمت مراجعة فروق لوحة المعلومات (`ci/check_android_dashboard_parity.sh`)؛ تم تحميل حزمة الأدلة إلى `status.md`. | تصدير فرق لوحة المعلومات، الارتباط بإدخال `status.md`، حزمة الإصدار المؤرشفة. |

## 2. مصفوفة بوابة CI والجودة| بوابة | الأوامر (الأوامر) / البرنامج النصي | ملاحظات |
|------|--------------------|-------|
| الوحدة + اختبارات التكامل | `ci/run_android_tests.sh` (يلتف `ci/run_android_tests.sh`) | يصدر `artifacts/android/tests/test-summary.json` + سجل الاختبار. يتضمن برنامج الترميز Norito وقائمة الانتظار والرجوع StrongBox واختبارات تسخير العميل Torii. مطلوب ليلا وقبل وضع العلامات. |
| التكافؤ لاعبا اساسيا | `ci/check_android_fixtures.sh` (يلتف `scripts/check_android_fixtures.py`) | يضمن تطابق تركيبات Norito المُعاد إنشاؤها مع مجموعة Rust الأساسية؛ قم بإرفاق فرق JSON عند فشل البوابة. |
| تطبيقات عينة | `ci/check_android_samples.sh` | ينشئ `examples/android/{operator-console,retail-wallet}` ويتحقق من صحة لقطات الشاشة المترجمة عبر `scripts/android_sample_localization.py`. |
| دوكس/I18N | `ci/check_android_docs_i18n.sh` | ملف Guards README + البدء السريع المحلي. قم بالتشغيل مرة أخرى بعد وصول تعديلات المستند إلى فرع الإصدار. |
| تكافؤ لوحة المعلومات | `ci/check_android_dashboard_parity.sh` | يؤكد تطابق مقاييس CI/المصدرة مع نظيراتها من Rust؛ مطلوب أثناء التحقق T+1. |
| اعتماد SDK الدخان | `ci/sdk_sorafs_orchestrator.sh` | يتدرب على روابط منسق Sorafs متعددة المصادر مع SDK الحالي. مطلوب قبل تحميل المصنوعات اليدوية. |
| التحقق من صحة | `scripts/android_strongbox_attestation_ci.sh --summary-out artifacts/android/attestation/ci-summary.json` | تجميع حزم شهادة StrongBox/TEE ضمن `artifacts/android/attestation/**`؛ قم بإرفاق الملخص بحزم GA. |
| التحقق من صحة فتحة مختبر الجهاز | `scripts/check_android_device_lab_slot.py --root artifacts/android/device_lab/<slot> --json-out artifacts/android/device_lab/summary.json` | التحقق من صحة حزم الأجهزة قبل إرفاق الأدلة لإصدار الحزم؛ يعمل CI مقابل فتحة العينة في `fixtures/android/device_lab/slot-sample` (القياس عن بعد/الشهادة/قائمة الانتظار/السجلات + `sha256sum.txt`). |

> **نصيحة:** أضف هذه المهام إلى مسار `android-release` Buildkite بحيث
> أسابيع التجميد، قم بإعادة تشغيل كل بوابة تلقائيًا باستخدام طرف فرع التحرير.

تعمل مهمة `.github/workflows/android-and6.yml` المدمجة على تشغيل الوبر،
مجموعة الاختبار، وملخص التصديق، وفحوصات فتحة معمل الجهاز في كل PR/دفعة
لمس مصادر Android، وتحميل الأدلة تحت `artifacts/android/{lint,tests,attestation,device_lab}/`.

## 3. فحص الوبر والتبعية

قم بتشغيل `scripts/android_lint_checks.sh --version <semver>` من جذر الريبو. ال
تنفيذ البرنامج النصي:

```
lintRelease ktlintCheck detekt dependencyGuardBaseline \
:operator-console:lintRelease :retail-wallet:lintRelease
```

- يتم أرشفة التقارير ومخرجات حماية التبعية ضمن
  `artifacts/android/lint/<label>/` والارتباط الرمزي `latest/` للإصدار
  خطوط الأنابيب.
- تتطلب نتائج الوبر الفاشلة إما المعالجة أو الإدخال في الإصدار
  RFC يوثق المخاطر المقبولة (معتمدة من Release Engineering + Program
  الرصاص).
- يقوم `dependencyGuardBaseline` بإعادة إنشاء قفل التبعية؛ إرفاق الفرق
  إلى حزمة الذهاب/عدم الذهاب.

## 4. تغطية مختبر الأجهزة والصندوق القوي

1. قم بحجز أجهزة Pixel + Galaxy باستخدام أداة تعقب السعة المشار إليها في
   `docs/source/compliance/android/device_lab_contingency.md`. إصدارات الكتل
   إذا كان التوفر أقل من 70%.
2. قم بتنفيذ scripts/android_strongbox_attestation_ci.sh --report \
   artifacts/android/attestation/` لتحديث تقرير التصديق.
3. قم بتشغيل مصفوفة الأجهزة (توثيق قائمة المجموعة/ABI في الجهاز
   تعقب). قم بالتقاط حالات الفشل في سجل الحوادث حتى لو نجحت عمليات إعادة المحاولة.
4. قم بتقديم تذكرة إذا كان الرجوع إلى Firebase Test Lab مطلوبًا؛ ربط التذكرة
   في القائمة المرجعية أدناه.

## 5. الامتثال والقياس عن بعد- اتبع `docs/source/compliance/android/and6_compliance_checklist.md` للاتحاد الأوروبي
  وتقديمات JP. تحديث `docs/source/compliance/android/evidence_log.csv`
  مع التجزئة + عناوين URL لوظيفة Buildkite.
- تحديث أدلة تنقيح القياس عن بعد عبر
  `scripts/telemetry/check_redaction_status.py --write-cache \
   --status-url https://android-observability.example/status.json`.
  قم بتخزين JSON الناتج تحت
  `artifacts/android/telemetry/<version>/status.json`.
- سجل مخرجات فرق المخطط من
  `scripts/telemetry/run_schema_diff.sh --android-config ... --rust-config ...`
  لإثبات التكافؤ مع مصدري الصدأ.

## 6. المصدر، SBOM، والنشر

1. التشغيل الجاف لخط أنابيب النشر:

   ```bash
   scripts/publish_android_sdk.sh \
     --version <semver> \
     --repo-dir artifacts/android/maven/<semver> \
     --dry-run
   ```

2. إنشاء مصدر SBOM + Sigstore:

   ```bash
   scripts/android_sbom_provenance.sh \
     --version <semver> \
     --out artifacts/android/provenance/<semver>
   ```

3. قم بإرفاق `artifacts/android/provenance/<semver>/manifest.json` وتوقيعه
   `checksums.sha256` إلى إصدار RFC.
4. عند الترقية إلى مستودع Maven الحقيقي، أعد التشغيل
   `scripts/publish_android_sdk.sh` بدون `--dry-run`، التقط وحدة التحكم
   قم بتسجيل الدخول وتحميل العناصر الناتجة إلى `artifacts/android/maven/<semver>`.

## 7. نموذج حزمة التقديم

يجب أن يتضمن كل إصدار GA/LTS/الإصلاح العاجل ما يلي:

1. **قائمة المراجعة المكتملة** — انسخ جدول هذا الملف، وحدد كل عنصر، ثم قم بالارتباط
   لدعم المصنوعات اليدوية (تشغيل Buildkite، والسجلات، واختلافات المستندات).
2. **أدلة مختبر الجهاز** — ملخص تقرير التصديق، وسجل الحجز، و
   أي تفعيلات طارئة.
3. **حزمة القياس عن بعد** — حالة التنقيح JSON، فرق المخطط، الرابط إلى
   تحديثات `docs/source/sdk/android/telemetry_redaction.md` (إن وجدت).
4. **عناصر الامتثال** — الإدخالات المضافة/المحدثة في مجلد الامتثال
   بالإضافة إلى سجل الأدلة المحدث CSV.
5. **حزمة المصدر** — SBOM، وتوقيع Sigstore، و`checksums.sha256`.
6. **ملخص الإصدار** — نظرة عامة على صفحة واحدة مرفقة بتلخيص `status.md`
   ما ورد أعلاه (التاريخ، الإصدار، تسليط الضوء على أي بوابات تم التنازل عنها).

قم بتخزين الحزمة تحت `artifacts/android/releases/<version>/` وقم بالإشارة إليها
في `status.md` وإصدار RFC.

- `scripts/run_release_pipeline.py --publish-android-sdk ...` تلقائيًا
  نسخ أحدث أرشيف الوبر (`artifacts/android/lint/latest`) وملف
  سجل أدلة الامتثال في `artifacts/android/releases/<version>/` لذلك
  تحتوي حزمة الإرسال دائمًا على موقع أساسي.

---

**تذكير:** قم بتحديث قائمة التحقق هذه كلما كانت هناك وظائف CI جديدة أو عناصر امتثال أو
أو تتم إضافة متطلبات القياس عن بعد. يظل عنصر خريطة الطريق AND6 مفتوحًا حتى
أثبتت قائمة المراجعة والأتمتة المرتبطة بها ثباتها لإصدارين متتاليين
القطارات.