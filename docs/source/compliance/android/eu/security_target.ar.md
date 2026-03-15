---
lang: ar
direction: rtl
source: docs/source/compliance/android/eu/security_target.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0
source_last_modified: "2026-01-03T18:07:59.195967+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android SDK Security Target — ETSI EN 319 401 Alignment

| المجال | القيمة |
|-------|-------|
| نسخة الوثيقة | 0.1 (2026-02-12) |
| النطاق | Android SDK (مكتبات العميل ضمن `java/iroha_android/` بالإضافة إلى البرامج النصية/المستندات الداعمة) |
| المالك | الامتثال والقانون (صوفيا مارتينز) |
| المراجعين | قائد برنامج Android، هندسة الإصدار، حوكمة SRE |

## 1. وصف إصبع القدم

يشتمل هدف التقييم (TOE) على كود مكتبة Android SDK (`java/iroha_android/src/main/java`)، وسطح التكوين الخاص به (`ClientConfig` + Norito)، والأدوات التشغيلية المشار إليها في `roadmap.md` للمعالم الرئيسية AND2/AND6/AND7.

المكونات الأساسية:

1. ** استيعاب التكوين ** — سلاسل `ClientConfig` ونقاط نهاية Torii وسياسات TLS وعمليات إعادة المحاولة وخطافات القياس عن بعد من بيان `iroha_config` الذي تم إنشاؤه ويفرض عدم قابلية التغيير بعد التهيئة (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`).
2. **إدارة المفاتيح / StrongBox** — يتم تنفيذ التوقيع المدعوم بالأجهزة عبر `SystemAndroidKeystoreBackend` و`AttestationVerifier`، مع السياسات الموثقة في `docs/source/sdk/android/key_management.md`. يستخدم التقاط/التحقق من صحة الشهادة `scripts/android_keystore_attestation.sh` ومساعد CI `scripts/android_strongbox_attestation_ci.sh`.
3. **القياس عن بعد والتنقيح** — مسارات تحويل الأجهزة من خلال المخطط المشترك الموضح في `docs/source/sdk/android/telemetry_redaction.md`، وتصدير السلطات المجزأة، وملفات تعريف الأجهزة المجمعة، وتجاوز خطافات التدقيق التي يفرضها دليل التشغيل للدعم.
4. **دفاتر تشغيل العمليات** — `docs/source/android_runbook.md` (استجابة المشغل) و`docs/source/android_support_playbook.md` (SLA + التصعيد) تعمل على تعزيز البصمة التشغيلية لـ TOE من خلال التجاوزات الحتمية وتدريبات الفوضى والتقاط الأدلة.
5. **مصدر الإصدار** — تستخدم الإصدارات المستندة إلى Gradle المكون الإضافي CycloneDX بالإضافة إلى علامات البناء القابلة للتكرار كما تم التقاطها في `docs/source/sdk/android/developer_experience_plan.md` وقائمة التحقق من الامتثال AND6. تم توقيع عناصر الإصدار والإشارة إليها في `docs/source/release/provenance/android/`.

## 2. الأصول والافتراضات

| الأصول | الوصف | الهدف الأمني ​​|
|-------|----------------------------|----|
| يظهر التكوين | لقطات Norito المشتقة من `ClientConfig` الموزعة مع التطبيقات. | الأصالة والنزاهة والسرية في حالة الراحة. |
| مفاتيح التوقيع | المفاتيح التي تم إنشاؤها أو استيرادها من خلال موفري StrongBox/TEE. | تفضيل StrongBox، وتسجيل الشهادات، ولا يوجد تصدير رئيسي. |
| تيارات القياس عن بعد | تتبعات/سجلات/مقاييس OTLP المصدرة من أدوات SDK. | استخدام الأسماء المستعارة (السلطات المجزأة)، وتقليل معلومات تحديد الهوية الشخصية (PII)، وتجاوز التدقيق. |
| تفاعلات دفتر الأستاذ | حمولات Norito، وبيانات تعريف القبول، وحركة مرور الشبكة Torii. | المصادقة المتبادلة، طلبات مقاومة إعادة التشغيل، إعادة المحاولة الحتمية. |

الافتراضات:

- يوفر نظام التشغيل المحمول وضع الحماية القياسي + SELinux؛ تطبق أجهزة StrongBox واجهة Google الرئيسية.
- يوفر المشغلون نقاط نهاية Torii بشهادات TLS موقعة من مراجع التصديق الموثوق بها من المجلس.
- بناء البنية التحتية يفي بمتطلبات البناء القابلة للتكرار قبل النشر إلى Maven.

## 3. التهديدات والضوابط| التهديد | التحكم | الأدلة |
|--------|---------|----------|
| يظهر التكوين العبث | يقوم `ClientConfig` بالتحقق من صحة البيانات (التجزئة + المخطط) قبل التقديم ويسجل عمليات إعادة التحميل المرفوضة عبر `android.telemetry.config.reload`. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`; `docs/source/android_runbook.md` §1–2. |
| التنازل عن مفاتيح التوقيع | السياسات التي تتطلبها StrongBox، وأدوات التصديق، وعمليات تدقيق مصفوفة الأجهزة تحدد الانحراف؛ التجاوزات موثقة لكل حادث. | `docs/source/sdk/android/key_management.md`; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`; `scripts/android_strongbox_attestation_ci.sh`. |
| تسرب معلومات تحديد الهوية الشخصية (PII) في القياس عن بعد | سلطات تجزئة Blake2b، وملفات تعريف الأجهزة المجمعة، وإغفال الناقل، وتجاوز التسجيل. | `docs/source/sdk/android/telemetry_redaction.md`; دعم قواعد اللعبة التي تمارسها §8. |
| إعادة التشغيل أو الرجوع إلى إصدار سابق على Torii RPC | يقوم منشئ الطلبات `/v1/pipeline` بفرض تثبيت TLS وسياسة قناة الضوضاء وإعادة محاولة الميزانيات مع سياق السلطة المجزأة. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ToriiRequestBuilder.java`; `docs/source/sdk/android/networking.md` (مخطط له). |
| الإصدارات غير الموقعة أو غير القابلة للتكرار | شهادات CycloneDX SBOM + Sigstore مقسمة بواسطة قائمة التحقق AND6؛ يتطلب إصدار RFCs دليلاً في `docs/source/release/provenance/android/`. | `docs/source/sdk/android/developer_experience_plan.md`; `docs/source/compliance/android/eu/sbom_attestation.md`. |
| التعامل مع الحادث غير مكتمل | يحدد Runbook + Playbook التجاوزات وتدريبات الفوضى وشجرة التصعيد؛ تتطلب تجاوزات القياس طلبات Norito الموقعة. | `docs/source/android_runbook.md`; `docs/source/android_support_playbook.md`. |

## 4. أنشطة التقييم

1. **مراجعة التصميم** — يتحقق الامتثال + SRE من أن التكوين وإدارة المفاتيح والقياس عن بعد وعناصر التحكم في الإصدار تتوافق مع أهداف أمان ETSI.
2. **فحوصات التنفيذ** — الاختبارات التلقائية:
   - يتحقق `scripts/android_strongbox_attestation_ci.sh` من الحزم الملتقطة لكل جهاز StrongBox مدرج في المصفوفة.
   - يضمن `scripts/check_android_samples.sh` و Managed Device CI أن تحترم نماذج التطبيقات عقود `ClientConfig`/القياس عن بعد.
3. **التحقق من الصحة التشغيلية** — تدريبات الفوضى ربع السنوية لكل `docs/source/sdk/android/telemetry_chaos_checklist.md` (تمارين التنقيح + التجاوز).
4. **الاحتفاظ بالأدلة** — المصنوعات اليدوية المخزنة ضمن `docs/source/compliance/android/` (هذا المجلد) والمشار إليها من `status.md`.

## 5. ETSI EN 319 401 رسم الخرائط| إن 319 401 بند | تحكم SDK |
|-------------------|------------|
| 7.1 السياسة الأمنية | تم توثيقه في هذا الهدف الأمني ​​+ دليل التشغيل للدعم. |
| 7.2 الأمن التنظيمي | RACI + الملكية عند الطلب في Support Playbook §2. |
| 7.3 إدارة الأصول | أهداف التكوين والمفتاح وأصول القياس عن بعد المحددة في الفقرة 2 أعلاه. |
| 7.4 التحكم في الوصول | سياسات StrongBox + تجاوز سير العمل الذي يتطلب عناصر Norito الموقعة. |
| 7.5 ضوابط التشفير | متطلبات إنشاء المفاتيح وتخزينها واعتمادها من AND2 (دليل إدارة المفاتيح). |
| 7.6 أمن العمليات | تجزئة القياس عن بعد، وتدريبات الفوضى، والاستجابة للحوادث، وإطلاق بوابة الأدلة. |
| 7.7 أمن الاتصالات | `/v1/pipeline` سياسة TLS + السلطات المجزأة (مستند تنقيح القياس عن بعد). |
| 7.8 اكتساب/ تطوير النظام | إنشاءات Gradle القابلة للتكرار ووحدات SBOM وبوابات المصدر في خطط AND5/AND6. |
| 7.9 علاقات الموردين | شهادات Buildkite + Sigstore المسجلة جنبًا إلى جنب مع SBOMs التابعة لجهة خارجية. |
| 7.10 إدارة الحوادث | تصعيد Runbook/Playbook، وتجاوز التسجيل، وعدادات فشل القياس عن بعد. |

## 6. الصيانة

- قم بتحديث هذا المستند عندما يقدم SDK خوارزميات تشفير جديدة أو فئات القياس عن بعد أو تغييرات أتمتة الإصدار.
- ربط النسخ الموقعة في `docs/source/compliance/android/evidence_log.csv` بملخصات SHA-256 وتوقيعات المراجعين.