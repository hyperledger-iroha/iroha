---
lang: es
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

importar ExplorerAddressCard desde '@site/src/components/ExplorerAddressCard';

:::nota المصدر القياسي
تعكس هذه الصفحة `docs/source/sns/address_display_guidelines.md` وتعمل الان
كمرجع بوابة موحد. يبقى ملف المصدر من اجل PRs الترجمة.
:::

يجب ان تتعامل المحافظ والمستكشفات وامثلة SDK مع عناوين الحساب كحمولات ثابتة لا
تغير. يعرض مثال محفظة Android في
`examples/android/retail-wallet` Configuración UX:- **هدفا نسخ منفصلان.** وفر زرين واضحين للنسخ: I105 (المفضل) والصيغة
  المضغوطة الخاصة بـ Sora (`sora...`، الخيار الثاني). I105 امن دائما للمشاركة خارجيا ويغذي
  حمولة QR. يجب ان تتضمن الصيغة المضغوطة تحذيرا مضمنا لانها تعمل فقط داخل
  تطبيقات واعية بـ Sora. مثال Android يربط زري Material y تلميحاتهما في
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`, y يطابق
  Para iOS SwiftUI y UX para `AddressPreviewCard`
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **خط ثابت ونص قابل للتحديد.** اعرض السلسلتين بخط monospace مع
  `textIsSelectable="true"` Haga clic en el enlace del IME.
  تجنب الحقول القابلة للتحرير: يمكن لـ IME اعادة كتابة kana او حقن نقاط كود بعرض
  صفر.
- **اشارات النطاق الافتراضي الضمني.** عندما يشير المحدد الى النطاق الضمني
  `default`، اعرض توضيحا يذكر المشغلين بان لا حاجة لاي لاحقة. يجب على
  المستكشفات ايضا تمييز تسمية النطاق القانونية عندما يشفر المحدد resumen.
- **حمولات QR I105.** يجب ان ترمز رموز QR سلسلة I105. اذا فشل توليد QR، اعرض
  خطا واضحا بدلا من صورة فارغة.
- **رسائل الحافظة.** بعد نسخ الصيغة المضغوطة، ارسل roast او snackbar يذكر
  المستخدمين انها خاصة بـ Sora ومعرضة لتشويه IME.

Utilice Unicode/IME y utilice el código ADDR-6
لتجربة محافظ/مستكشفات.

## لقطات مرجعية

استخدم اللقطات التالية خلال مراجعات الترجمة لضمان بقاء تسميات الازرار
والتلميحات والتحذيرات متوافقة عبر المنصات:

- Versión Android: `/img/sns/address_copy_android.svg`

  ![مرجع نسخ مزدوج Android](/img/sns/address_copy_android.svg)- Versión iOS: `/img/sns/address_copy_ios.svg`

  ![مرجع نسخ مزدوج iOS](/img/sns/address_copy_ios.svg)

## SDK de aplicaciones

El SDK incluye el I105 y la interfaz de usuario del dispositivo.
Nombre:

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- Inspector de JavaScript: `inspectAccountId(...)`
  الى `warnings` عندما يقدم المستدعون literal `sora...`, حتى يتمكن مستكشفو
  المحافظ/لوحات التحكم من عرض تحذير Sora-only اثناء تدفقات اللصق/التحقق بدلا
  من عرضه فقط عند توليد الصيغة المضغوطة ذاتيا.
- Pitón: `AccountAddress.display_formats(network_prefix: int = 753)`
- Rápido: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
-Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

Utilice la interfaz de usuario para acceder a la interfaz de usuario. يعرض مساعد
JavaScript incluye `selector` y `domainSummary` (`tag`, `digest_hex`,
`registry_id`, `label`) Interfaz de usuario de interfaz de usuario local-12
او مدعوما بسجل دون اعادة تحليل الحمولة الخام.

## عرض حي لادوات المستكشف



يجب ان تعكس المستكشفات اعمال القياس والاتاحة نفسها في المحافظ:- طبق `data-copy-mode="i105|qr"` على ازرار النسخ حتى تتمكن الواجهات
  الامامية من اصدار عدادات الاستخدام بالتوازي مع مقياس Torii
  `torii_address_format_total`. المكون التجريبي اعلاه يطلق حدث
  `iroha:address-copy` o `{mode,timestamp}` - اربط ذلك بخط تحليلاتك/تليمترتك
  (مثل ارسالها الى Segment او جامع NORITO) حتى تتمكن لوحات المتابعة من ربط
  استخدام صيغة العنوان على الخادم بوضع النسخ لدى العميل. اعكس ايضا عدادات نطاق
  Torii (`torii_address_domain_total{domain_kind}`) في نفس التدفق حتى تتمكن
  مراجعات تقاعد Local-12 من تصدير دليل 30 يوم `domain_kind="local12"` مباشرة من
  Aquí `address_ingest` o Grafana.
- اربط كل عنصر تحكم بتلميحات `aria-label`/`aria-describedby` مميزة تشرح ما اذا
  كانت السلسلة امنة للمشاركة (I105) او خاصة بـ Sora (مضغوطة). ادرج تسمية
  النطاق الضمني في الوصف حتى تعرض تقنيات المساعدة نفس السياق المرئي.
- وفر منطقة اعلان حية (مثل `<output aria-live="polite">...</output>`) تعلن
  نتائج النسخ والتحذيرات، بما يطابق سلوك VoiceOver/TalkBack الموصل بالفعل في
  Aplicación Swift/Android.

هذه الاتاحة تحقق ADDR-6b عبر اثبات قدرة المشغلين على مراقبة كل من ادخال Torii
واوضاع نسخ العميل قبل تعطيل محددات Local.

## عدة ترحيل Local -> Global

استخدم [عدة Local -> Global](local-to-global-toolkit.md) لادارة تدقيق وتحويل
محددات Local القديمة. يصدر المساعد تقرير تدقيق JSON y المحولة
I105/المضغوطة التي يرفقها المشغلون بتذاكر الجاهزية, بينما يربط دليل التشغيل
Utilice Grafana y Alertmanager para realizar la transición.## مرجع سريع للتخطيط الثنائي (ADDR-1a)

عندما تعرض SDK ادوات متقدمة للعناوين (المفتشون، تلميحات التحقق، بناة manifest)،
Utilice el cable de conexión `docs/account_structure.md`. تخطيط
Aquí `header · selector · controller`, hay un encabezado aquí:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (bits 7-5) Descripción القيم غير الصفرية محجوزة ويجب ان تؤدي
  Aquí `AccountAddressError::InvalidHeaderVersion`.
- `addr_class` يميز بين المتحكم الفردي (`0`) والمتعدد التواقيع (`1`).
- `norm_version = 1` يشفر قواعد محدد Norma v1. ستعيد المعايير المستقبلية استخدام
  نفس الحقل المكون من 2 بت.
- `ext_flag` Interior `0`؛ البتات المفعلة تشير الى امتدادات حمولة غير مدعومة.

يتبع المحدد مباشرة الـ encabezado:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

Esta es la interfaz de usuario y los SDK que aparecen en los siguientes enlaces:

- `0x00` = نطاق افتراضي ضمني (بدون حمولة).
- `0x01` = resumen de datos (`blake2s_mac("SORA-LOCAL-K:v1", label)` de 12 bytes).
- `0x02` = مدخل سجل عالمي (`registry_id:u32` big-endian).

امثلة hexadecimal قانونية يمكن لادوات المحافظ ربطها او ادراجها في documentos/pruebas:

| نوع المحدد | Hex قانوني |
|---------------|---------------|
| افتراضي ضمني | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| digerir محلي (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| سجل عالمي (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

راجع `docs/source/references/address_norm_v1.md` لجدول المحدد/الحالة الكامل و
`docs/account_structure.md` لمخطط البايتات الكامل.

## فرض الصيغ القانونية

يجب على المشغلين الذين يحولون ترميزات Local القديمة الى I105 قانوني او سلاسل
Utilice la CLI para acceder a ADDR-5:1. `iroha tools address inspect` Establece un código JSON en el I105 y el código fuente.
   والـ hexadecimal القانوني. El cable de alimentación es `domain` o `kind`/`warning`.
   Esta es la versión `input_domain`. عندما يكون `kind` y `local12`
   Instalar CLI en stderr y JSON y ejecutar CI y CI.
   SDK aquí. مرر `legacy  suffix` متى اردت اعادة تشغيل الترميز المحول
   كـ `<i105>@<domain>`.
2. Utilice los SDK para utilizar JavaScript:

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.i105.value, summary.i105Warning);
   ```
  يحافظ المساعد على بادئة I105 المكتشفة من literal ما لم تقدم `networkPrefix`
  صراحة, لذا لا تعاد صياغة الملخصات للشبكات غير الافتراضية بصمت مع بادئة افتراضية.3. حول الحمولة القانونية عبر اعادة استخدام حقول `i105.value`
   من الملخص (او اطلب ترميزا اخر عبر `--format`). هذه السلاسل امنة بالفعل
   للمشاركة خارجيا.
4. حدث manifiesta والسجلات والوثائق المواجهة للعميل بالصيغ القانونية وابلغ
   الاطراف المقابلة ان محددات Local سترفض بعد اكتمال Cutover.
5. لمجموعات البيانات الكبيرة, شغل
   `iroha tools address audit --input addresses.txt --network-prefix 753`. يقرأ الامر
   literales مفصولة باسطر جديدة (التعليقات التي تبدا بـ `#` يتم تجاهلها، و
   `--input -` (al igual que STDIN) y archivos JSON
   قانونية/I105/مضغوطة لكل ادخال، ويحسب اخطاء التحليل وتحذيرات نطاق Local. استخدم
   `--allow-errors` عند تدقيق dumps القديمة التي تحتوي صفوفا مهملة، واضبط
   الاتمتة عبر `strict CI post-check` حين يصبح المشغلون مستعدين لحظر محددات Local في CI.
6. عندما تحتاج لاعادة كتابة سطر بسطر، استخدم
  لملفات الجداول الخاصة بمعالجة محددات Local, استخدم
  Archivo CSV `input,status,format,...` Configuración y configuración
  واخفاقات التحليل في مرور واحد. يتخطى المساعد الصفوف غير المحلية افتراضيا،
  ويحول كل ادخال متبق الى الترميز المطلوب (I105/مضغوط/hex/JSON), y ويحافظ على
  النطاق الاصلي عندما يتم تعيين `legacy  suffix`. Fuente de `--allow-errors`
  لمواصلة الفحص حتى عند وجود literales تالفة.
7. Instale CI/lint en el archivo `ci/check_address_normalize.sh` para obtener más información
   Local de `fixtures/account/address_vectors.json`, y ويحولها عبر
   `iroha tools address normalize` ، ويعيد تشغيل
   `iroha tools address audit` لاثبات ان الاصدارات لم تعد تصدر resúmenesLocal.

`torii_address_local8_total{endpoint}` بالاضافة الى
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}`, y Grafana
`dashboards/grafana/address_ingest.json` توفر اشارة الالتزام: عندما تعرض لوحات
الانتاج صفرا من عمليات ارسال Local الشرعية وصفرا من تصادمات Local-12 لمدة 30 يوما
متتالية, سيحول Torii بوابة Local-8 الى فشل صارم على mainnet, يليه Local-12 بعد
ان تمتلك النطاقات العالمية ادخالات سجل مطابقة. اعتبر مخرجات CLI الاشعار الموجه
للمشغل لهذا التجميد - Información sobre herramientas de SDK y herramientas
للحفاظ على التوافق مع معايير الخروج في خارطة الطريق. يستخدم Torii الان افتراضيا
تشخيص regresiones. استمر في عكس `torii_address_domain_total{domain_kind}` الى
Grafana (`dashboards/grafana/address_ingest.json`) حتى يتمكن حزمة دليل ADDR-7 من
El producto `domain_kind="local12"` tiene una duración de 30 días.
تعطل mainnet المحددات القديمة. Administrador de alertas
(`dashboards/alerts/address_ingest_rules.yml`) Esta es la siguiente:- `AddressLocal8Resurgence` يستدعي عندما يبلغ سياق عن زيادة Local-8 جديدة. اوقف
  عمليات implementación للوضع الصارم، حدد سطح SDK المخالف في لوحة المتابعة، واضبط
  الافتراضي (`true`).
- `AddressLocal12Collision` يعمل عندما يقوم اسمان Local-12 بعمل hash الى نفس
  digerir. اوقف ترويج manifiestos, شغل عدة Local -> Global لتدقيق ربط resúmenes, و
  نسق مع حوكمة Nexus قبل اعادة اصدار ادخال السجل او اعادة تفعيل implementaciones
  aguas abajo.
- `AddressInvalidRatioSlo`
  الاسطول (باستثناء رفض Local-8/strict-mode) نسبة 0.1% لمدة عشر دقائق. استخدم
  `torii_address_invalid_total` Software de programación/escritura de software y SDK
  المالك قبل اعادة تفعيل الوضع الصارم.

### مقتطف مذكرة الاصدار (محفظة ومُستكشف)

ادرج النقطة التالية في ملاحظات اصدار المحفظة/المستكشف عند تنفيذ cutover:

> **العناوين:** تمت اضافة مساعد `iroha tools address normalize`
> وربطه في CI (`ci/check_address_normalize.sh`) حتى تتمكن مسارات المحفظة/المستكشف
> من تحويل محددات Local القديمة الى صيغ I105/مضغوطة قانونية قبل حظر Local-8/Local-12
> على red principal. حدث اي عمليات تصدير مخصصة لتشغيل الامر وارفق القائمة المعيارية
> بحزمة دليل الاصدار.