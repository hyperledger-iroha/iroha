---
lang: ar
direction: rtl
source: docs/source/genesis_bootstrap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6feb2b03bd8f6a41de693a0c3f3c4ffc058072bc7942e2bc50b3fd9770aa56d4
source_last_modified: "2026-01-03T18:08:01.368173+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Genesis Bootstrap من أقرانهم الموثوقين

يمكن لأقران Iroha بدون `genesis.file` المحلي جلب كتلة تكوين موقعة من أقران موثوق بهم
باستخدام بروتوكول التمهيد المشفر Norito.

- **البروتوكول:** تبادل الأقران `GenesisRequest` (`Preflight` للبيانات الوصفية، و`Fetch` للحمولة) و
  إطارات `GenesisResponse` ذات مفاتيح بواسطة `request_id`. يتضمن المستجيبون معرف السلسلة، ومفتاح التوقيع، و
  التجزئة، وتلميح حجم اختياري؛ يتم إرجاع الحمولات فقط على `Fetch`، ومعرفات الطلب المكررة
  تلقي `DuplicateRequest`.
- **الحراس:** يفرض المستجيبون القائمة المسموح بها (`genesis.bootstrap_allowlist` أو النظراء الموثوق بهم
  set)، ومطابقة معرف السلسلة/مفتاح النشر/التجزئة، وحدود المعدل (`genesis.bootstrap_response_throttle`)، و
  الحد الأقصى للحجم (`genesis.bootstrap_max_bytes`). تتلقى الطلبات خارج القائمة المسموح بها `NotAllowed`، و
  الحمولات الموقعة بواسطة المفتاح الخاطئ تتلقى `MismatchedPubkey`.
- **تدفق الطالب:** عندما يكون التخزين فارغًا ويكون `genesis.file` غير محدد (و
  `genesis.bootstrap_enabled=true`)، تقوم العقدة بإجراء اختبارات مبدئية للأقران الموثوق بهم باستخدام الخيار الاختياري
  `genesis.expected_hash`، ثم يقوم بإحضار الحمولة والتحقق من صحة التوقيعات عبر `validate_genesis_block`،
  ويستمر `genesis.bootstrap.nrt` بجانب Kura قبل تطبيق الكتلة. يعيد Bootstrap المحاولة
  الشرف `genesis.bootstrap_request_timeout`، `genesis.bootstrap_retry_interval`، و
  `genesis.bootstrap_max_attempts`.
- **أوضاع الفشل:** يتم رفض الطلبات بسبب الأخطاء في القائمة المسموح بها، وعدم تطابق السلسلة/المفتاح العام/التجزئة، والحجم
  انتهاكات الحد الأقصى، أو حدود الأسعار، أو التكوين المحلي المفقود، أو معرفات الطلبات المكررة. التجزئة المتضاربة
  عبر أقرانهم إحباط الجلب؛ لا يعود المستجيبون/المهلات إلى التكوين المحلي.
- **خطوات المشغل:** تأكد من إمكانية الوصول إلى نظير واحد موثوق به على الأقل من خلال تكوين صالح وتكوينه
  `bootstrap_allowlist`/`bootstrap_max_bytes`/`bootstrap_response_throttle` ومقابض إعادة المحاولة، و
  اختياريًا، قم بتثبيت `expected_hash` لتجنب قبول حمولات غير متطابقة. يمكن أن تكون الحمولات المستمرة
  يُعاد استخدامه في عمليات التمهيد اللاحقة عن طريق الإشارة من `genesis.file` إلى `genesis.bootstrap.nrt`.