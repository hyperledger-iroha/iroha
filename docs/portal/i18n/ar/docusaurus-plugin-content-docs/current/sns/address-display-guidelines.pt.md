---
lang: ar
direction: rtl
source: docs/portal/docs/sns/address-display-guidelines.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

استيراد ExplorerAddressCard من '@site/src/components/ExplorerAddressCard'؛

:::ملاحظة فونتي كانونيكا
هذه الصفحة espelha `docs/source/sns/address_display_guidelines.md` e Agora تخدم
كنسخة من بوابة Canonica. الملف الدائم للعلاقات العامة
com.traducao.
:::

يمكن للبطاقات والمستكشفين ونماذج SDK أن تسهل عمليات حسابية مماثلة
الحمولات النافعة. نموذج لبطاقة البيع بالتجزئة Android em
`examples/android/retail-wallet` يعرض الآن لوحة تجربة المستخدم الرائعة:- **قم بالنسخ مرة أخرى.** أرسل اثنين من نسخ النسخ الصريحة: I105
  (مفضل) وشكل مشترك من Sora (`sora...`، أفضل خيار ثاني). I105 و semper seguro para
  قم بمشاركة الطعام والطعام الخارجي أو الحمولة عبر QR. مجموعة متنوعة
  يجب أن يتضمن ذلك نصيحة مضمنة حتى تعمل داخل التطبيقات المتوافقة مع com
  سورا. نموذج لبطاقة البيع بالتجزئة Android liga التي تحتوي على الأحذية والمواد الخاصة بها
  تلميحات الأدوات م
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`، ه أ
  العرض التوضيحي لنظام التشغيل iOS SwiftUI يستكشف أو يستخدم تجربة المستخدم عبر `AddressPreviewCard` em
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **Monospace، texto selecionavel.** تحويل Ambas كسلاسل مع خط
  monospace e `textIsSelectable="true"` حتى يتمكن المستخدمون من استكشافه
  يتم استدعاء القيم في IME. تجنب تحرير المجالات: يمكنك إعادة تشغيل محررات أسلوب الإدخال (IMEs).
  أو injetar pontos de codego de largura صفر.
- **مبادئ السيادة الضمنية.** عند اختيار السيادة
  `default` الضمني، هو أحد المشغلين الأسطوريين الذين لا يفعلون ذلك
  لاحقة وضرورية. يجب على المستكشفين أيضًا التخلص من علامة السيادة
  Canonico عندما يتم اختيار الكود أو الملخص.
- **QR I105.** يقوم Codigos QR بترميز سلسلة I105. إذا قمت بذلك عن طريق QR
  لسوء الحظ، يوجد خطأ واضح في صورة بيضاء.
- **رسالة في منطقة النقل.** بعد النسخ بطريقة مكتوبة،قم بإصدار خبز محمص أو وجبة خفيفة من مستخدميها من que ela e somente Sora e
  دعم التشويش بواسطة IME.

اتبع هذه الدرابزين لتجنب تلف Unicode/IME والتزم بمعايير
استخدم خريطة الطريق ADDR-6 لـ UX للبطاقات/المستكشفين.

## لقطات الشاشة المرجعية

استخدم كمراجع لتتبع مراجعات الموقع لضمان أن نظام التشغيل
يتم عرض مجموعات الأحذية وتلميحات الأدوات والنصائح بين المنصات:

- مرجع أندرويد: `/img/sns/address_copy_android.svg`

  ![مرجع Android مزدوج النسخ](/img/sns/address_copy_android.svg)

- المرجع iOS: `/img/sns/address_copy_ios.svg`

  ![مرجع iOS مزدوج النسخ](/img/sns/address_copy_ios.svg)

## مساعدات SDK

توفر كل SDK أداة مساعدة مريحة من خلال إعادة تنسيق I105 والاشتراك فيها
بالإضافة إلى سلسلة من النصائح التي تجعل واجهة المستخدم متسقة:

- جافا سكريبت: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- مفتش جافا سكريبت: `inspectAccountId(...)` يعيد سلسلة التوجيه
  قم بالاشتراك والربط بـ `warnings` عند التواصل معنا حرفيًا
  `sora...`، حتى تتمكن لوحات المعلومات/المستكشف من العرض أو التحذير
  Somente Sora durante Fluxos de Collagem/Validacao Em Vez de Apenas Quando Geram
  شكل من أشكال الشراكة من خلال كونتا بروبريا.
- بايثون: `AccountAddress.display_formats(network_prefix: int = 753)`
- سويفت: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
- جافا/كوتلين: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)استخدم هذه الأدوات المساعدة عند إعادة تنفيذ منطق التشفير في واجهة المستخدم الخاصة بك. يا
يعرض مساعد JavaScript الحمولة `selector` في `domainSummary` (`tag`,
`digest_hex`، `registry_id`، `label`) لتشير واجهات المستخدم إلى المحدد
Local-12 أو يتم إصلاحه من خلال التسجيل دون إصلاح الحمولة الكاملة.

## عرض تجريبي لأداة المستكشف



يجب على المستكشفين أن يستكشفوا أو يعملون في مجال القياس عن بعد وإمكانية الوصول
كارتيرا:- قم بنسخ `data-copy-mode="i105|qr"` لنسخ الأحذية لذلك
  يمكن للواجهات الأمامية أن تُصدر قواطع الاستخدام جنبًا إلى جنب مع المقياس Torii
  `torii_address_format_total`. هذا الجزء من العرض التوضيحي يختلف عن الحدث
  `iroha:address-copy` كوم `{mode,timestamp}`; قم بتوصيل خط الأنابيب الخاص بك
  التحليل/القياس عن بعد (على سبيل المثال، إرسال المقطع أو جامع NORITO)
  لكي تتمكن لوحات المعلومات من الارتباط أو استخدام التنسيقات الداخلية
  الخادم مع طرق نسخ العميل. Replique tambem os contadores de
  dominio Torii (`torii_address_domain_total{domain_kind}`) لا يوجد تغذية mesmo لـ
  يمكن لمراجعة التفويض المحلي 12 تصدير تجربة في 30 يومًا
  `domain_kind="local12"` للطلاء مباشرة `address_ingest` لـ Grafana.
- Emparelhe cada controle com pistas `aria-label`/`aria-describedby` distintas
  يشرح ما إذا كان حرفيًا وآمنًا للمشاركة (I105) أو بعض سورا
  (كومبريميدو). قم بتضمين أسطورة السيادة الضمنية التي تصفها
  التكنولوجيا هي الأكثر مساعدة أو في نفس السياق الذي يعرضه بصريًا.
- معرض الحياة الإقليمية (على سبيل المثال، `<output aria-live="polite">...</output>`)
  الإعلان عن نتائج النسخ والنصائح، بالإضافة إلى السلوك
  VoiceOver/TalkBack متصل بنماذج Swift/Android.

هذه الأداة مرضية ADDR-6b لتختبر ما يمكن للمشغلين مراقبته
من أجل استيعاب Torii من حيث طرق النسخ التي يقوم بها العميل قبل ذلك
اختيارات محلية محظورة.## مجموعة أدوات الهجرة المحلية -> العالمية

استخدم [مجموعة الأدوات المحلية -> العالمية](local-to-global-toolkit.md) للتشغيل التلقائي
مراجعة ومحادثة البدائل المحلية المختارة. يا مساعد تنبعث من طنو أو صلة
JSON de Audition Quanto a lista Convertida I105/comprimida que Operadores
قم باختبار تذاكر الاستعداد أثناء أو دليل التشغيل المرتبط بلوحات المعلومات
Grafana وقم بإعادة تنبيه المدير للتحكم في عملية القطع في الوضع الافتراضي.

## المرجع السريع للتخطيط الثنائي (ADDR-1a)

عندما تعرض أدوات تطوير البرمجيات (SDKs) الأدوات المتقدمة للمباحث (المفتشون، يقولون
التحقق من الصحة، صانعو البيان)، مصممون لتنسيق الأسلاك
كانونيكو إم `docs/account_structure.md`. يا تخطيط دائم ه
`header · selector · controller`، تقوم وحدات بت نظام التشغيل بالرأس:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (البتات 7-5) ؛ القيم لا صفر ساو محفوظة ه ديم
  ليفانتار `AccountAddressError::InvalidHeaderVersion`.
- `addr_class` يميز وحدات التحكم البسيطة (`0`) من multisig (`1`).
- `norm_version = 1` مشفر كما هو محدد بواسطة Norm v1. القواعد المستقبلية
  إعادة استخدام نفس النطاق من 2 بت.
- `ext_flag` ودائمًا `0`; بتات مهمة تشير إلى الحمولة الممتدة غير
  سوبورتاداس.

قم باختيار الرأس مباشرة:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

يجب أن تكون واجهات المستخدم وأدوات تطوير البرامج (SDKs) واضحة لعرض نوع المحدد:- `0x00` = dominio padrao ضمنيًا (الحمولة الصافية).
- `0x01` = ملخص محلي (12 بايت `blake2s_mac("SORA-LOCAL-K:v1", label)`).
- `0x02` = مدخل السجل العالمي (`registry_id:u32` كبير النهاية).

أمثلة على الأدوات الأساسية السداسية التي يمكن لأدوات البطاقة ربطها أو تشغيلها
 المستندات/الاختبارات:

| نوع الاختيار | هيكس كانونيكو |
|---------------|---------------|
| الحدث الضمني | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| الملخص المحلي (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| التسجيل العالمي (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

Veja `docs/source/references/address_norm_v1.md` للوحة كاملة
حدد/الحالة و`docs/account_structure.md` لرسم تخطيطي كامل للبايت.

## استيراد formas canonicas

المشغلون الذين يقومون بتحويل الرموز البديلة المحلية لـ I105 Canonico أو
السلاسل المضمنة يجب أن تتبع سير العمل CLI الموثق في ADDR-5:

1. `iroha tools address inspect` يصدر ملخص JSON المنشأ مع I105،
   يشتمل على الحمولات النافعة السداسية الكنسي. تتضمن السيرة الذاتية أيضًا موضوعًا
   `domain` مع المجالات `kind`/`warning` وecoa أي سلطة مطلوبة عبر o
   كامبو `input_domain`. عند `kind` و`local12`، تقوم CLI بتقديم المشورة إليك
   stderr واستكمال JSON يوجهان توجيهًا لخطوط الأنابيب CI وSDKs
   possam exibi-la. انتقل إلى `legacy  suffix` واستمر في إعادة إنتاجه
   كوديفيكاكا كونفيرتيدا كومو `<i105>@<domain>`.
2. يمكن لأدوات تطوير البرامج (SDKs) عرض نفس الرسالة/الاستئناف عبر مساعد جافا سكريبت:```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.i105.value, summary.i105Warning);
   ```
  يقوم المساعد بحفظ البادئة I105 المكتشفة بشكل حرفي بما لا يقل عن ذلك
  بشكل صريح `networkPrefix`، بما في ذلك السيرة الذاتية لـ Redes Nao Padrao Nao Sao
  يتم إعادة عرضه بشكل صامت باستخدام البادئة.3. قم بتحويل الحمولة النافعة من Canon وإعادة استخدام المجلدات `i105.value` ou
   `i105` للاستئناف (أو تطلب ترميزًا آخر عبر `--format`). عيسى
   Strings ja sao seguras para compartilhamento externo.
4. تحديث البيانات والسجلات والمستندات الرائعة للعميل بشكل
   canonica and notification as counterparts de que seletores local serao rejeitados
   عندما يتم الانتهاء من ذلك.
5. لمجموعات البيانات الجماعية، قم بالتنفيذ
   `iroha tools address audit --input addresses.txt --network-prefix 753`. يا كوماندوز
   الأدب المنفصل عن لينها الجديدة (التعليقات الأولية com `#` sao
   الجهلة، و`--input -` أو علامة STDIN الأمريكية)، وإصدار علاقة JSON
   com resumos canonicos/I105/comprimidos para cada intrada e conta erros de
   التحليل ونصائح النطاق المحلي. استخدم `--allow-errors` لمقالب المدقق البديلة
   com linhas lixo، e trave a automacao com `strict CI post-check` quando os
   يتم ضبط المشغلين بشكل مباشر لحظر المحددات المحلية في CI.
6. عند الحاجة إلى كتابة هذا الكتاب، استخدمه
  للتخطيط لإصلاح المحددات المحلية، استخدم
  لتصدير ملف CSV `input,status,format,...` الذي يقوم بإزالة ملفات الترميز
  Canonicas و avisos و falhas de parse في مسار واحد.
   يا مساعد الجهل linhas nao Local بواسطة Padrao، قم بتحويل كل مدخل بقية
   للتدوين المطلوب (I105/comprimido/hex/JSON)، والحفاظ على السيادةالأصلي عند `legacy  suffix` وتم تحديده. الجمع بين كوم `--allow-errors`
   لمواصلة التباين أثناء تفريغ المحتوى الأدبي المشوه.
7. يمكن تنفيذ CI/lint تلقائيًا `ci/check_address_normalize.sh`، وهو أمر إضافي
   يتم تحديده محليًا بواسطة `fixtures/account/address_vectors.json`، ويتم التحويل عبره
   `iroha tools address normalize`، ويتم إعادة تنفيذه
   `iroha tools address audit` لإثبات أنه لا يصدر أي عنصر
   ما يهضم المحلية.

`torii_address_local8_total{endpoint}` جونتو كوم
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}`، والرسم Grafana
`dashboards/grafana/address_ingest.json` من أجل التنفيذ النهائي: عندما
لوحات المعلومات الخاصة بالإنتاج لا تحتوي على أي حسد للشرعية المحلية ولا تحتوي على أي فوضى
Local-12 لمدة 30 يومًا متتالية، Torii أو فتح البوابة Local-8 للفشل الشديد
على الشبكة الرئيسية، يتم اتباعه عبر Local-12 عند دخول المجالات العالمية
مراسلو التسجيل ضع في اعتبارك رسالة CLI مثل نصيحة المشغل
تم تجميعها - سلسلة من النصائح واستخدامها في تلميحات أدوات SDK و
Automacao للحفاظ على التوازن مع معايير صيدا لخريطة الطريق. Torii أغورا
مجموعات تطوير/اختبار الانحدارات التشخيصية. استمر في الإسبلهاندو
`torii_address_domain_total{domain_kind}` لا Grafana
(`dashboards/grafana/address_ingest.json`) للحصول على حزمة الأدلة ADDR-7
إثبات أن `domain_kind="local12"` يظل ثابتًا عند الصفر في اللحظة المطلوبة في 30
 لقد تم اختيار البدائل المفضلة لديك في اليوم السابق للشبكة الرئيسية. يا مدير التنبيهات
(`dashboards/alerts/address_ingest_rules.yml`) أديسيونا تريس الدرابزين:- `AddressLocal8Resurgence` الصفحة دائمًا ما يُبلغ السياق عن زيادة
  محلي-8 نوفو. من خلال طرح أسلوب بسيط، قم بترجمة SDK سطحي
  الاستجابة لا توجد في لوحة القيادة، إذا لزم الأمر، يتم تحديدها مؤقتًا
  بادراو (`true`).
- `AddressLocal12Collision` dispara quando dois labels Local-12 fazem hash para
  في نفس الوقت هضم. قم بإيقاف الإعلانات الترويجية مؤقتًا، وقم بتنفيذ مجموعة الأدوات المحلية -> العالمية
  لتدقيق أو خريطة الملخصات والتنسيق مع الإدارة Nexus مسبقًا
  قم بإعادة إدخال التسجيل أو إعادة عمليات النشر في اتجاه مجرى النهر.
- `AddressInvalidRatioSlo` يجب أن يتم ذلك عند عدم صلاحية كل شيء من البداية
  (باستثناء rejeicoes Local-8/strict-mode) تتجاوز نسبة SLO التي تصل إلى 0.1% خلال عدة دقائق.
  استخدم `torii_address_invalid_total` لترجمة السياق/الاستجابة
  والتنسيق مع مجموعة أدوات تطوير البرامج (SDK) المملوكة قبل إعادة تفعيلها.

### سجل مذكرة الإصدار (البطاقة والمستكشف)

قم بتضمين النقطة التالية في ملاحظات تحرير البطاقة/المستكشف عند إرسالها
القطع:

> **التفاصيل:** إضافة أو مساعد `iroha tools address normalize`
> تم الاتصال بـ CI (`ci/check_address_normalize.sh`) لخطوط الأنابيب
> خيارات تحويل بطاقة/مستكشف possam البدائل المحلية للأشكال
> Canonicas I105/comprimidas antes de Local-8/Local-12 محظورة تمامًا
> الشبكة الرئيسية. قم بتنشيط صادرات quaisquer المخصصة لتوجيه أو قيادة e
> قم بإضافة قائمة التطبيع إلى حزمة الأدلة الصادرة.