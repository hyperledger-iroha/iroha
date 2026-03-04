---
lang: pt
direction: ltr
source: docs/portal/docs/reference/address-safety.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: سلامة العناوين واتاحة الوصول
description: UX لعرض ومشاركة عناوين Iroha بأمان (ADDR-6c).
---

Você pode usar o ADDR-6c. طبّق هذه القيود على المحافظ و Explorers وادوات SDK واي سطح من البوابة يعرض او يقبل عناوين موجهة Não. O nome do arquivo é `docs/account_structure.md`; وتشرح القائمة ادناه كيف نعرض هذه الصيغ دون المساس بالسلامة او اتاحة الوصول.

## تدفقات مشاركة آمنة

- اجعل كل اجراء نسخ/مشاركة يستخدم عنوان IH58 افتراضيا. Verifique se a soma de verificação está correta.
- قدم اجراء "مشاركة" يجمع العنوان النصي الكامل مع رمز QR مشتق من نفس payload. Certifique-se de que o dispositivo esteja funcionando corretamente.
- عند الحاجة للاختصار بسبب المساحة (بطاقات صغيرة, اشعارات), احتفظ بالبادئة المقروءة, واعرض نقاطا, واحتفظ بآخر 4–6 احرف حتى تبقى نقطة ارتكاز checksum. Certifique-se de que o dispositivo esteja funcionando corretamente.
- امنع عدم تطابق الحافظة عبر اظهار brinde تأكيد يعرض سلسلة IH58 المنسوخة بدقة. Para usar a telemetria, você pode usar o UX para usar o UX.

## IME وضمانات الادخال

- ارفض الادخال غير ASCII no site. عندما تظهر اثار تركيب IME (largura total, Kana, علامات النغمة), اعرض تحذيرا inline يشرح كيفية تبديل لوحة O problema é que o dinheiro está fora do lugar.
- وفر منطقة لصق نصية عادية تزيل العلامات المركبة e تستبدل مسافات بمسافات ASCII قبل التحقق. Isso significa que o IME está no caminho certo.
- شدد التحقق ضد marceneiros de largura zero e seletores de variação وغيرها من نقاط Unicode الخفية. سجل فئة نقطة الرمز المرفوضة حتى تتمكن مجموعات fuzzing من استيراد telemetria.

## توقعات تقنيات المساعدة

- Use `aria-label` e `aria-describedby` para definir a carga útil e a carga útil do dispositivo. 4–8 احرف (“ih traço b três dois…”). Isso pode ser feito por meio de uma mensagem de texto.
- اعلن عن نجاح النسخ/المشاركة عبر تحديث região ao vivo بطريقة educado. اذكر الوجهة (الحافظة, مشاركة, QR) حتى يعرف المستخدم اكتمال الاجراء دون تحريك التركيز.
- وفر نص `alt` وصفي لمعاينات QR (como “Endereço IH58 para `<account>` na cadeia `0x1234`”). قدم خيار "نسخ العنوان كنص" بجانب لوحة QR للمستخدمين ضعاف البصر.

## العناوين المضغوطة الخاصة بـ Sora فقط

- Gating: اخفِ السلسلة المضغوطة `sora…` خلف تأكيد صريح. Verifique se o Sora Nexus está funcionando corretamente.
- Rotulagem: كل ظهور يجب ان يتضمن شارة مرئية "Somente Sora" وتلميحا يوضح لماذا تتطلب الشبكات الاخرى صيغة IH58.
- Guarda-corpos: اذا لم يكن مميز السلسلة النشطة هو تخصيص Nexus, فارفض توليد العنوان المضغوط تماما ووجّه المستخدم الى IH58.
- Telemetria: سجل عدد مرات طلب ونسخ الصيغة المضغوطة حتى يتمكن playbook الحوادث من رصد ارتفاعات المشاركة غير المقصودة.

## بوابات الجودة- وسّع اختبارات UI الالية (او مجموعات a11y no livro de histórias) للتحقق من ان مكونات العنوان تعرض بيانات ARIA المطلوبة E você pode alterar o IME.
- ضمن سيناريوهات QA يدوية لادخال IME (kana, pinyin), وتمرير قارئ الشاشة (VoiceOver/NVDA), ونسخ QR في سمات عالية Não há problema.
- اعكس هذه الفحوصات في قوائم فحص الاصدار بجانب اختبارات تكافؤ IH58 حتى تبقى Você pode fazer isso sem problemas.