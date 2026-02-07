---
lang: fr
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

importer ExplorerAddressCard depuis '@site/src/components/ExplorerAddressCard' ;

:::note المصدر القياسي
تعكس هذه الصفحة `docs/source/sns/address_display_guidelines.md` et تعمل الان
كمرجع بوابة موحد. يبقى ملف المصدر من اجل PRs الترجمة.
:::

يجب ان تتعامل المحافظ والمستكشفات وامثلة SDK مع عناوين الحساب كحمولات ثابتة لا
تتغير. يعرض مثال محفظة Android في
`examples/android/retail-wallet` pour l'UX :- **هدفا نسخ منفصلان.** وفر زرين واضحين للنسخ: IH58 (المفضل) والصيغة
  Lien vers Sora (`sora...`) sur Sora. IH58 امن دائما للمشاركة خارجيا ويغذي
  حمولة QR. يجب ان تتضمن الصيغة المضغوطة تحذيرا مضمنا لانها تعمل فقط داخل
  تطبيقات واعية بـ Sora. مثال Android يربط زري Material وتلميحاتهما في
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`, ويطابق
  iOS SwiftUI est compatible avec UX avec `AddressPreviewCard`.
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **خط ثابت ونص قابل للتحديد.** اعرض السلسلتين بخط monospace مع
  `textIsSelectable="true"` حتى يتمكن المستخدمون من فحص القيم دون تشغيل IME.
  تجنب الحقول القابلة للتحرير: يمكن لـ IME اعادة كتابة kana او حقن نقاط كود بعرض
  bien.
- **اشارات النطاق الافتراضي الضمني.** عندما يشير المحدد الى النطاق الضمني
  `default`, اعرض توضيحا يذكر المشغلين بان لا حاجة لاي لاحقة. يجب على
  المستكشفات ايضا تمييز تسمية النطاق القانونية عندما يشفر المحدد digest.
- **حمولات QR IH58.** يجب ان ترمز رموز QR سلسلة IH58. اذا فشل توليد QR، اعرض
  خطا واضحا بدلا من صورة فارغة.
- **رسائل الحافظة.** بعد نسخ الصيغة المضغوطة، ارسل toasts et snack-bar يذكر
  المستخدمين انها خاصة بـ Sora ومعرضة لتشويه IME.

Vous pouvez utiliser Unicode/IME et ajouter ADDR-6
لتجربة محافظ/مستكشفات.

## لقطات مرجعية

استخدم اللقطات التالية خلال مراجعات الترجمة لضمان بقاء تسميات الازرار
والتلميحات والتحذيرات متوافقة عبر المنصات:

- Version Android : `/img/sns/address_copy_android.svg`

  ![مرجع نسخ مزدوج Android](/img/sns/address_copy_android.svg)- Version iOS : `/img/sns/address_copy_ios.svg`

  ![مرجع نسخ مزدوج iOS](/img/sns/address_copy_ios.svg)

## SDK de mise à jour

Le SDK est également compatible avec IH58 et avec l'interface utilisateur
متسقة:

- Javascript : `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- Inspecteur JavaScript : `inspectAccountId(...)` يعيد سلسلة تحذير مضغوطة ويضيفها
  الى `warnings` عندما يقدم المستدعون littéral `sora...`, حتى يتمكن مستكشفو
  المحافظ/لوحات التحكم من عرض تحذير Sora-only اثناء تدفقات اللصق/التحقق بدلا
  من عرضه فقط عند توليد الصيغة المضغوطة ذاتيا.
-Python : `AccountAddress.display_formats(network_prefix: int = 753)`
-Swift : `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
-Java/Kotlin : `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

Il s'agit d'une application pour l'interface utilisateur. يعرض مساعد
JavaScript est utilisé pour `selector` ou `domainSummary` (`tag`, `digest_hex`,
`registry_id`, `label`) Utilisez l'interface utilisateur pour vous connecter à Local-12
او مدعوما بسجل دون اعادة تحليل الحمولة الخام.

## عرض حي لادوات المستكشف



يجب ان تعكس المستكشفات اعمال القياس والاتاحة نفسها في المحافظ:- طبق `data-copy-mode="ih58|compressed|qr"` على ازرار النسخ حتى تتمكن الواجهات
  الامامية من اصدار عدادات الاستخدام بالتوازي مع مقياس Torii
  `torii_address_format_total`. المكون التجريبي اعلاه يطلق حدث
  `iroha:address-copy` vers `{mode,timestamp}` - اربط ذلك بخط تحليلاتك/تليمترتك
  (مثل ارسالها الى Segment او جامع NORITO) حتى تتمكن لوحات المتابعة من ربط
  استخدام صيغة العنوان على الخادم بوضع النسخ لدى العميل. اعكس ايضا عدادات نطاق
  Torii (`torii_address_domain_total{domain_kind}`) pour plus de détails
  مراجعات تقاعد Local-12 من تصدير دليل 30 يوم `domain_kind="local12"` مباشرة من
  Pour `address_ingest` à Grafana.
- اربط كل عنصر تحكم بتلميحات `aria-label`/`aria-describedby` مميزة تشرح ما اذا
  كانت السلسلة امنة للمشاركة (IH58) et خاصة by Sora (مضغوطة). ادرج تسمية
  النطاق الضمني في الوصف حتى تعرض تقنيات المساعدة نفس السياق المرئي.
- وفر منطقة اعلان حية (مثل `<output aria-live="polite">...</output>`) تعلن
  Comment utiliser VoiceOver/TalkBack pour utiliser VoiceOver/TalkBack
  Utiliser Swift/Android.

L'ADDR-6b est une option pour la connexion avec le module Torii.
واوضاع نسخ العميل قبل تعطيل محددات Local.

## عدة ترحيل Local -> Global

استخدم [عدة Local -> Global](local-to-global-toolkit.md) لادارة تدقيق وتحويل
محددات Local القديمة. Utiliser JSON et utiliser JSON pour créer des liens
IH58/المضغوطة التي يرفقها المشغلون بتذاكر الجاهزية، بينما يربط دليل التشغيل
Le Grafana et Alertmanager permettent le basculement vers la version ultérieure.## مرجع سريع للتخطيط الثنائي (ADDR-1a)

Les SDK sont également compatibles avec le manifeste (manifeste).
Il s'agit d'un fil métallique `docs/account_structure.md`. التخطيط
Utilisez `header · selector · controller` pour l'en-tête :

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (bits 7-5) القيم غير الصفرية محجوزة ويجب ان تؤدي
  Dans `AccountAddressError::InvalidHeaderVersion`.
- `addr_class` يميز بين المتحكم الفردي (`0`) et والمتعدد التواقيع (`1`).
- `norm_version = 1` correspond à la norme v1. ستعيد المعايير المستقبلية استخدام
  نفس الحقل المكون من 2 بت.
- `ext_flag` pour `0`؛ البتات المفعلة تشير الى امتدادات حمولة غير مدعومة.

يتبع المحدد مباشرة الـ en-tête :

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

Voici quelques-unes des interfaces utilisateur et des SDK qui sont disponibles :

- `0x00` = نطاق افتراضي ضمني (بدون حمولة).
- `0x01` = résumé محلي (`blake2s_mac("SORA-LOCAL-K:v1", label)` 12 octets).
- `0x02` = مدخل سجل عالمي (`registry_id:u32` big-endian).

امثلة hex قانونية يمكن لادوات المحافظ ربطها او ادراجها في docs/tests :

| نوع المحدد | Hex قانوني |
|--------------------|---------------|
| افتراضي ضمني | `0x02000001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| résumé محلي (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| سجل عالمي (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

راجع `docs/source/references/address_norm_v1.md` لجدول المحدد/الحالة الكامل و
`docs/account_structure.md` لمخطط البايتات الكامل.

## فرض الصيغ القانونية

يجب على المشغلين الذين يحولون ترميزات Local القديمة الى IH58 قانوني او سلاسل
Utilisez la CLI pour ADDR-5 :1. `iroha tools address inspect` utilise JSON pour IH58 et IH58.
   والـ hex القانوني. يتضمن الملخص ايضا كائن `domain` avec `kind`/`warning`
   ويعكس اي نطاق مقدم عبر الحقل `input_domain`. Pour `kind` et `local12`
   Utilisez CLI pour stderr et JSON pour utiliser CI et CI.
   Les SDK sont disponibles. مرر `--append-domain` متى اردت اعادة تشغيل الترميز المحول
   Voir `<ih58>@<domain>`.
2. Les SDK sont également compatibles avec JavaScript :

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed);
   ```
  يحافظ المساعد على بادئة IH58 المكتشفة من littéral ما لم تقدم `networkPrefix`
  صراحة، لذا لا تعاد صياغة الملخصات للشبكات غير الافتراضية بصمت مع بادئة افتراضية.3. حول الحمولة القانونية عبر اعادة استخدام حقول `ih58.value` et `compressed`
   من الملخص (او اطلب ترميزا اخر عبر `--format`). هذه السلاسل امنة بالفعل
   للمشاركة خارجيا.
4. حدث manifeste والسجلات والوثائق المواجهة للعميل بالصيغ القانونية وابلغ
   Il s'agit d'un cutover local.
5. لمجموعات البيانات الكبيرة، شغل
   `iroha tools address audit --input addresses.txt --network-prefix 753`. يقرأ الامر
   littéraux مفصولة باسطر جديدة (التعليقات التي تبدا بـ `#` يتم تجاهلها، و
   `--input -` et la version STDIN) et la version JSON
   قانونية/IH58/مضغوطة لكل ادخال، ويحسب اخطاء التحليل وتحذيرات نطاق Local. استخدم
   `--allow-errors` عند تدقيق dumps القديمة التي تحتوي صفوفا مهملة، واضبط
   La clé `--fail-on-warning` est une clé pour la connexion locale à CI.
6. عندما تحتاج لاعادة كتابة سطر بسطر، استخدم
  لملفات الجداول الخاصة بمعالجة محددات Local, استخدم
  لتصدير CSV `input,status,format,...` يبرز الترميزات القانونية والتحذيرات
  واخفاقات التحليل في مرور واحد. يتخطى المساعد الصفوف غير المحلية افتراضيا،
  ويحول ادخال متبق الى الترميز المطلوب (IH58/مضغوط/hex/JSON) et ويحافظ على
  La référence est `--append-domain`. اقرنه مع `--allow-errors`
  لمواصلة الفحص حتى عند وجود littéraux تالفة.
7. يمكن لاتمتة CI/lint تشغيل `ci/check_address_normalize.sh` الذي يستخرج محددات
   Local من `fixtures/account/address_vectors.json`, ويحولها عبر
   `iroha tools address normalize`, ويعيد تشغيل
   `iroha tools address audit --fail-on-warning` لاثبات ان الاصدارات لم تعد تصدر digestsLocale.

`torii_address_local8_total{endpoint}` pour la lecture
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}`, et Grafana
`dashboards/grafana/address_ingest.json` توفر اشارة التزام: عندما تعرض لوحات
الانتاج صفرا من عمليات ارسال Local الشرعية وصفرا من تصادمات Local-12 du 30 janvier
Utilisez Torii pour Local-8 comme pour le réseau principal, ou Local-12 pour
ان تمتلك النطاقات العالمية ادخالات سجل مطابقة. اعتبر مخرجات CLI الاشعار الموجه
للمشغل لهذا التجميد - نفس سلسلة التحذير تستخدم عبر info-bulles du SDK et des info-bulles
للحفاظ على التوافق مع معايير الخروج في خارطة الطريق. يستخدم Torii الان افتراضيا
تشخيص régressions. استمر في عكس `torii_address_domain_total{domain_kind}` ى
Grafana (`dashboards/grafana/address_ingest.json`) est utilisé pour ADDR-7
اثبات ان `domain_kind="local12"` depuis 30 ans
تعطل mainnet المحددات القديمة. Télécharger Alertmanager
(`dashboards/alerts/address_ingest_rules.yml`) تضيف ثلاث حواجز :- `AddressLocal8Resurgence` est connecté à Local-8. اوقف
  Le déploiement du SDK est également possible avec le SDK et les fonctionnalités de déploiement.
  الافتراضي (`true`).
- `AddressLocal12Collision` prend en charge la fonction de hachage Local-12
  digérer. اوقف ترويج manifestes, شغل عدة Local -> Global لتدقيق ربط digests, و
  Utilisez le Nexus pour les déploiements de déploiement
  en aval.
- `AddressInvalidRatioSlo` يحذر عندما يتجاوز معدل عدم الصلاحية على مستوى
  La valeur (en mode Local-8/strict-mode) est de 0,1% par rapport à la valeur locale. استخدم
  `torii_address_invalid_total` Téléchargement/Détails du SDK
  المالك قبل اعادة تفعيل الوضع الصارم.

### مقتطف مذكرة الاصدار (محفظة ومُستكشف)

La transition vers le cutover est la suivante :

> **العناوين:** تمت اضافة مساعد `iroha tools address normalize --only-local --append-domain`
> وربطه في CI (`ci/check_address_normalize.sh`) حتى تتمكن مسارات المحفظة/المستكشف
> من تحويل محدات Local القديمة الى صيغ IH58/مضغوطة قانونية قبل حظر Local-8/Local-12
> على réseau principal. حدث اي عمليات تصدير مخصصة لتشغيل الامر وارفق القائمة المعيارية
> بحزمة دليل الاصدار.