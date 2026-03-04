---
lang: pt
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

importar ExplorerAddressCard de '@site/src/components/ExplorerAddressCard';

:::nota مستند ماخذ
یہ صفحہ `docs/source/sns/address_display_guidelines.md` کی عکاسی کرتا ہے اور اب
پورٹل کی کینونیکل کاپی ہے۔ سورس فائل ترجمہ PRs کے لئے برقرار رہتی ہے۔
:::

والٹس, ایکسپلوررز اور SDK مثالیں اکاؤنٹ ایڈریسز کو غیر متبدل payload سمجھیں۔
Android کی ریٹیل والٹ مثال
`examples/android/retail-wallet` میں مطلوبہ UX پیٹرن دکھایا گیا ہے:

- **دو کاپی اہداف۔** دو واضح کاپی بٹنز دیں: IH58 (ترجیحی) اور صرف Sora والا
  کمپریسڈ فارم (`sora...`, segundo melhor). IH58 ہمیشہ بیرونی شیئرنگ کے لئے محفوظ ہے اور QR
  carga útil کمپریسڈ فارم میں inline وارننگ لازمی ہے کیونکہ یہ صرف
  Consciente de Sora Android مثال دونوں Material بٹنز اور ٹول ٹپس
  کو `examples/android/retail-wallet/src/main/res/layout/activity_main.xml` میں
  وائر کرتی ہے، اور iOS SwiftUI ڈیمو `examples/ios/NoritoDemo/Sources/ContentView.swift`
  کے اندر `AddressPreviewCard` کے ذریعے یہی UX دہراتا ہے۔
- **Monospace, قابل انتخاب متن۔** دونوں strings کو monospace فونٹ اور
  `textIsSelectable="true"` é um cartão de memória IME que funciona
  دیکھ سکیں۔ قابلِ ترمیم فیلڈز سے بچیں: IME kana کو بدل سکتا ہے یا largura zero
  کوڈ پوائنٹس داخل کر سکتا ہے۔
- **غیر ضمنی ڈیفالٹ ڈومین کے اشارے۔** Seletor de جب ضمنی `default` ڈومین کی طرف
  ہو تو ایک legenda دکھائیں جو آپریٹرز کو یاد دلائے کہ sufixo درکار نہیں۔
  ایکسپلوررز کو بھی canonical ڈومین لیبل ہائی لائٹ کرنا چاہیے جب seletor
  codificação digest کرے۔
- **Cargas úteis QR IH58۔** QR کوڈز کو Codificação de string IH58 کرنا چاہیے۔ QR
  geração ناکام ہو تو خالی تصویر کے بجائے واضح erro دکھائیں۔
- **کلپ بورڈ پیغام۔** کمپریسڈ فارم کاپی کرنے کے بعد torradas یا lanchonete دکھائیں
  جو صارفین کو یاد دلائے کہ یہ صرف Sora ہے اور IME سے خراب ہو سکتا ہے۔

ان گارڈ ریلز پر عمل Corrupção Unicode/IME روکتا ہے اور والٹ/ایکسپلورر UX کے لئے
Aceitação do roteiro ADDR-6 معیار پورا کرتا ہے۔

## اسکرین شاٹ فکسچرز

لوکلائزیشن ریویوز کے دوران درج ذیل فکسچرز استعمال کریں تاکہ بٹن لیبلز, ٹول ٹپس
O que você precisa saber sobre o seu negócio:

- Nome do Android: `/img/sns/address_copy_android.svg`

  ![Android ڈوئل کاپی ریفرنس](/img/sns/address_copy_android.svg)

- iOS Versão: `/img/sns/address_copy_ios.svg`

  ![iOS ڈوئل کاپی ریفرنس](/img/sns/address_copy_ios.svg)

## Ajudantes do SDK

ہر SDK ایک سہولت helper فراہم کرتا ہے جو IH58 اور کمپریسڈ فارم کے ساتھ وارننگ
string دیتا ہے تاکہ UI لیئرز مستقل رہیں:

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- Inspetor JavaScript: `inspectAccountId(...)` کمپریسڈ وارننگ string لوٹاتا ہے
  اور اسے `warnings` میں شامل کرتا ہے جب کالرز `sora...` literal دیں, تاکہ والٹ/
  ایکسپلورر ڈیش بورڈز colar/validação فلو میں Somente Sora وارننگ دکھا سکیں, نہ کہ
  صرف تب جب وہ کمپریسڈ فارم خود بنائیں۔
-Python: `AccountAddress.display_formats(network_prefix: int = 753)`
- Rápido: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
-Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

ان ajudantes کو استعمال کریں, UI لیئر میں codificar لاجک دوبارہ مت لکھیں۔ JavaScript
auxiliar `domainSummary` com carga útil `selector` (`tag`, `digest_hex`, `registry_id`,
`label`) بھی دیتا ہے تاکہ UIs یہ ظاہر کر سکیں کہ seletor Local-12 ہے یا رجسٹری
سے بیکڈ ہے، بغیر carga útil bruta دوبارہ análise کیے۔

## ایکسپلورر instrumentação ڈیمو

ایکسپلوررز کو والٹ کی telemetria e acessibilidade کے کام کو espelho کرنا چاہیے:

- کاپی بٹنز پر `data-copy-mode="ih58|compressed (`sora`)|qr"` لگائیں تاکہ فرنٹ اینڈز Torii
  میٹرک `torii_address_format_total` کے ساتھ contadores de uso نکال سکیں۔ اوپر والا
  ڈیمو کمپوننٹ `{mode,timestamp}` کے ساتھ `iroha:address-copy` ایونٹ بھیجتا ہے؛
  Um pipeline de análise/telemetria (segmento de segmento ou coletor apoiado por NORITO)
  سے جوڑیں تاکہ dashboards سرور سائیڈ formato de endereço استعمال اور کلائنٹ کاپی
  موڈز کو correlacionar کر سکیں۔ Contadores de domínio Torii
  (`torii_address_domain_total{domain_kind}`) کو اسی فیڈ میں بھیجیں تاکہ Local-12
  ریٹائرمنٹ ریویوز `address_ingest` Grafana بورڈ سے براہ راست 30 dias por semana
  `domain_kind="local12"` برآمد کر سکیں۔
- ہر کنٹرول کے لئے الگ `aria-label`/`aria-describedby` ہنٹس دیں جو بتائیں کہ
  literal شیئر کرنے کے لئے محفوظ ہے (IH58) یا صرف Sora (کمپریسڈ)۔ ضمنی ڈومین
  legenda کو descrição میں شامل کریں تاکہ tecnologia assistiva وہی سیاق دکھائے
  Isso é tudo o que você precisa saber
- ایک região ativa (مثلاً `<output aria-live="polite">...</output>`) رکھیں جو کاپی
  Como usar o Swift/Android Como usar o VoiceOver/TalkBack
  رویے کے مطابق۔

یہ instrumentação ADDR-6b پوری کرتی ہے کیونکہ یہ دکھاتی ہے کہ آپریٹرز Local
seletores کے غیر فعال ہونے سے پہلے Ingestão Torii e modos de cópia do lado do cliente
دونوں کا مشاہدہ کر سکتے ہیں۔

## Local -> Kit de ferramentas de migração global

Seletores locais کی auditoria e conversão خودکار ہو۔ auditoria JSON auxiliar
IH58/کمپریسڈ لسٹ دونوں بناتا ہے جنہیں آپریٹرز bilhetes de preparação کے ساتھ منسلک
کرتے ہیں, جبکہ متعلقہ runbook Grafana dashboards e Alertmanager قواعد کو لنک
کرتا ہے جو transição de modo estrito کو portão کرتے ہیں۔

## بائنری layout کا فوری حوالہ (ADDR-1a)

جب SDKs ferramentas de endereço avançadas (inspetores, dicas de validação, construtores de manifesto)
دکھائیں تو desenvolvedores کو `docs/account_structure.md` میں موجود fio canônico
فارمیٹ کی طرف بھیجیں۔ layout ہمیشہ `header · selector · controller` ہوتا ہے, جہاں
bits de cabeçalho یہ ہیں:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (bits 7-5) موجودہ ہے؛ diferente de zero اقدار ریزرو ہیں اور
  `AccountAddressError::InvalidHeaderVersion` Cartão de crédito
- Controladores `addr_class` único (`0`) e multisig (`1`) میں فرق بتاتا ہے۔
- `norm_version = 1` Seletor de norma v1 قواعد codificação کرتا ہے؛ مستقبل کے normas اسی
  فیلڈ کو دوبارہ استعمال کریں گے۔
- `ext_flag` ہمیشہ `0` ہے؛ فعال bits غیر معاون extensões de carga útil دکھاتے ہیں۔

seletor de cabeçalho فوراً کے بعد آتا ہے:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

UIs e SDKs e seletores são os seguintes:

- `0x00` = ضمنی ڈیفالٹ ڈومین (کوئی carga útil نہیں)۔
- `0x01` = resumo do resumo (`blake2s_mac("SORA-LOCAL-K:v1", label)` de 12 bytes).
- `0x02` = گلوبل رجسٹری انٹری (`registry_id:u32` big-endian).

کینونیکل hex مثالیں جنہیں والٹ ٹولنگ docs/tests میں لنک یا embed کر سکتی ہے:

| Seletor قسم | Hexágono canônico |
|---------------|---------------|
| ضمنی ڈیفالٹ | `0x02000001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| Digerir o resumo (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| گلوبل رجسٹری (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |Selecione o seletor/estado ٹیبل کے لئے `docs/source/references/address_norm_v1.md`
Diagrama de bytes کے لئے `docs/account_structure.md` دیکھیں۔

## Formas canônicas نافذ کرنا

O fluxo de trabalho CLI do ADDR-5 é o seguinte:

1. `iroha tools address inspect` para IH58, کمپریسڈ, e cargas úteis hexadecimais canônicas کے ساتھ
   resumo JSON estruturado resumo `kind`/`warning` e `domain`
   آبجیکٹ بھی ہوتے ہیں اور `input_domain` کے ذریعے دیے گئے ڈومین کو بھی echo
   کرتا ہے۔ Use `kind` `local12` e CLI stderr para usar como JSON
   resumo وہی رہنمائی دہراتا ہے تاکہ Pipelines CI e SDKs اسے superfície کر سکیں۔
   جب بھی آپ convert شدہ codificação کو `<ih58>@<domain>` کی صورت میں replay کرنا
   چاہیں ou `--append-domain` دیں۔
2. SDKs اسی وارننگ/summary e JavaScript helper کے ذریعے دکھا سکتے ہیں:

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed (`sora`));
   ```
  literal auxiliar سے detectar کیا گیا Prefixo IH58 محفوظ رکھتا ہے جب تک آپ
  `networkPrefix` é um produto de alta qualidade اس لئے redes não padrão کے
  resumos خاموشی سے prefixo padrão کے ساتھ دوبارہ render نہیں ہوتے۔

3. carga útil canônica کو `ih58.value` یا `compressed (`sora`)` فیلڈز سے reutilização کر کے تبدیل
   کریں (یا `--format` کے ذریعے دوسری codificação مانگیں)۔ یہ strings پہلے سے
   بیرونی شیئرنگ کے لئے محفوظ ہیں۔
4. manifestos, registros e documentos voltados para o cliente e canônicos فارم سے اپ ڈیٹ
   کریں اور فریقین کو مطلع کریں کہ cutover مکمل ہونے پر Seletores locais ریجیکٹ
   ہوں گے۔
5. بلک ڈیٹا سیٹس کے لئے
   `iroha tools address audit --input addresses.txt --network-prefix 753` چلائیں۔ کمانڈ
   literais separados por nova linha پڑھتی ہے ( `#` سے شروع ہونے والے comentários نظرانداز
   ہوتے ہیں, اور `--input -` یا کوئی فلیگ نہ ہو تو STDIN استعمال ہوتا ہے), ہر
   اندراج کے لئے resumos canônicos/IH58 (ترجیحی)/comprimidos (`sora`) (`sora`, segundo melhor) کے ساتھ JSON رپورٹ بناتی
   linhas ہوں تو `--allow-errors` استعمال کریں, اور جب آپریٹرز Seletores locais کو
   CI میں بلاک کرنے کے لئے تیار ہوں تو `--fail-on-warning` سے آٹومیشن گیٹ کریں۔
6. Como reescrever nova linha para nova linha
  Planilhas de remediação do seletor local
  استعمال کریں تاکہ `input,status,format,...` CSV برآمد ہو جو codificações canônicas,
  avisos e falhas de análise ajudante ڈیفالٹ طور پر
  linhas não locais چھوڑ دیتا ہے، باقی entradas کو مطلوبہ codificação (IH58 ترجیحی/comprimido (`sora`) segundo melhor/hex/JSON)
  میں بدلتا ہے، اور `--append-domain` پر اصل ڈومین محفوظ رکھتا ہے۔ `--allow-errors`
  کے ساتھ جوڑیں تاکہ خراب literais e dumps پر بھی scan جاری رہے۔
7. Automação CI/lint `ci/check_address_normalize.sh` چلا سکتی ہے، جو
   `fixtures/account/address_vectors.json` سے Seletores locais نکال کر
   `iroha tools address normalize` سے تبدیل کرتی ہے، اور
   `iroha tools address audit --fail-on-warning` دوبارہ چلاتی ہے تاکہ ثابت ہو کہ
   lançamentos اب Resumos locais نہیں نکالتے۔`torii_address_local8_total{endpoint}` کے ساتھ
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
Placa `torii_address_collision_domain_total{endpoint,domain}` e Grafana
Sinal de aplicação `dashboards/grafana/address_ingest.json` دیتے ہیں: جب
painéis de produção 30 dias após envios locais legítimos
Colisões locais 12 دکھائیں تو Torii Porta local 8 کو mainnet پر hard-fail کرے
گا، پھر Local-12 کو اس وقت جب domínios globais میں entradas de registro correspondentes ہوں۔
اس freeze کے لئے CLI output کو voltado para o operador نوٹس سمجھیں - وہی warning string
Dicas de ferramentas do SDK e automação میں استعمال ہوتی ہے تاکہ critérios de saída do roteiro سے
ہے؛ diagnóstico de regressões
`torii_address_domain_total{domain_kind}` e Grafana (`dashboards/grafana/address_ingest.json`)
میں espelho کرتے رہیں تاکہ pacote de evidências ADDR-7 یہ ثابت کر سکے کہ
seletores کو desativar کرے۔ Pacote Alertmanager
(`dashboards/alerts/address_ingest_rules.yml`) Os guarda-corpos são os seguintes:

- `AddressLocal8Resurgence` اس وقت página کرتا ہے جب کوئی contexto نیا Local-8
  incrementar implementações em modo estrito روکیں, painel میں SDK ofensivo
  کریں جب تک sinal صفر نہ ہو جائے، پھر padrão (`true`) بحال کریں۔
- `AddressLocal12Collision` تب فائر ہوتا ہے جب دو rótulos locais-12 ایک ہی resumo
  پر hash ہوں۔ promoções de manifesto روکیں، Local -> Kit de ferramentas global چلا کر resumo
  mapeamento آڈٹ کریں، اور Nexus governança کے ساتھ coordenada کریں اس سے پہلے کہ
  entrada de registro دوبارہ جاری ہو یا implementações downstream بحال ہوں۔
- `AddressInvalidRatioSlo` خبردار کرتا ہے جب proporção inválida para toda a frota (Local-8/
  rejeições de modo estrito کے بغیر) 10 منٹ تک 0,1% SLO سے بڑھ جائے۔
  `torii_address_invalid_total` استعمال کر کے متعلقہ contexto/motivo کی نشاندہی
  کریں اور possuir SDK ٹیم کے ساتھ coordenada کر کے modo estrito دوبارہ فعال کریں۔

### ریلیز نوٹ اسنیپٹ (والٹ اور ایکسپلورر)

cutover کے وقت والٹ/ایکسپلورر ریلیز نوٹس میں درج ذیل bullet شامل کریں:

> **Endereços:** `iroha tools address normalize --only-local --append-domain` helper شامل
> کیا گیا اور اسے CI (`ci/check_address_normalize.sh`) میں وائر کیا گیا تاکہ
> میں تبدیل کر سکیں, قبل اس کے کہ Mainnet Local-8/Local-12 پر بلاک ہوں۔ کسی بھی
> exportações personalizadas کو اپ ڈیٹ کریں تاکہ کمانڈ چلائی جائے اور lista normalizada کو
> liberar pacote de evidências کے ساتھ منسلک کریں۔