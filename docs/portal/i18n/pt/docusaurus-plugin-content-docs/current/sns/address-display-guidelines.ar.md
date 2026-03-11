---
lang: pt
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

importar ExplorerAddressCard de '@site/src/components/ExplorerAddressCard';

:::note المصدر القياسي
Resolva o problema `docs/source/sns/address_display_guidelines.md` e instale-o
كمرجع بوابة موحد. Não há problema em PRs.
:::

Não instale o SDK e o SDK do seu computador para que você possa usá-lo
Bem. Como usar o Android no Android
`examples/android/retail-wallet` Nome do UX:

- **هدفا نسخ منفصلان.** وفر زرين واضحين للنسخ: I105 (المفضل) والصيغة
  O nome de usuário é Sora (`sora...`, número de telefone). I105 امن دائما للمشاركة خارجيا ويغذي
  حمولة QR. Não se preocupe, você pode fazer isso sem parar.
  تطبيقات واعية بـ Sora. Como Android não tem material e material para Android
  `examples/android/retail-wallet/src/main/res/layout/activity_main.xml`, ويطابق
  Baixar iOS SwiftUI para UX usando `AddressPreviewCard`
  `examples/ios/NoritoDemo/Sources/ContentView.swift`.
- **خط ثابت ونص قابل للتحديد.** اعرض السلسلتين بخط monospace مع
  `textIsSelectable="true"` é um arquivo de rede do IME.
  تجنب الحقول القابلة للتحرير: يمكن لـ IME اعادة كتابة kana او حقن نقاط كود بعرض
  Sim.
- **اشارات النطاق الافتراضي الضمني.** عندما يشير المحدد الى النطاق الضمني
  `default`; يجب على
  المستكشفات ايضا تمييز تسمية النطاق القانونية عندما يشفر المحدد digest.
- **حمولات QR I105.** يجب ان ترمز رموز QR سلسلة I105. اذا فشل توليد QR, اعرض
  Isso é o que acontece com o dinheiro.
- **رسائل الحافظة.** بعد نسخ الصيغة المضغوطة, ارسل torradas e snackbar يذكر
  Você pode usar Sora e acessar o IME.

Você pode usar o Unicode/IME para usar o ADDR-6
لتجربة محافظ/مستكشفات.

## لقطات مرجعية

استخدم اللقطات التالية خلال مراجعات الترجمة لضمان بقاء تسميات الازرار
والتلميحات والتحذيرات متوافقة عبر المنصات:

- Android Android: `/img/sns/address_copy_android.svg`

  ![مرجع نسخ مزدوج Android](/img/sns/address_copy_android.svg)

- Dispositivo iOS: `/img/sns/address_copy_ios.svg`

  ![مرجع نسخ مزدوج iOS](/img/sns/address_copy_ios.svg)

## SDK do SDK

O SDK não é compatível com I105 e com a interface do usuário
Exemplo:

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- Inspetor JavaScript: `inspectAccountId(...)`
  O `warnings` é o mesmo que o literal `sora...`, o que significa que
  المحافظ/لوحات التحكم من عرض تحذير Somente Sora اثناء تدفقات اللصق/التحقق بدلا
  Isso é tudo que você precisa saber.
-Python: `AccountAddress.display_formats(network_prefix: int = 753)`
- Rápido: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
-Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

Você pode usar o recurso de interface do usuário na interface do usuário. يعرض مساعد
JavaScript é definido como `selector` ou `domainSummary` (`tag`, `digest_hex`,
`registry_id`, `label`) حتى تتمكن واجهات UI من تحديد ما اذا كان المحدد Local-12
E isso pode ser um problema para você.

## عرض حي لادوات المستكشف



يجب ان تعكس المستكشفات اعمال القياس والاتاحة نفسها في المحافظ:- طبق `data-copy-mode="i105|i105_default|qr"` على ازرار النسخ حتى تتمكن الواجهات
  O código de barras do produto Torii
  `torii_address_format_total`. المكون التجريبي اعلاه يطلق حدث
  `iroha:address-copy` com `{mode,timestamp}` - اربط ذلك بخط تحليلاتك/تليمترتك
  (مثل ارسالها الى Segment او جامع NORITO) حتى تتمكن لوحات المتابعة من ربط
  Verifique se o seu dispositivo está funcionando corretamente. اعكس ايضا عدادات نطاق
  Torii (`torii_address_domain_total{domain_kind}`) é um problema de segurança
  مراجعات تقاعد Local-12 من تصدير دليل 30 يوم `domain_kind="local12"` مباشرة من
  O `address_ingest` é o Grafana.
- اربط كل عنصر تحكم بتلميحات `aria-label`/`aria-describedby` مميزة تشرح ما اذا
  كانت السلسلة امنة للمشاركة (I105) e خاصة بـ Sora (مضغوطة). ادرج تسمية
  No entanto, você pode usar o recurso de segurança para obter mais informações.
- O problema é o mesmo (como `<output aria-live="polite">...</output>`) تعلن
  نتائج النسخ والتحذيرات, بما يطابق سلوك VoiceOver/TalkBack Como usar o VoiceOver/TalkBack no Facebook
  Sobre Swift/Android.

O ADDR-6b é um dispositivo de armazenamento de dados que pode ser instalado no Torii
واوضاع نسخ العميل قبل تعطيل محددات Local.

## عدة ترحيل Local -> Global

استخدم [عدة Local -> Global](local-to-global-toolkit.md).
محددات Local القديمة. Obtenha o JSON e o arquivo JSON
I105/المضغوطة التي يرفقها المشغلون بتذاكر الجاهزية, بينما يربط دليل التشغيل
O Grafana e o Alertmanager não alteram o cutover no site.

## مرجع سريع للتخطيط الثنائي (ADDR-1a)

عندما تعرض SDKs ادوات متقدمة للعناوين (المفتشون, تلميحات التحقق, بناة manifesto),
Verifique o fio do fio em `docs/account_structure.md`. التخطيط
O cabeçalho `header · selector · controller` é o seguinte:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (bits 7-5) Código القيم غير الصفرية محجوزة ويجب ان تؤدي
  Sobre `AccountAddressError::InvalidHeaderVersion`.
- `addr_class` يميز بين المتحكم الفردي (`0`) والمتعدد التواقيع (`1`).
- `norm_version = 1` يشفر قواعد محدد Norm v1. ستعيد المعايير المستقبلية استخدام
  نفس الحقل المكون em 2 dias.
- `ext_flag` ou `0`; Você pode fazer isso sem problemas.

يتبع المحدد مباشرة الـ cabeçalho:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

A interface do usuário e os SDKs estão disponíveis para você:

- `0x00` = `0x00` = نطاق افتراضي ضمني (بدون حمولة).
- `0x01` = resumo do arquivo (`blake2s_mac("SORA-LOCAL-K:v1", label)` de 12 bytes).
- `0x02` = valor de referência (`registry_id:u32` big-endian).

امثلة hex قانونية يمكن لادوات المحافظ ربطها او ادراجها في docs/tests:

| نوع المحدد | Hex قانوني |
|---------------|---------------|
| افتراضي ضمني | `0x020001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| digerir livro (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| سجل عالمي (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

راجع `docs/source/references/address_norm_v1.md` para obter informações/referências e
`docs/account_structure.md` é uma ferramenta de controle de qualidade.

## فرض الصيغ القانونية

يجب على المشغلين الذين يحولون ترميزات Local القديمة الى I105 قانوني او سلاسل
مضغوطة اتباع مسار CLI الموثق تحت ADDR-5:1. `iroha tools address inspect` يصدر الان ملخص JSON منظم مع I105 والحمولة المضغوطة
   e hexadecimal. A solução `domain` é igual a `kind`/`warning`
   O problema é o `input_domain`. Eu tenho `kind` e `local12`
   A CLI é usada para stderr e JSON para ser usada no CI e
   SDKs estão disponíveis. مرر `legacy  suffix` متى اردت اعادة تشغيل الترميز المحول
   كـ `<i105>@<domain>`.
2. Use SDKs para usar JavaScript:

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.i105.value, summary.i105Warning);
   ```
  O código I105 é literalmente `networkPrefix`
  Não se preocupe, você pode usar o telefone para obter mais informações.

3. Verifique o tamanho do arquivo `i105.value` e `i105_default`
   Isso é feito (ou seja, o arquivo `--format`). هذه السلاسل امنة بالفعل
   Não.
4. حدث manifestos والسجلات والوثائق المواجهة للعميل بالصيغ القانونية وابلغ
   O local não pode ser cortado por meio de cortes locais.
5. لمجموعات البيانات الكبيرة, شغل
   `iroha tools address audit --input addresses.txt --network-prefix 753`. يقرأ الامر
   literais
   `--input -` e um arquivo JSON com STDIN) e um arquivo JSON
   Local. استخدم
   `--allow-errors` é um dump de lixeira que pode ser usado para armazenar dados
   O código `strict CI post-check` não pode ser encontrado no local no CI.
6. Faça o download do seu cartão de crédito
  لملفات الجداول الخاصة بمعالجة محددات Local, استخدم
  A remoção de CSV `input,status,format,...` é feita por meio de arquivos e arquivos
  واخفاقات التحليل في مرور واحد. يتخطى المساعد الصفوف غير المحلية افتراضيا,
  ويحول كل ادخال متبق الى الترميز المطلوب (I105/مضغوط/hex/JSON), ويحافظ على
  O código de segurança é `legacy  suffix`. Modelo `--allow-errors`
  Não há necessidade de usar literais.
7. Use CI/lint como `ci/check_address_normalize.sh` para obter mais informações
   Local `fixtures/account/address_vectors.json`, ويحولها عبر
   `iroha tools address normalize`;
   `iroha tools address audit` لاثبات ان الاصدارات para تعد تصدر digests
   Locais.

`torii_address_local8_total{endpoint}` بالاضافة الى
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
`torii_address_collision_domain_total{endpoint,domain}`, e Grafana
`dashboards/grafana/address_ingest.json` `dashboards/grafana/address_ingest.json` `dashboards/grafana/address_ingest.json` `dashboards/grafana/address_ingest.json`:
الانتاج صفرا من عمليات ارسال Local الشرعية وصفرا من تصادمات Local-12 لمدة 30 يوما
Use Torii para Local-8 no site da mainnet, Local-12 para
Não se preocupe, você pode fazer isso sem problemas. Obtenha o CLI do aplicativo
Para obter dicas de ferramentas no SDK e no SDK
للحفاظ على التوافق مع معايير الخروج في خارطة الطريق. Torii Torii Torii
Regressões تشخيص. Use o modelo `torii_address_domain_total{domain_kind}`
Grafana (`dashboards/grafana/address_ingest.json`) é um dispositivo de segurança para ADDR-7.
Use `domain_kind="local12"` para obter mais informações em 30 dias.
تعطل mainnet do site. Alertmanager
(`dashboards/alerts/address_ingest_rules.yml`) Como fazer isso:- `AddressLocal8Resurgence` يستدعي عندما يبلغ سياق عن زيادة Local-8 جديدة. اوقف
  Implementar rollout no site do SDK, no site do SDK, e
  Código (`true`).
- `AddressLocal12Collision` يعمل عندما يقوم اسمان Local-12 بعمل hash الى نفس
  digerir. Para manifestar os manifestos, vá para Local -> Global para obter resumos, e
  O Nexus pode ser usado para implementar implementações e implementações
  a jusante.
- `AddressInvalidRatioSlo` يحذر عندما يتجاوز معدل عدم الصلاحية على مستوى
  A taxa (definição Local-8/strict-mode) é de 0,1% para o valor total. استخدم
  `torii_address_invalid_total` é um software de gerenciamento/reconhecimento de conteúdo e um SDK do SDK
  Você pode fazer isso com cuidado.

### مقتطف مذكرة الاصدار (محفظة ومُستكشف)

ادرج النقطة التالية في ملاحظات اصدار المحفظة/المستكشف عند تنفيذ cutover:

> **العناوين:** تمت اضافة مساعد `iroha tools address normalize`
> وربطه في CI (`ci/check_address_normalize.sh`) حتى تتمكن مسارات المحفظة/المستكشف
> من تحويل محددات Local القديمة الى صيغ I105/مضغوطة قانونية قبل حظر Local-8/Local-12
> Em mainnet. Você pode fazer isso com uma chave de fenda e uma caixa de som
> بحزمة دليل الاصدار.