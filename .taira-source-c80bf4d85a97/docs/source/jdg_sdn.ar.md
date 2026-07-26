---
lang: ar
direction: rtl
source: docs/source/jdg_sdn.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1ee87ee60e2e8c9d9636b282231b33de3cf1fd7240c8d31d0a0a1673651dcef1
source_last_modified: "2026-01-03T18:07:58.621058+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

٪ شهادات JDG-SDN والتناوب

تلخص هذه المذكرة نموذج التنفيذ لشهادات عقدة البيانات السرية (SDN).
المستخدمة من قبل تدفق بيانات الولاية القضائية (JDG).

## تنسيق الالتزام
- `JdgSdnCommitment` يربط النطاق (`JdgAttestationScope`) المشفر
  تجزئة الحمولة والمفتاح العام لـ SDN. الأختام هي التوقيعات المكتوبة
  (`SignatureOf<JdgSdnCommitmentSignable>`) عبر الحمولة النافعة ذات علامات المجال
  `iroha:jurisdiction:sdn:commitment:v1\x00 || norito(signable)`.
- التحقق من الصحة الهيكلية (`validate_basic`) يفرض:
  -`version == JDG_SDN_COMMITMENT_VERSION_V1`
  - نطاقات كتلة صالحة
  - الأختام غير الفارغة
  - نطاق المساواة مقابل التصديق عند التشغيل عبر
    `JdgAttestation::validate_with_sdn`/`validate_with_sdn_registry`
- تتم معالجة إلغاء البيانات المكررة بواسطة مدقق التصديق (الموقع + تجزئة الحمولة
  التفرد) لمنع الالتزامات المحتجزة/المكررة.

## سياسة التسجيل والتناوب
- مفاتيح SDN موجودة في `JdgSdnRegistry`، ومفاتيح بواسطة `(Algorithm, public_key_bytes)`.
- يسجل `JdgSdnKeyRecord` ارتفاع التنشيط وارتفاع التقاعد الاختياري،
  ومفتاح الوالدين الاختياري.
- يخضع التدوير لـ `JdgSdnRotationPolicy` (حاليًا: `dual_publish_blocks`)
  نافذة متداخلة). يؤدي تسجيل مفتاح فرعي إلى تحديث تقاعد الوالدين إلى
  `child.activation + dual_publish_blocks`، مع الدرابزين:
  - يتم رفض الوالدين المفقودين
  - يجب أن تكون عمليات التنشيط في ازدياد صارم
  - يتم رفض التداخلات التي تتجاوز نافذة السماح
- يعرض مساعدو التسجيل السجلات المثبتة (`record`، `keys`) للحالة
  والتعرض لواجهة برمجة التطبيقات (API).

## تدفق التحقق
- `JdgAttestation::validate_with_sdn_registry` يغلف الهيكل
  عمليات التحقق من الشهادات وإنفاذ SDN. المواضيع `JdgSdnPolicy`:
  - `require_commitments`: فرض وجود معلومات تحديد الهوية الشخصية/الحمولات السرية
  - `rotation`: نافذة السماح المستخدمة عند تحديث تقاعد الوالدين
- يتم فحص كل التزام من أجل:
  - الصلاحية الهيكلية + تطابق نطاق التصديق
  - وجود مفتاح مسجل
  - نافذة نشطة تغطي نطاق الكتلة المصدق (حدود التقاعد بالفعل
    تضمين نعمة النشر المزدوج)
  - ختم صالح على نص الالتزام الموسوم بالمجال
- تظهر الأخطاء المستقرة فهرس أدلة المشغل:
  `MissingSdnCommitments`، `UnknownSdnKey`، `InactiveSdnKey`، `InvalidSeal`،
  أو فشل `Commitment`/`ScopeMismatch` الهيكلي.

## دليل تشغيل المشغل
- **التوفير:** قم بتسجيل أول مفتاح SDN باستخدام `activated_at` عند أو قبل
  أول ارتفاع الكتلة السرية. قم بنشر البصمة الرئيسية لمشغلي JDG.
- **تدوير:** قم بإنشاء المفتاح اللاحق، وقم بتسجيله باستخدام `rotation_parent`
  مشيرا إلى المفتاح الحالي، وتأكيد التقاعد الأصلي يساوي
  `child_activation + dual_publish_blocks`. إعادة ختم التزامات الحمولة مع
  المفتاح النشط أثناء نافذة التداخل.
- **التدقيق:** كشف لقطات التسجيل (`record`، `keys`) عبر Torii/status
  الأسطح حتى يتمكن المدققون من تأكيد المفتاح النشط ونوافذ التقاعد. تنبيه
  إذا كان النطاق المصدق يقع خارج النافذة النشطة.
- **الاسترداد:** `UnknownSdnKey` → تأكد من أن السجل يتضمن مفتاح الختم؛
  `InactiveSdnKey` → تدوير أو ضبط ارتفاعات التنشيط؛ `InvalidSeal` →
  إعادة ختم الحمولات وتحديث الشهادات.## مساعد وقت التشغيل
- `JdgSdnEnforcer` (`crates/iroha_core/src/jurisdiction.rs`) حزم السياسة +
  التسجيل والتحقق من صحة الشهادات عبر `validate_with_sdn_registry`.
- يمكن تحميل السجلات من حزم `JdgSdnKeyRecord` المشفرة بـ Norito (راجع
  `JdgSdnEnforcer::from_reader`/`from_path`) أو تجميعها مع
  `from_records`، الذي يطبق حواجز الحماية الدوارة أثناء التسجيل.
- يمكن للمشغلين الاستمرار في حزمة Norito كدليل على Torii/status
  تطفو على السطح بينما تغذي نفس الحمولة المنفذ الذي يستخدمه القبول و
  حراس الإجماع. يمكن تهيئة منفذ عالمي واحد عند بدء التشغيل عبر
  `init_enforcer_from_path`، و`enforcer()`/`registry_snapshot()`/`sdn_registry_status()`
  كشف السياسة المباشرة + السجلات الرئيسية لأسطح الحالة/Torii.

## الاختبارات
- تغطية الانحدار في `crates/iroha_data_model/src/jurisdiction.rs`:
  `sdn_registry_accepts_active_commitment`، `sdn_registry_rejects_unknown_key`،
  `sdn_registry_rejects_inactive_key`، `sdn_registry_rejects_bad_signature`،
  `sdn_registry_sets_parent_retirement_window`,
  `sdn_registry_rejects_overlap_beyond_policy`، بجانب الموجود
  اختبارات التصديق الهيكلي/التحقق من صحة SDN.