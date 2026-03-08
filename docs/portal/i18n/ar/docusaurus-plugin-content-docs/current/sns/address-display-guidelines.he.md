---
lang: he
direction: rtl
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/sns/address-display-guidelines.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b0acbfd6d6e581c26725d1afe6c516d1549f5e424b0115507b2542b782e3c753
source_last_modified: "2026-01-28T17:58:57+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: address-display-guidelines
lang: ar
direction: rtl
source: docs/portal/docs/sns/address-display-guidelines.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---
import ExplorerAddressCard from '@site/src/components/ExplorerAddressCard';

:::note المصدر القياسي
تعكس هذه الصفحة `docs/source/sns/address_display_guidelines.md` وتعمل الان
كمرجع بوابة موحد. يبقى ملف المصدر من اجل PRs الترجمة.
:::

يجب ان تتعامل المحافظ والمستكشفات وامثلة SDK مع عناوين الحساب كحمولات ثابتة لا
تتغير. يعرض مثال محفظة Android في
`examples/android/retail-wallet` نمط UX المطلوب:

- **هدفا نسخ منفصلان.** وفر زرين واضحين للنسخ: IH58 (المفضل) والصيغة
  المضغوطة الخاصة بـ Sora (`sora...`، الخيار الثاني). IH58 امن دائما للمشاركة خارجيا ويغذي
  حمولة QR. يجب ان تتضمن الصيغة المضغوطة تحذيرا مضمنا لانها تعمل فقط داخل
  تطبيقات واعية بـ Sora. مثال Android يربط زري Material وتلميحاتهما في
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`، ويطابق
  عرض iOS SwiftUI نفس UX عبر `AddressPreviewCard` داخل
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **خط ثابت ونص قابل للتحديد.** اعرض السلسلتين بخط monospace مع
  `textIsSelectable="true"` حتى يتمكن المستخدمون من فحص القيم دون تشغيل IME.
  تجنب الحقول القابلة للتحرير: يمكن لـ IME اعادة كتابة kana او حقن نقاط كود بعرض
  صفر.
- **اشارات النطاق الافتراضي الضمني.** عندما يشير المحدد الى النطاق الضمني
  `default`، اعرض توضيحا يذكر المشغلين بان لا حاجة لاي لاحقة. يجب على
  المستكشفات ايضا تمييز تسمية النطاق القانونية عندما يشفر المحدد digest.
- **حمولات QR IH58.** يجب ان ترمز رموز QR سلسلة IH58. اذا فشل توليد QR، اعرض
  خطا واضحا بدلا من صورة فارغة.
- **رسائل الحافظة.** بعد نسخ الصيغة المضغوطة، ارسل toast او snackbar يذكر
  المستخدمين انها خاصة بـ Sora ومعرضة لتشويه IME.

اتباع هذه الضوابط يمنع فساد Unicode/IME ويلبي معايير قبول خارطة الطريق ADDR-6
لتجربة محافظ/مستكشفات.

## لقطات مرجعية

استخدم اللقطات التالية خلال مراجعات الترجمة لضمان بقاء تسميات الازرار
والتلميحات والتحذيرات متوافقة عبر المنصات:

- مرجع Android: `/img/sns/address_copy_android.svg`

  ![مرجع نسخ مزدوج Android](/img/sns/address_copy_android.svg)

- مرجع iOS: `/img/sns/address_copy_ios.svg`

  ![مرجع نسخ مزدوج iOS](/img/sns/address_copy_ios.svg)

## مساعدات SDK

كل SDK يوفر مساعدا يعيد صيغتي IH58 والمضغوطة مع سلسلة التحذير حتى تبقى طبقات UI
متسقة:

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- JavaScript inspector: `inspectAccountId(...)` يعيد سلسلة تحذير مضغوطة ويضيفها
  الى `warnings` عندما يقدم المستدعون literal `sora...`، حتى يتمكن مستكشفو
  المحافظ/لوحات التحكم من عرض تحذير Sora-only اثناء تدفقات اللصق/التحقق بدلا
  من عرضه فقط عند توليد الصيغة المضغوطة ذاتيا.
- Python: `AccountAddress.display_formats(network_prefix: int = 753)`
- Swift: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
- Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

استخدم هذه المساعدات بدلا من اعادة تنفيذ منطق الترميز في طبقات UI. يعرض مساعد
JavaScript ايضا حمولة `selector` على `domainSummary` (`tag`, `digest_hex`,
`registry_id`, `label`) حتى تتمكن واجهات UI من تحديد ما اذا كان المحدد Local-12
او مدعوما بسجل دون اعادة تحليل الحمولة الخام.

## عرض حي لادوات المستكشف

<ExplorerAddressCard />

يجب ان تعكس المستكشفات اعمال القياس والاتاحة نفسها في المحافظ:

- طبق `data-copy-mode="ih58|compressed|qr"` على ازرار النسخ حتى تتمكن الواجهات
  الامامية من اصدار عدادات الاستخدام بالتوازي مع مقياس Torii
  `torii_address_format_total`. المكون التجريبي اعلاه يطلق حدث
  `iroha:address-copy` مع `{mode,timestamp}` - اربط ذلك بخط تحليلاتك/تليمترتك
  (مثل ارسالها الى Segment او جامع NORITO) حتى تتمكن لوحات المتابعة من ربط
  استخدام صيغة العنوان على الخادم بوضع النسخ لدى العميل. اعكس ايضا عدادات نطاق
  Torii (`torii_address_domain_total{domain_kind}`) في نفس التدفق حتى تتمكن
  مراجعات تقاعد Local-12 من تصدير دليل 30 يوم `domain_kind="local12"` مباشرة من
  لوحة `address_ingest` في Grafana.
- اربط كل عنصر تحكم بتلميحات `aria-label`/`aria-describedby` مميزة تشرح ما اذا
  كانت السلسلة امنة للمشاركة (IH58) او خاصة بـ Sora (مضغوطة). ادرج تسمية
  النطاق الضمني في الوصف حتى تعرض تقنيات المساعدة نفس السياق المرئي.
- وفر منطقة اعلان حية (مثل `<output aria-live="polite">...</output>`) تعلن
  نتائج النسخ والتحذيرات، بما يطابق سلوك VoiceOver/TalkBack الموصل بالفعل في
  امثلة Swift/Android.

هذه الاتاحة تحقق ADDR-6b عبر اثبات قدرة المشغلين على مراقبة كل من ادخال Torii
واوضاع نسخ العميل قبل تعطيل محددات Local.

## عدة ترحيل Local -> Global

استخدم [عدة Local -> Global](local-to-global-toolkit.md) لادارة تدقيق وتحويل
محددات Local القديمة. يصدر المساعد تقرير تدقيق JSON والقائمة المحولة
IH58/المضغوطة التي يرفقها المشغلون بتذاكر الجاهزية، بينما يربط دليل التشغيل
لوحات Grafana وقواعد Alertmanager التي تضبط cutover في الوضع الصارم.

## مرجع سريع للتخطيط الثنائي (ADDR-1a)

عندما تعرض SDKs ادوات متقدمة للعناوين (المفتشون، تلميحات التحقق، بناة manifest)،
اشِر المطورين الى صيغة wire القانونية في `docs/account_structure.md`. التخطيط
دائما `header · selector · controller`، حيث بتات header هي:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (bits 7-5) اليوم؛ القيم غير الصفرية محجوزة ويجب ان تؤدي
  الى `AccountAddressError::InvalidHeaderVersion`.
- `addr_class` يميز بين المتحكم الفردي (`0`) والمتعدد التواقيع (`1`).
- `norm_version = 1` يشفر قواعد محدد Norm v1. ستعيد المعايير المستقبلية استخدام
  نفس الحقل المكون من 2 بت.
- `ext_flag` دائما `0`؛ البتات المفعلة تشير الى امتدادات حمولة غير مدعومة.

يتبع المحدد مباشرة الـ header:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

يجب ان تكون واجهات UI وSDKs جاهزة لعرض نوع المحدد:

- `0x00` = نطاق افتراضي ضمني (بدون حمولة).
- `0x01` = digest محلي (12-byte `blake2s_mac("SORA-LOCAL-K:v1", label)`).
- `0x02` = مدخل سجل عالمي (`registry_id:u32` big-endian).

امثلة hex قانونية يمكن لادوات المحافظ ربطها او ادراجها في docs/tests:

| نوع المحدد | Hex قانوني |
|---------------|---------------|
| افتراضي ضمني | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| digest محلي (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| سجل عالمي (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

راجع `docs/source/references/address_norm_v1.md` لجدول المحدد/الحالة الكامل و
`docs/account_structure.md` لمخطط البايتات الكامل.

## فرض الصيغ القانونية

يجب على المشغلين الذين يحولون ترميزات Local القديمة الى IH58 قانوني او سلاسل
مضغوطة اتباع مسار CLI الموثق تحت ADDR-5:

1. `iroha tools address inspect` يصدر الان ملخص JSON منظم مع IH58 والحمولة المضغوطة
   والـ hex القانوني. يتضمن الملخص ايضا كائن `domain` مع حقول `kind`/`warning`
   ويعكس اي نطاق مقدم عبر الحقل `input_domain`. عندما يكون `kind` هو `local12`
   تطبع CLI تحذيرا على stderr ويعكس ملخص JSON نفس التوجيه حتى تتمكن خطوط CI و
   SDKs من عرضه. مرر `legacy  suffix` متى اردت اعادة تشغيل الترميز المحول
   كـ `<ih58>@<domain>`.
2. يمكن لـ SDKs عرض نفس التحذير/الملخص عبر مساعد JavaScript:

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed);
   ```
  يحافظ المساعد على بادئة IH58 المكتشفة من literal ما لم تقدم `networkPrefix`
  صراحة، لذا لا تعاد صياغة الملخصات للشبكات غير الافتراضية بصمت مع بادئة افتراضية.

3. حول الحمولة القانونية عبر اعادة استخدام حقول `ih58.value` او `compressed`
   من الملخص (او اطلب ترميزا اخر عبر `--format`). هذه السلاسل امنة بالفعل
   للمشاركة خارجيا.
4. حدث manifests والسجلات والوثائق المواجهة للعميل بالصيغ القانونية وابلغ
   الاطراف المقابلة ان محددات Local سترفض بعد اكتمال cutover.
5. لمجموعات البيانات الكبيرة، شغل
   `iroha tools address audit --input addresses.txt --network-prefix 753`. يقرأ الامر
   literals مفصولة باسطر جديدة (التعليقات التي تبدا بـ `#` يتم تجاهلها، و
   `--input -` او عدم وجود علم يستخدم STDIN)، ويصدر تقرير JSON بملخصات
   قانونية/IH58/مضغوطة لكل ادخال، ويحسب اخطاء التحليل وتحذيرات نطاق Local. استخدم
   `--allow-errors` عند تدقيق dumps القديمة التي تحتوي صفوفا مهملة، واضبط
   الاتمتة عبر `strict CI post-check` حين يصبح المشغلون مستعدين لحظر محددات Local في CI.
6. عندما تحتاج لاعادة كتابة سطر بسطر، استخدم
  لملفات الجداول الخاصة بمعالجة محددات Local، استخدم
  لتصدير CSV `input,status,format,...` يبرز الترميزات القانونية والتحذيرات
  واخفاقات التحليل في مرور واحد. يتخطى المساعد الصفوف غير المحلية افتراضيا،
  ويحول كل ادخال متبق الى الترميز المطلوب (IH58/مضغوط/hex/JSON)، ويحافظ على
  النطاق الاصلي عندما يتم تعيين `legacy  suffix`. اقرنه مع `--allow-errors`
  لمواصلة الفحص حتى عند وجود literals تالفة.
7. يمكن لاتمتة CI/lint تشغيل `ci/check_address_normalize.sh` الذي يستخرج محددات
   Local من `fixtures/account/address_vectors.json`، ويحولها عبر
   `iroha tools address normalize`، ويعيد تشغيل
   `iroha tools address audit` لاثبات ان الاصدارات لم تعد تصدر digests
   Local.

`torii_address_local8_total{endpoint}` بالاضافة الى
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}`, ولوحة Grafana
`dashboards/grafana/address_ingest.json` توفر اشارة الالتزام: عندما تعرض لوحات
الانتاج صفرا من عمليات ارسال Local الشرعية وصفرا من تصادمات Local-12 لمدة 30 يوما
متتالية، سيحول Torii بوابة Local-8 الى فشل صارم على mainnet، يليه Local-12 بعد
ان تمتلك النطاقات العالمية ادخالات سجل مطابقة. اعتبر مخرجات CLI الاشعار الموجه
للمشغل لهذا التجميد - نفس سلسلة التحذير تستخدم عبر tooltips في SDK والاتمتة
للحفاظ على التوافق مع معايير الخروج في خارطة الطريق. يستخدم Torii الان افتراضيا
تشخيص regressions. استمر في عكس `torii_address_domain_total{domain_kind}` الى
Grafana (`dashboards/grafana/address_ingest.json`) حتى يتمكن حزمة دليل ADDR-7 من
اثبات ان `domain_kind="local12"` بقيت صفرا خلال نافذة 30 يوما المطلوبة قبل ان
تعطل mainnet المحددات القديمة. حزمة Alertmanager
(`dashboards/alerts/address_ingest_rules.yml`) تضيف ثلاث حواجز:

- `AddressLocal8Resurgence` يستدعي عندما يبلغ سياق عن زيادة Local-8 جديدة. اوقف
  عمليات rollout للوضع الصارم، حدد سطح SDK المخالف في لوحة المتابعة، واضبط
  الافتراضي (`true`).
- `AddressLocal12Collision` يعمل عندما يقوم اسمان Local-12 بعمل hash الى نفس
  digest. اوقف ترويج manifests، شغل عدة Local -> Global لتدقيق ربط digests، و
  نسق مع حوكمة Nexus قبل اعادة اصدار ادخال السجل او اعادة تفعيل rollouts
  downstream.
- `AddressInvalidRatioSlo` يحذر عندما يتجاوز معدل عدم الصلاحية على مستوى
  الاسطول (باستثناء رفض Local-8/strict-mode) نسبة 0.1% لمدة عشر دقائق. استخدم
  `torii_address_invalid_total` لتحديد السياق/السبب المسؤول ونسق مع فريق SDK
  المالك قبل اعادة تفعيل الوضع الصارم.

### مقتطف مذكرة الاصدار (محفظة ومُستكشف)

ادرج النقطة التالية في ملاحظات اصدار المحفظة/المستكشف عند تنفيذ cutover:

> **العناوين:** تمت اضافة مساعد `iroha tools address normalize`
> وربطه في CI (`ci/check_address_normalize.sh`) حتى تتمكن مسارات المحفظة/المستكشف
> من تحويل محددات Local القديمة الى صيغ IH58/مضغوطة قانونية قبل حظر Local-8/Local-12
> على mainnet. حدث اي عمليات تصدير مخصصة لتشغيل الامر وارفق القائمة المعيارية
> بحزمة دليل الاصدار.
