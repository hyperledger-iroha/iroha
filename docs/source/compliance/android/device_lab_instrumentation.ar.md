---
lang: ar
direction: rtl
source: docs/source/compliance/android/device_lab_instrumentation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9d384e21d09f3c4f57b7fc5181d69dc0da739dd6ed4dcb89a57ea58fd29bb898
source_last_modified: "2026-01-03T18:07:59.259775+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# خطافات أجهزة مختبر أجهزة Android (AND6)

يغلق هذا المرجع إجراء خريطة الطريق "مرحلة ما تبقى من معمل الأجهزة /
خطافات الأجهزة قبل انطلاق AND6 ". ويشرح كيف كل محفوظة
يجب أن تقوم فتحة Device-lab بالتقاط بيانات القياس عن بعد وقائمة الانتظار والتصديق حتى يتم
تشترك قائمة التحقق من الامتثال AND6 وسجل الأدلة وحزم الإدارة في نفس الشيء
سير العمل الحتمي. قم بإقران هذه الملاحظة مع إجراء الحجز
(`device_lab_reservation.md`) ودليل تشغيل تجاوز الفشل عند التخطيط للتدريبات.

## الأهداف والنطاق

- **الأدلة الحتمية** - جميع مخرجات الأجهزة موجودة تحتها
  يظهر `artifacts/android/device_lab/<slot-id>/` مع SHA-256 للمدققين
  يمكن فرق الحزم دون إعادة تشغيل المجسات.
- **سير عمل البرنامج النصي أولاً** - إعادة استخدام المساعدين الموجودين
  (`ci/run_android_telemetry_chaos_prep.sh`،
  `scripts/android_keystore_attestation.sh`، `scripts/android_override_tool.sh`)
  بدلاً من أوامر adb المخصصة.
- **تبقى قوائم المراجعة متزامنة** - كل عملية تشغيل تشير إلى هذا المستند من
  قائمة التحقق من الامتثال AND6 وإلحاق المصنوعات بـ
  `docs/source/compliance/android/evidence_log.csv`.

## تخطيط قطعة أثرية

1. اختر معرف فتحة فريدًا يطابق تذكرة الحجز، على سبيل المثال.
   `2026-05-12-slot-a`.
2. زرع الدلائل القياسية:

   ```bash
   export ANDROID_DEVICE_LAB_SLOT=2026-05-12-slot-a
   export ANDROID_DEVICE_LAB_ROOT="artifacts/android/device_lab/${ANDROID_DEVICE_LAB_SLOT}"
   mkdir -p "${ANDROID_DEVICE_LAB_ROOT}"/{telemetry,attestation,queue,logs}
   ```

3. احفظ كل سجل أوامر داخل المجلد المطابق (على سبيل المثال.
   `telemetry/status.ndjson`، `attestation/pixel8pro.log`).
4. يظهر الالتقاط SHA-256 بمجرد إغلاق الفتحة:

   ```bash
   find "${ANDROID_DEVICE_LAB_ROOT}" -type f -print0 | sort -z \
     | xargs -0 shasum -a 256 > "${ANDROID_DEVICE_LAB_ROOT}/sha256sum.txt"
   ```

## مصفوفة الأجهزة

| التدفق | الأمر (الأوامر) | موقع الإخراج | ملاحظات |
|------|-----------|-----------------|-------|
| تنقيح القياس عن بعد + حزمة الحالة | `scripts/telemetry/check_redaction_status.py --status-url <collector> --json-out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/status.ndjson` | `telemetry/status.ndjson`، `telemetry/status.log` | تشغيل في بداية ونهاية الفتحة؛ قم بإرفاق CLI stdout بـ `status.log`. |
| قائمة الانتظار المعلقة + الفوضى الإعدادية | `ANDROID_PENDING_QUEUE_EXPORTS="pixel8=${ANDROID_DEVICE_LAB_ROOT}/queue/pixel8.bin" ci/run_android_telemetry_chaos_prep.sh --status-only` | `queue/*.bin`، `queue/*.json`، `queue/*.sha256` | مرايا السيناريوD من `readiness/labs/telemetry_lab_01.md`؛ قم بتوسيع env var لكل جهاز في الفتحة. |
| تجاوز ملخص دفتر الأستاذ | `scripts/android_override_tool.sh digest --out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/override_digest.json` | `telemetry/override_digest.json` | مطلوبة حتى في حالة عدم وجود أي تجاوزات نشطة؛ اثبات حالة الصفر |
| شهادة StrongBox / TEE | `scripts/android_keystore_attestation.sh --device pixel8pro-strongbox-a --out "${ANDROID_DEVICE_LAB_ROOT}/attestation/pixel8pro"` | `attestation/<device>/*.{json,zip,log}` | كرر ذلك لكل جهاز محجوز (تطابق الأسماء في `android_strongbox_device_matrix.md`). |
| انحدار شهادة تسخير CI | `scripts/android_strongbox_attestation_ci.sh --output "${ANDROID_DEVICE_LAB_ROOT}/attestation/ci"` | `attestation/ci/*` | يلتقط نفس الأدلة التي يقوم بتحميلها CI؛ تدرج في التشغيل اليدوي للتماثل. |
| الوبر / خط الأساس للتبعية | `ANDROID_LINT_SUMMARY_OUT="${ANDROID_DEVICE_LAB_ROOT}/logs/jdeps-summary.txt" make android-lint` | `logs/jdeps-summary.txt`، `logs/lint.log` | تشغيل مرة واحدة لكل نافذة تجميد؛ استشهد بالملخص في حزم الامتثال. |

## إجراء الفتحة القياسية1. **رحلة ما قبل الرحلة (T-24h)** – تأكد من أن تذكرة الحجز تشير إلى ذلك
   المستند، وقم بتحديث إدخال مصفوفة الجهاز، وقم ببذر جذر القطعة الأثرية.
2. **خلال الفتحة**
   - قم بتشغيل حزمة القياس عن بعد + أوامر تصدير قائمة الانتظار أولاً. تمرير
     `--note <ticket>` إلى `ci/run_android_telemetry_chaos_prep.sh` لذلك السجل
     يشير إلى معرف الحادث.
   - تشغيل البرامج النصية للتصديق لكل جهاز. عندما ينتج الحزام أ
     `.zip`، انسخه في جذر المنتج وسجل Git SHA المطبوع على
     نهاية البرنامج النصي.
   - قم بتنفيذ `make android-lint` باستخدام المسار التلخيصي الذي تم تجاوزه حتى لو كان CI
     ركض بالفعل؛ يتوقع المدققون وجود سجل لكل فتحة.
3. **ما بعد التشغيل**
   - قم بإنشاء `sha256sum.txt` و`README.md` (ملاحظات حرة) داخل الفتحة
     مجلد يلخص الأوامر المنفذة.
   - إلحاق صف بـ `docs/source/compliance/android/evidence_log.csv` باستخدام الملحق
     معرف الفتحة ومسار بيان التجزئة ومراجع Buildkite (إن وجدت) والأحدث
     النسبة المئوية لسعة مختبر الجهاز من تصدير تقويم الحجز.
   - قم بربط مجلد الفتحة في تذكرة `_android-device-lab` بملف AND6
     قائمة المراجعة وتقرير الإصدار `docs/source/android_support_playbook.md`.

## التعامل مع الفشل والتصعيد

- في حالة فشل أي أمر، التقط إخراج stderr ضمن `logs/` واتبع
  سلم التصعيد في `device_lab_reservation.md` §6.
- يجب أن يشير النقص في قائمة الانتظار أو القياس عن بعد على الفور إلى حالة التجاوز
  `docs/source/sdk/android/telemetry_override_log.md` وقم بالإشارة إلى معرف الفتحة
  حتى تتمكن الإدارة من تتبع التدريبات.
- يجب تسجيل الانحدارات في الشهادة
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  مع تسلسلات الأجهزة الفاشلة ومسارات الحزمة المسجلة أعلاه.

## قائمة مراجعة التقارير

قبل وضع علامة على اكتمال الفتحة، تأكد من تحديث المراجع التالية:

- `docs/source/compliance/android/and6_compliance_checklist.md` - ضع علامة على
  أكمل صف الأجهزة ولاحظ معرف الفتحة.
- `docs/source/compliance/android/evidence_log.csv` - إضافة/تحديث الإدخال باستخدام
  تجزئة الفتحة وقراءة السعة.
- تذكرة `_android-device-lab` - قم بإرفاق روابط المنتجات ومعرفات مهام Buildkite.
- `status.md` - قم بتضمين ملاحظة مختصرة في ملخص جاهزية Android التالي
  يعرف قراء خارطة الطريق أي فتحة أنتجت أحدث الأدلة.

باتباع هذه العملية، يتم الاحتفاظ بـ "خطافات الأجهزة + مختبر الأجهزة" الخاصة بـ AND6.
علامة فارقة قابلة للتدقيق وتمنع الاختلاف اليدوي بين الحجز والتنفيذ،
والإبلاغ.