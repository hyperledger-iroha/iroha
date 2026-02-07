---
lang: fr
direction: ltr
source: docs/portal/docs/reference/address-safety.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : سلامة العناوين واتاحة الوصول
description : UX UX est basé sur Iroha (ADDR-6c).
---

Il s'agit de l'ADDR-6c. Il s'agit d'un SDK pour les explorateurs et les explorateurs et également pour les utilisateurs et les utilisateurs. نموذج البيانات القياسي موجود في `docs/account_structure.md`؛ وتشرح القائمة ادناه كيف نعرض هذه الصيغ دون المساس بالسلامة او اتاحة الوصول.

## تدفقات مشاركة آمنة

- اجعل كل اجراء نسخ/مشاركة يستخدم عنوان IH58 افتراضيا. Il s'agit d'une somme de contrôle pour la somme de contrôle.
- قدم اجراء "مشاركة" يجمع العنوان النصي الكامل مع رمز QR مشتق من نفس payload. اسمح للمستخدمين بفحص الاثنين قبل التأكيد.
- عند الحاجة للاختصار بسبب المساحة (بطاقات صغيرة، اشعارات), احتفظ بالبادئة المقروءة، واعرض Il y a 4 à 6 niveaux de somme de contrôle. وفر نقرة/اختصار لوحة مفاتيح لنسخ السلسلة الكاملة دون اختصار.
- امنع عدم تطابق الحافظة عبر اظهار toast تأكيد يعرض سلسلة IH58 المنسوخة بدقة. Il s'agit d'une télémétrie, d'une application de télémétrie et d'une application UX.

## IME وضمانات الادخال- ارفض الادخال غير ASCII في حقول العنوان. IME (pleine largeur), Kana, علامات النغمة), اعرض تحذيرا inline يشرح كيفية تبديل لوحة المفاتيح الى ادخال لاتيني قبل المحاولة مجددا.
- Vous pouvez également utiliser des fichiers ASCII ou ASCII. هذا يمنع فقدان التقدم عند تعطيل IME في منتصف التدفق.
- Il existe des menuisiers de largeur nulle et des sélecteurs de variation ainsi que des outils Unicode. Il y a un problème de fuzzing avec la télémétrie.

## توقعات تقنيات المساعدة

- `aria-label` et `aria-describedby` pour la charge utile de 4 à 8 احرف (« ih tiret b trois deux… »). هذا يمنع قارئات الشاشة من انتاج تدفق غير مفهوم من الاحرف.
- اعلن عن نجاح النسخ/المشاركة عبر تحديث région en direct بطريقة poli. اذكر الوجهة (حافظة، مشاركة، QR) حتى يعرف المستخدم اكتمال الاجراء دون تحريك التركيز.
- وفر نص `alt` et لمعاينات QR (مثلا « Adresse IH58 pour `<account>` sur la chaîne `0x1234` »). قدم خيار "نسخ العنوان كنص" بجانب لوحة QR للمستخدمين ضعاف البصر.

## العناوين المضغوطة الخاصة بـ Sora فقط- Gating : اخفِ السلسلة المضغوطة `sora…` خلف تأكيد صريح. Vous pouvez également utiliser Sora Nexus pour le faire.
- Étiquetage : "Sora-only" et "Sora-only" et IH58.
- Garde-corps: اذا لم يكن مميز السلسلة النشطة هو تخصيص Nexus, فارفض توليد العنوان المضغوط تماما ووجّه مستخدم الى IH58.
- Télémétrie : سجل عدد مرات طلب ونسخ الصيغة المضغوطة حتى يتمكن Playbook الحوادث من رصد ارتفاعات المشاركة غير المقصودة.

## بوابات الجودة

- وسّع اختبارات UI الالية (او مجموعات a11y في storybook) للتحقق من ان مكونات العنوان تعرض بيانات ARIA المطلوبة وان رسائل رفض IME تظهر.
- Les applications QA et IME (kana, pinyin) et les applications (VoiceOver/NVDA) et QR et les applications قبل الاصدار.
- اعكس هذه الفحوصات في قوائم فحص الاصدار بجانب اختبارات تكافؤ IH58 حتى تبقى التراجعات محجوبة حتى تصحح.