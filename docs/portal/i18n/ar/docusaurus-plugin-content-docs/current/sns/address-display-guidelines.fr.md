---
lang: ar
direction: rtl
source: docs/portal/docs/sns/address-display-guidelines.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

استيراد ExplorerAddressCard من '@site/src/components/ExplorerAddressCard'؛

:::ملاحظة المصدر الكنسي
هذه الصفحة تعكس `docs/source/sns/address_display_guidelines.md` و sert
صيانة نسخة Canonique du Portail. يبقى الملف المصدري للعلاقات العامة
دي ترادوكشن.
:::

تحتوي الحافظات والمستكشفون وأمثلة SDK على العناوين
من حساب الحمولات غير القابلة للتغيير. مثال على محفظة Android بالتجزئة
في `examples/android/retail-wallet` يتم صيانة نمط UX:- **اثنين من أسراب النسخ.** قم بتزويد اثنين من أزرار النسخ الصريحة: IH58
  (يفضل) وشكل الضغط Sora فقط (`sora...`، الخيار الثاني). IH58 موجود دائمًا
  تأكد من مشاركة حمولة QR خارجيًا وشحنها. الضغط المتغير
  يجب أن تتضمن إعلانًا مضمنًا لأنه لا يعمل في
  تطبيقات الأسعار تهمة على قدم المساواة سورا. مثال على Android المتفرع من الأزرار المزدوجة Material et
  تلميحات الأدوات لدينا
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`، وآخرون
  العرض التوضيحي لنظام التشغيل iOS SwiftUI يعيد إحياء تجربة المستخدم عبر `AddressPreviewCard` في
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **مساحة أحادية، يمكن اختيار النص.** قم بعرض السلاسل الثنائية مع الشرطة
  monospace و`textIsSelectable="true"` يساعدان المستخدمين
  المفتش les valeurs sans invoquer un IME. تجنب الأبطال القابلة للتحرير: les
  يمكن أن يقوم IME بإعادة كتابة القناة أو إدخال نقاط التعليمات البرمجية إلى صفر أكبر.
- **مؤشرات المجال الافتراضي الضمني.** عند تحديد النقطة سور
  المجال الضمني `default`، يعرض أسطورة للمشغلين
  qu'aucun لاحقة لا تتطلب. يجب على المستكشفين أيضًا أن يتقدموا للأمام
  تسمية المجال الكنسي عندما يقوم المحدد بتشفير ملخص.
- **QR IH58.** تعمل رموز QR على تشفير السلسلة IH58. سي لا جيل دو
  صدى QR، لعرض خطأ واضح بدلاً من مقطع فيديو للصورة.
- **أوراق الرسائل المطبوعة.** بعد أن تتجنب نسخ الشكل المضغوط، قم بتحريرهاالخبز المحمص أو الوجبات الخفيفة Rappelant aux utilisateurs qu'elle est Sora-only et sujette
  على غرار تشويه IME.

ستؤدي هذه الحماية الجيدة إلى تجنب الفساد في Unicode/IME وتلبية المعايير
قبول خريطة الطريق ADDR-6 لـ UX portefeuille/explorateur.

## يلتقط المرجعية

استخدم المراجع التالية عند إيرادات الترجمة لضمان ذلك
أن علامات الأزرار وتلميحات الأدوات والإعلانات تظل متوازية فيما بينها
الصفائح:

- مرجع أندرويد: `/img/sns/address_copy_android.svg`

  ![نسخة Android المزدوجة المرجعية](/img/sns/address_copy_android.svg)

- مرجع iOS: `/img/sns/address_copy_ios.svg`

  ![مرجع نسخة iOS مزدوجة](/img/sns/address_copy_ios.svg)

## مساعدين SDK

يعرض Chaque SDK مساعدًا للعقد يقوم بإرجاع نماذج IH58 et
اضغط أيضًا على سلسلة الإعلانات حتى تبقى الأرائك في واجهة المستخدم
متماسكة:

- جافا سكريبت: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- مفتش جافا سكريبت: `inspectAccountId(...)` يعيد السلسلة
  ضغط الإعلان والإضافة إلى `warnings` عند المستأنفين
  قدم حرفيًا `sora...`، حتى يتمكن المستكشفون/لوحات اللوح من اللوح
  يمكن عرض النافذة بإعلان Sora فقط أثناء تدفقها
  الكولاج/التحقق من الصحة مما يؤدي فقط إلى إنشاء ميمات مماثلة للشكل
  ضاغط.
- بايثون: `AccountAddress.display_formats(network_prefix: int = 753)`
- سويفت: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
- جافا/كوتلين: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)استخدم هذه الأدوات المساعدة بدلاً من إعادة تنفيذ منطق التشفير في هذه
واجهة مستخدم الأرائك. يقوم مساعد JavaScript بكشف الحمولة `selector` أيضًا
`domainSummary` (`tag`، `digest_hex`، `registry_id`، `label`) لواجهات المستخدم
مؤشر قوي إذا كان المحدد هو Local-12 أو قم بالتسجيل بدون تسجيل
إعادة تحليل الحمولة الخام.

## عرض توضيحي لأداة المستكشف



يجب على المستكشفين إعادة إنتاج عمل القياس عن بعد وإمكانية الوصول إليه
fait pour le portefeuille:- Appliquez `data-copy-mode="ih58|compressed|qr"` aux boutons de copie afin que
  يمكن للواجهات الأمامية أن تطلق حاسبات الاستخدام بالتوازي
  متري Torii `torii_address_format_total`. العرض التوضيحي للمؤلف
  أرسل حدثًا `iroha:address-copy` مع `{mode,timestamp}` - تخلص من هذا
  خط أنابيب التحليل/القياس عن بعد الخاص بك (على سبيل المثال مبعوث مقطع أو وحدة
  قاعدة التجميع في NORITO) حتى تتمكن لوحات المعلومات من ربط الاستخدام
  تنسيق عنوان الخادم مع أوضاع نسخ العميل. ريفليتيز
  بالإضافة إلى حاسبات المجال Torii (`torii_address_domain_total{domain_kind}`)
  في تدفق الميمات من أجل إعادة إنتاج أفلام Local-12 القوية، يمكنك تصديرها
  Preuve de 30 jours `domain_kind="local12"` مباشرة بعد اللوحة
  `address_ingest` من Grafana.
- Associez chaque controle a des المؤشرات `aria-label`/`aria-describedby`
  المميزات التي توضح ما إذا كان الحرفي موجودًا في مشاركة (IH58) أو Sora فقط
  (ضغط). أدخل أسطورة المجال الضمني في الوصف من أجل
  تعكس تقنيات المساعدة سياق الصورة التي يتم عرضها.
- عرض منطقة مباشرة (على سبيل المثال `<output aria-live="polite">...</output>`).
  قم بإعلان نتائج النسخ والتحذيرات بمحاذاة
  يتم توصيل VoiceOver/TalkBack عبر الكابل من خلال أمثلة Swift/Android.هذه الأجهزة ترضي ADDR-6b وتؤكد أن المشغلين قد يتمكنون من ذلك
مراقبة عملية الإدخال Torii وأوضاع نسخ العميل قبل ذلك
المحددات المحلية soient desactives.

## مجموعة أدوات الهجرة المحلية -> العالمية

استخدم [toolkit Local -> Global](local-to-global-toolkit.md) للصب
أتمتة عملية التدقيق وتحويل المحددات المحلية. لو المساعد
قم بإنشاء تقرير مراجعة JSON والقائمة المحولة إلى IH58/compressee مرة أخرى
ينضم المشغلون إلى تذاكر الاستعداد بينما يرتبط دفتر التشغيل
تكمن في لوحات المعلومات Grafana والقواعد Alertmanager التي تم قفلها
قطع في وضع صارم.

## المرجع السريع للتخطيط الثنائي (ADDR-1a)

عندما تعرض SDK انقطاعًا مسبقًا في العنوان (المفتشون، مؤشرات
التحقق من الصحة، منشئو البيان)، يشيرون إلى التطوير مقابل التنسيق
التقاط سلك canonique في `docs/account_structure.md`. التخطيط دائمًا
`header · selector · controller`، أو أجزاء الرأس هي:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```- `addr_version = 0` (البتات 7-5) اليوم؛ القيم غير الصفرية هي احتياطيات
  ورافعة doivent `AccountAddressError::InvalidHeaderVersion`.
- `addr_class` يميز بين وحدات التحكم البسيطة (`0`) من multisig (`1`).
- `norm_version = 1` يقوم بتشفير قواعد التحديد Norm v1. العقود الآجلة ليه نورم
  إعادة استخدام ميمي البطل 2 بت.
- `ext_flag` vaut toujours `0`; les bits actifs indiquent des Extensions de
  الحمولة غير مدفوعة.

يناسب المحدد الرأس فورًا:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

تعمل واجهات المستخدم وSDK على عرض نوع المحدد:

- `0x00` = النطاق الافتراضي الضمني (بدون حمولة).
- `0x01` = ملخص محلي (12 بايت `blake2s_mac("SORA-LOCAL-K:v1", label)`).
- `0x02` = مدخل التسجيل العالمي (`registry_id:u32` كبير النهاية).

أمثلة على القواعد السداسية التي يمكن أن تكون أدوات المحفظة إما مدمجة أو متكاملة
مستندات/اختبارات aux:

| نوع المحدد | السداسية الكنسي |
|---------------|---------------|
| ضمني افتراضيا | `0x02000001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| الملخص المحلي (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| التسجيل العالمي (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

Voir `docs/source/references/address_norm_v1.md` pour la table complet
حدد/الحالة و`docs/account_structure.md` للرسم التخطيطي الكامل
بايت.

## فرض الأشكال canoniques

المشغلون الذين يقومون بتحويل الترميزات المحلية إلى IH58 canonique
يجب عليك اتباع سلاسل الضغطات لسير العمل CLI المستند الخاص بـ ADDR-5:1. `iroha tools address inspect` يقوم بصيانة استئناف بنية JSON مع IH58،
   ضغط وحمولة الحمولات السداسية. تتضمن السيرة الذاتية أيضًا كائنًا
   `domain` مع الأبطال `kind`/`warning` ويفتح المجال بأكمله عبر
   لو تشامب `input_domain`. عندما `kind` vaut `local12`، لا CLI يطبع un
   Avertissement sur stderr واستئناف JSON يعكس الرسالة المرسلة لذلك
   يمكن عرض خطوط الأنابيب CI وSDK. باسيز `--append-domain`
   عندما ترغب في تجديد التشفير المحول إلى الشكل `<ih58>@<domain>`.
2. يمكن لـ SDK عرض رسالة التنبيه/الاستئناف عبر المساعد
   جافا سكريبت:

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed);
   ```
  يقوم المساعد في الحفاظ على البادئة IH58 بالكشف عن الحرفي إذا كنت ترغب في ذلك
  قم بتقديم `networkPrefix` بشكل واضح، قم بإرسال السيرة الذاتية للأبحاث
  Non defaut ne sont pas rerendus silencieusement avec le prefixe par defaut.3. قم بتحويل الحمولة الكنسي إلى إعادة استخدام الأبطال `ih58.value` ou
   `compressed` من السيرة الذاتية (أو تطلب تشفيرًا آخر عبر `--format`). سيس
   السلاسل هي بالتأكيد مشاركة خارجية.
4. قم بإعداد البيانات والسجلات والمستندات الموجهة يوميًا من قبل العميل
   النموذج القانوني وإخطار المشاركين بأن المحددات المحلية ستظهر
   يرفض une fois le Cutover termine.
5. قم بتنفيذ الألعاب بشكل جماعي
   `iroha tools address audit --input addresses.txt --network-prefix 753`. لا أمر
   lit des literaux separes par nouvelle ligne (les commentaires commencant par
   `#` يتم تجاهله، و`--input -` أو علامة تستخدم STDIN)، وإصدار تقرير
   JSON مع السيرة الذاتية canoniques/IH58/compresse لكل إدخال وحساب
   أخطاء التحليل بالإضافة إلى إعلانات النطاق المحلي. استخدم
   `--allow-errors` أثناء تدقيق مخلفات المخلفات المحتوي على الخطوط
   الطفيليات، وحظر الأتمتة عبر `--fail-on-warning` عندما تكون
   تعمل المشغلات على حظر المحددات المحلية في CI.
6. عندما تحتاج إلى إعادة كتابة الخط على الخط، استخدمه
  Pour les feuilles de calcule de recession des sélecteurs Local, utilisez
  من أجل مُصدِّر ملف CSV `input,status,format,...` الذي تم إعداده مسبقًا بالتشفيرات
  المعايير والإعلانات والتحليلات في خطوة واحدة.
   يقوم المساعد بتجاهل الخطوط غير المحلية بشكل افتراضي، وتحويلها كل مرةيتم حفظه في التشفير المطلوب (IH58/compresse/hex/JSON)، مع الاحتفاظ بالملف
   المجال الأصلي عندما `--append-domain` نشط. أسوشييز لو أ
   `--allow-errors` لمواصلة تحليل المحتوى أثناء تفريغ المحتوى
   أشكال literaux mal.
7. قد يؤدي أتمتة CI/lint إلى تنفيذ المنفذ `ci/check_address_normalize.sh`، وهو ما
   قم بإخراج المحددات المحلية من `fixtures/account/address_vectors.json`، les
   قم بالتحويل عبر `iroha tools address normalize`، واستمتع
   `iroha tools address audit --fail-on-warning` للتحقق من الإصدارات
   n'emetent plus de هضم محلي.`torii_address_local8_total{endpoint}` زائد
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}` واللوحة Grafana
`dashboards/grafana/address_ingest.json` يوفر إشارة التطبيق:
مرة واحدة تكون لوحات معلومات الإنتاج خالية من أي طلبات محلية
شرعية وصفر تصادمات محلية 12 قلادة 30 يوم متتالية، Torii
قم بإلقاء نظرة على البوابة المحلية 8 وتتبعها بشكل صارم على الشبكة الرئيسية، وتتبعها عبر Local-12 une
حتى تحتوي النطاقات العالمية على إدخالات التسجيل المقابلة.
ضع في اعتبارك عملية CLI كعامل تشغيل لهذا الجل - سلسلة الميمات
 يتم استخدام الإعلان في تلميحات الأدوات SDK والأتمتة من أجل
الحفاظ على التكافؤ مع معايير خارطة الطريق. الاستفادة من Torii
ما هو تطوير/اختبار المجموعات عند تشخيص الانحدارات. تابع أ
ميرويتر `torii_address_domain_total{domain_kind}` في Grafana
(`dashboards/grafana/address_ingest.json`) للحصول على حزمة الأمان ADDR-7
يمكنك رؤية ما إذا كان `domain_kind="local12"` يستقر على النافذة صفرًا
يقوم Alertmanager (`dashboards/alerts/address_ingest_rules.yml`) بإضافة ثلاث مرات
الحواجز:- `AddressLocal8Resurgence` كل صفحة تشير إلى سياق جديد
  الزيادة المحلية-8. قم بإيقاف عمليات الطرح في الوضع الصارم، وقم بتخصيصها
  Surface SDK الخاطئ في لوحة القيادة، وإذا كنت بحاجة إلى تحديد مؤقت
  الافتراضي (`true`).
- `AddressLocal12Collision` يتم حذفه من خلال التسميات المزدوجة Local-12 hashent
  مقابل ميمي هضم. Mettez enتوقف مؤقتًا عن الترقيات في البيان، قم بتنفيذها
  مجموعة الأدوات المحلية -> العالمية لتدقيق خرائط الهضم والتنسيق
  مع الإدارة Nexus قبل إعادة إدخال التسجيل أو
  قم بإعادة تشغيل عمليات الطرح بشكل فعال.
- `AddressInvalidRatioSlo` للحماية عندما تكون النسبة غير صالحة لمستوى الصف
  التعويم (باستثناء الإرجاعات المحلية 8/الوضع الصارم) يتجاوز SLO بنسبة 0.1%
  دقائق قلادة ديكس. استخدم `torii_address_invalid_total` للمعرف
  السياق/السبب المسؤول والتنسيق مع معدات SDK المملوكة مسبقًا
  de reenclencher le mode الصارم.

### ملحق مذكرة الإصدار (الحقيبة والمستكشف)

قم بتضمين الرصاصة التالية في ملاحظات الإصدار/المستكشف
lors du Cutover:> **العناوين:** Ajoute le helper `iroha tools address normalize --only-local --append-domain`
> والفرع في CI (`ci/check_address_normalize.sh`) لخطوط الأنابيب
> Portefeuille/explorateur puissent converter les Selecteurs Local Herites vers
> الأشكال canoniques IH58/compressees avant que Local-8/Local-12 soient
> كتل على الشبكة الرئيسية. قم بتخصيص الصادرات يوميًا لتنفيذها
> الأمر وضم القائمة الطبيعية إلى حزمة الإصدار المسبق.