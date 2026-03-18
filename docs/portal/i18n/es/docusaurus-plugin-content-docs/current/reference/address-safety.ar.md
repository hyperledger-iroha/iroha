---
lang: es
direction: ltr
source: docs/portal/docs/reference/address-safety.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: سلامة العناوين واتاحة الوصول
descripción: متطلبات UX لعرض ومشاركة عناوين Iroha بأمان (ADDR-6c).
---

Utilice el ADDR-6c. طبّق هذه القيود على المحافظ و explorers وادوات SDK واي سطح من البوابة يعرض او يقبل عناوين موجهة للبشر. Nombre del producto: `docs/account_structure.md`؛ وتشرح القائمة ادناه كيف نعرض هذه الصيغ دون المساس بالسلامة او اتاحة الوصول.

## تدفقات مشاركة آمنة

- Utilice un dispositivo de encendido/apagado para el modelo I105. اعرض النطاق المحسوم كسياق داعم حتى تبقى السلسلة ذات checksum في المقدمة.
- قدم اجراء "مشاركة" يجمع العنوان النصي الكامل مع رمز QR مشتق من نفس payload. اسمح للمستخدمين بفحص الاثنين قبل التأكيد.
- عند الحاجة للاختصار بسبب المساحة (بطاقات صغيرة، اشعارات), احتفظ بالبادئة المقروءة، واعرض نقاطا، Haga clic en 4–6 para realizar la suma de comprobación. وفر نقرة/اختصار لوحة مفاتيح لنسخ السلسلة الكاملة دون اختصار.
- Para hacer una tostada, use el I105. عندما تتوفر telemetría, احسب محاولات النسخ مقابل عمليات المشاركة لالتقاط تراجعات UX بسرعة.

## IME وضمانات الادخال- ارفض الادخال غير ASCII في حقول العنوان. عندما تظهر اثار تركيب IME (ancho completo, Kana, علامات النغمة), اعرض تحذيرا inline يشرح كيفية تبديل لوحة المفاتيح الى ادخال لاتيني قبل المحاولة مجددا.
- Y la configuración de la configuración de la computadora y la configuración de archivos ASCII. هذا يمنع فقدان التقدم عند تعطيل IME في منتصف التدفق.
- شدد التحقق ضد ensambladores de ancho cero y selectores de variación وغيرها من نقاط Unicode الخفية. سجل فئة نقطة الرمز المرفوضة حتى تتمكن مجموعات fuzzing من استيراد telemetría.

## توقعات تقنيات المساعدة

- Haga clic en `aria-label` e `aria-describedby` para cambiar la carga útil entre 4 y 8 pasos (“ih guión b tres dos…”). هذا يمنع قارئات الشاشة من انتاج تدفق غير مفهوم من الاحرف.
- اعلن عن نجاح النسخ/المشاركة عبر تحديث región en vivo بطريقة educado. اذكر الوجهة (الحافظة، مشاركة، QR) حتى يعرف المستخدم اكتمال الاجراء دون تحريك التركيز.
- وفر نص `alt` وصفي لمعاينات QR (مثلا “Dirección I105 para `<account>` en la cadena `0x1234`”). قدم خيار "نسخ العنوان كنص" بجانب لوحة QR للمستخدمين ضعاف البصر.

## العناوين المضغوطة الخاصة بـ Sora فقط- Entrada: اخفِ السلسلة المضغوطة `i105` خلف تأكيد صريح. يجب ان يكرر التأكيد ان الصيغة تعمل فقط على سلاسل Sora Nexus.
- Etiquetado: كل ظهور يجب ان يتضمن شارة مرئية "Sora-only" وتلميحا يوضح لماذا تتطلب الشبكات الاخرى صيغة I105.
- Barandillas: اذا لم يكن مميز السلسلة النشطة هو تخصيص Nexus, فارفض توليد العنوان المضغوط تماما ووجّه مستخدم الى I105.
- Telemetría: سجل عدد مرات طلب ونسخ الصيغة المضغوطة حتى يتمكن playbook الحوادث من رصد ارتفاعات المشاركة غير المقصودة.

## بوابات الجودة

- وسّع اختبارات UI الالية (او مجموعات a11y في storybook) للتحقق من ان مكونات العنوان تعرض بيانات ARIA المطلوبة وان رسائل رفض IME تظهر.
- ضمن سيناريوهات QA يدوية لادخال IME (kana, pinyin), y قارئ الشاشة (VoiceOver/NVDA), y QR في سمات عالية التباين قبل الاصدار.
- اعكس هذه الفحوصات في قوائم فحص الاصدار بجانب اختبارات تكافؤ I105 حتى تبقى التراجعات محجوبة حتى تصحح.