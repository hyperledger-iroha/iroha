---
lang: ar
direction: rtl
source: docs/automation/android/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 27b5ac3c7adb19a87f0b3d076f3c9618b188602898ed3954808ac9f7a52b3a62
source_last_modified: "2025-11-15T13:38:36.954059+00:00"
translation_last_reviewed: 2026-01-21
---

# خط أساس أتمتة توثيق Android (AND5)

<div dir="rtl">

يتطلب بند AND5 في خارطة الطريق أن تكون أتمتة التوثيق والتعريب والنشر قابلة
للتدقيق قبل بدء AND6 (CI & Compliance). يسجل هذا المجلد الأوامر والمخرجات
وبنية الأدلة التي تشير إليها AND5/AND6، بما يعكس الخطط الواردة في
`docs/source/sdk/android/developer_experience_plan.md` و
`docs/source/sdk/android/parity_dashboard_plan.md`.

## المسارات والأوامر

| المهمة | الأوامر | المخرجات المتوقعة | ملاحظات |
|--------|---------|-------------------|---------|
| مزامنة stubs للتعريب | `python3 scripts/sync_docs_i18n.py` (اختياريًا تمرير `--lang <code>` لكل تشغيل) | ملف سجل محفوظ تحت `docs/automation/android/i18n/<timestamp>-sync.log` بالإضافة إلى التزامات stubs المترجمة | يحافظ على تزامن `docs/i18n/manifest.json` مع stubs المترجمة؛ يسجل السجل أكواد اللغات التي تم لمسها والالتزام المضمَّن في خط الأساس. |
| التحقق من fixtures + تماثل Norito | `ci/check_android_fixtures.sh` (يغلف `python3 scripts/check_android_fixtures.py --json-out artifacts/android/parity/<stamp>/summary.json`) | انسخ ملخص JSON الناتج إلى `docs/automation/android/parity/<stamp>-summary.json` | يتحقق من الحمولات في `java/iroha_android/src/test/resources`، وتجزيئات المانيفست، وأطوال fixtures الموقّعة. أرفق الملخص مع أدلة الإيقاع ضمن `artifacts/android/fixture_runs/`. |
| بيان العينات ودليل النشر | `scripts/publish_android_sdk.sh --version <semver> [--repo-url …]` (يشغّل الاختبارات + SBOM + provenance) | بيانات وصفية لحزمة provenance مع `sample_manifest.json` الناتج من `docs/source/sdk/android/samples/` محفوظة في `docs/automation/android/samples/<version>/` | يربط تطبيقات عينات AND5 بأتمتة الإصدارات: التقط البيان المُولَّد، وتجزيء SBOM، وسجل provenance لمراجعة البيتا. |
| تغذية لوحة تماثل | `python3 scripts/check_android_fixtures.py … --json-out artifacts/android/parity/<stamp>/summary.json` ثم `python3 scripts/android_parity_metrics.py --summary <summary> --output artifacts/android/parity/<stamp>/metrics.prom` | انسخ لقطة `metrics.prom` أو تصدير Grafana JSON إلى `docs/automation/android/parity/<stamp>-metrics.prom` | يغذي خطة اللوحة كي تتمكن AND5/AND7 من التحقق من عدادات الإرسالات غير الصالحة واعتماد القياس. |

## التقاط الأدلة

1. **ضع طابعًا زمنيًا لكل شيء.** سمِّ الملفات باستخدام طابع وقت UTC
   (`YYYYMMDDTHHMMSSZ`) حتى تتمكن لوحات التماثل ومحاضر الحوكمة والوثائق المنشورة
   من الإشارة إلى التشغيل نفسه.
2. **أدرج المراجع إلى الالتزامات.** يجب أن يتضمن كل سجل تجزئة الالتزام الخاص
   بالتشغيل وأي إعدادات ذات صلة (مثل `ANDROID_PARITY_PIPELINE_METADATA`). عند
   الحاجة إلى تنقيح الخصوصية، أضف ملاحظة واربط بمخزن آمن.
3. **أرشفة سياق محدود.** يتم حفظ الملخصات المنظمة فقط (JSON، `.prom`، `.log`).
   ينبغي أن تبقى المخرجات الثقيلة (حزم APK، لقطات الشاشة) في `artifacts/` أو في
   التخزين الكائني مع تسجيل التجزئة الموقّعة في السجل.
4. **تحديث إدخالات الحالة.** عند تقدم مراحل AND5 في `status.md`، اذكر الملف
   المقابل (مثل `docs/automation/android/parity/20260324T010203Z-summary.json`) حتى
   يتمكن المدققون من تتبع خط الأساس دون التنقيب في سجلات CI.

اتباع هذا التخطيط يحقق متطلب AND6 الخاص بـ "خطوط أساس docs/automation المتاحة
للتدقيق" ويحافظ على برنامج توثيق Android متوافقًا مع الخطط المنشورة.

</div>
