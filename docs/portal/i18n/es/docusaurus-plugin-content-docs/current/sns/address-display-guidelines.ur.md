---
lang: es
direction: ltr
source: docs/portal/docs/sns/address-display-guidelines.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

importar ExplorerAddressCard desde '@site/src/components/ExplorerAddressCard';

:::nota مستند ماخذ
یہ صفحہ `docs/source/sns/address_display_guidelines.md` کی عکاسی کرتا ہے اور اب
پورٹل کی کینونیکل کاپی ہے۔ سورس فائل ترجمہ PRs کے لئے برقرار رہتی ہے۔
:::

Uso de SDK y carga útil de carga útil
Android کی ریٹیل والٹ مثال
`examples/android/retail-wallet` میں مطلوبہ UX پیٹرن دکھایا گیا ہے:- **دو کاپی اہداف۔** دو واضح کاپی بٹنز دیں: IH58 (ترجیحی) اور صرف Sora والا
  کمپریسڈ فارم (`sora...`، segundo mejor). IH58 ہمیشہ بیرونی شیئرنگ کے لئے محفوظ ہے اور QR
  carga útil بناتا ہے۔ کمپریسڈ فارم میں en línea وارننگ لازمی ہے کیونکہ یہ صرف
  Consciente de Sora ایپس میں کام کرتا ہے۔ Android مثال دونوں Material بٹنز اور ٹول ٹپس
  Número `examples/android/retail-wallet/src/main/res/layout/activity_main.xml` Número
  Aplicación iOS SwiftUI `examples/ios/NoritoDemo/Sources/ContentView.swift`
  کے اندر `AddressPreviewCard` کے ذریعے یہی UX دہراتا ہے۔
- **Monoespacio, قابل انتخاب متن۔** دونوں cadenas کو monoespacio فونٹ اور
  `textIsSelectable="true"` کے ساتھ رینڈر کریں تاکہ صارفین IME کے بغیر اقدار
  دیکھ سکیں۔ قابلِ ترمیم فیلڈز سے بچیں: IME kana کو بدل سکتا ہے یا de ancho cero
  کوڈ پوائنٹس داخل کر سکتا ہے۔
- **غیر ضمنی ڈیفالٹ ڈومین کے اشارے۔** جب selector ضمنی `default` ڈومین کی طرف
  ہو تو ایک título دکھائیں جو آپریٹرز کو یاد دلائے کہ sufijo درکار نہیں۔
  ایکسپلوررز کو بھی canonical ڈومین لیبل ہائی لائٹ کرنا چاہیے جب selector
  codificar resumen کرے۔
- **Cargas útiles QR IH58 ۔** Código QR کوڈز کو Codificación de cadena IH58 کرنا چاہیے۔ QR
  generación ناکام ہو تو خالی تصویر کے بجائے واضح error دکھائیں۔
- **کلپ بورڈ پیغام۔** کمپریسڈ فارم کاپی کرنے کے بعد tostadas یا snackbar دکھائیں
  جو صارفین کو یاد دلائے کہ یہ صرف Sora ہے اور IME سے خراب ہو سکتا ہے۔

ان گارڈ ریلز پر عمل Corrupción Unicode/IME روکتا ہے اور والٹ/ایکسپلورر UX کے لئے
Aceptación de la hoja de ruta ADDR-6 معیار پورا کرتا ہے۔

## اسکرین شاٹ فکسچرزلوکلائزیشن ریویوز کے دوران درج ذیل فکسچرز استعمال کریں تاکہ بٹن لیبلز، ٹول ٹپس
اور وارننگز پلیٹ فارمز کے درمیان ہم آہنگ رہیں:

- Versión de Android: `/img/sns/address_copy_android.svg`

  ![Android ڈوئل کاپی ریفرنس](/img/sns/address_copy_android.svg)

- iOS color: `/img/sns/address_copy_ios.svg`

  ![iOS ڈوئل کاپی ریفرنس](/img/sns/address_copy_ios.svg)

## Ayudantes del SDK

ہر SDK ایک سہولت helper فراہم کرتا ہے جو IH58 اور کمپریسڈ فارم کے ساتھ وارننگ
string دیتا ہے تاکہ UI لیئرز مستقل رہیں:

- JavaScript: `AccountAddress.displayFormats(networkPrefix?: number)`
  (`javascript/iroha_js/src/address.js`)
- Inspector de JavaScript: `inspectAccountId(...)` کمپریسڈ وارننگ string لوٹاتا ہے
  اور اسے `warnings` میں شامل کرتا ہے جب کالرز `sora...` literal دیں، تاکہ والٹ/
  ایکسپلورر ڈیش بورڈز pegar/validación فلو میں Sora-only وارننگ دکھا سکیں، نہ کہ
  صرف تب جب وہ کمپریسڈ فارم خود بنائیں۔
- Pitón: `AccountAddress.display_formats(network_prefix: int = 753)`
- Rápido: `AccountAddress.displayFormats(networkPrefix: UInt16 = 753)`
-Java/Kotlin: `AccountAddress.displayFormats(int networkPrefix = 753)`
  (`java/iroha_android/src/main/java/org/hyperledger/iroha/android/address/AccountAddress.java`)

ان helpers کو استعمال کریں، UI لیئر میں codificar لاجک دوبارہ مت لکھیں۔ javascript
ayudante `domainSummary` میں `selector` carga útil (`tag`, `digest_hex`, `registry_id`,
`label`) بھی دیتا ہے تاکہ UIs یہ ظاہر کر سکیں کہ selector Local-12 ہے یا رجسٹری
سے بیکڈ ہے، بغیر carga útil sin procesar دوبارہ analizar کیے۔

## ایکسپلورر instrumentación ڈیمو



ایکسپلوررز کو والٹ کی telemetría اور accesibilidad کے کام کو espejo کرنا چاہیے:- کاپی بٹنز پر `data-copy-mode="ih58|compressed (`sora`)|qr"` لگائیں تاکہ فرنٹ اینڈز Torii
  میٹرک `torii_address_format_total` کے ساتھ contadores de uso نکال سکیں۔ اوپر والا
  ڈیمو کمپوننٹ `{mode,timestamp}` کے ساتھ `iroha:address-copy` ایونٹ بھیجتا ہے؛
  Canal de análisis/telemetría de اسے اپنے (recopilador respaldado por NORITO del segmento یا)
  سے جوڑیں تاکہ paneles سرور سائیڈ formato de dirección استعمال اور کلائنٹ کاپی
  موڈز کو correlacionar کر سکیں۔ Contadores de dominio Torii
  (`torii_address_domain_total{domain_kind}`) کو اسی فیڈ میں بھیجیں تاکہ Local-12
  ریٹائرمنٹ ریویوز `address_ingest` Grafana بورڈ سے براہ راست 30 دن کا ثبوت
  `domain_kind="local12"` برآمد کر سکیں۔
- ہر کنٹرول کے لئے الگ `aria-label`/`aria-describedby` ہنٹس دیں جو بتائیں کہ
  literal شیئر کرنے کے لئے محفوظ ہے (IH58) یا صرف Sora (کمپریسڈ)۔ ضمنی ڈومین
  título کو descripción میں شامل کریں تاکہ tecnología de asistencia وہی سیاق دکھائے
  جو بصری طور پر نظر آتا ہے۔
- ایک región en vivo (مثلاً `<output aria-live="polite">...</output>`) رکھیں جو کاپی
  Programación de aplicaciones Swift/Android con VoiceOver/TalkBack
  رویے کے مطابق۔

یہ instrumentación ADDR-6b پوری کرتی ہے کیونکہ یہ دکھاتی ہے کہ آپریٹرز Local
selectores کے غیر فعال ہونے سے پہلے Torii modos de ingesta y copia del lado del cliente
دونوں کا مشاہدہ کر سکتے ہیں۔

## Local -> Kit de herramientas de migración globalSelectores locales کی auditoría اور conversión خودکار ہو۔ auditoría JSON auxiliar رپورٹ اور
IH58/کمپریسڈ لسٹ دونوں بناتا ہے جنہیں آپریٹرز tickets de preparación کے ساتھ منسلک
کرتے ہیں، جبکہ متعلقہ runbook Grafana paneles de control y Alertmanager قواعد کو لنک
کرتا ہے جو corte de modo estricto کو puerta کرتے ہیں۔

## Diseño بائنری کا فوری حوالہ (ADDR-1a)

Herramientas avanzadas de direcciones de SDK (inspectores, sugerencias de validación, creadores de manifiestos)
دکھائیں تو desarrolladores کو `docs/account_structure.md` میں موجود canonical wire
فارمیٹ کی طرف بھیجیں۔ diseño ہمیشہ `header · selector · controller` ہوتا ہے، جہاں
bits de encabezado یہ ہیں:

```
bit index:   7        5 4      3 2      1 0
             ┌─────────┬────────┬────────┬────┐
payload bit: │version  │ class  │  norm  │ext │
             └─────────┴────────┴────────┴────┘
```

- `addr_version = 0` (bits 7-5) موجودہ ہے؛ distinto de cero اقدار ریزرو ہیں اور
  `AccountAddressError::InvalidHeaderVersion` اٹھانی چاہئیں۔
- Controladores `addr_class` single (`0`) y multisig (`1`) میں فرق بتاتا ہے۔
- `norm_version = 1` Selector de norma v1 قواعد codificar کرتا ہے؛ مستقبل کے normas اسی
  Tarjeta gráfica de 2 bits
- `ext_flag` ہمیشہ `0` ہے؛ فعال bits غیر معاون extensiones de carga útil دکھاتے ہیں۔

selector فوراً encabezado کے بعد آتا ہے:

```
┌──────────┬──────────────────────────────────────────────┐
│ tag (u8) │ payload (depends on selector kind)           │
└──────────┴──────────────────────────────────────────────┘
```

UIs y SDKs کو selector کی قسم دکھانے کے لئے تیار ہونا چاہیے:

- `0x00` = ضمنی ڈیفالٹ ڈومین (کوئی carga útil نہیں)۔
- `0x01` = resumen de datos (`blake2s_mac("SORA-LOCAL-K:v1", label)` de 12 bytes).
- `0x02` = گلوبل رجسٹری انٹری (`registry_id:u32` big-endian).

کینونیکل hexadecimal مثالیں جنہیں والٹ ٹولنگ docs/tests میں لنک یا embed کر سکتی ہے:| Selector قسم | Hexágono canónico |
|---------------|---------------|
| ضمنی ڈیفالٹ | `0x02000001203b6a27bcceb6a42d62a3a8d02a6f0d73653215771de243a63ac048a18b59da29` |
| Resumen de لوکل (`treasury`) | `0x0201b18fe9c1abbac45b3e38fc5d0001203b77a042f1de02f6d5f418f36a2a28ea` |
| گلوبل رجسٹری (`android`) | `0x020200000059a6a47eb7c9aa415f77b18636a85a57837d5518ff5357ef63c35202` |

Selector/estado del equipo ٹیبل کے لئے `docs/source/references/address_norm_v1.md` اور
Diagrama de bytes completo کے لئے `docs/account_structure.md` دیکھیں۔

## Formas canónicas نافذ کرنا

Utilice el flujo de trabajo CLI ADDR-5 para su configuración:

1. `iroha tools address inspect` اب IH58, کمپریسڈ، اور cargas útiles hexadecimales canónicas کے ساتھ
   resumen JSON estructurado دیتا ہے۔ resumen Pantalla `kind`/`warning` y `domain`
   آبجیکٹ بھی ہوتے ہیں اور `input_domain` کے ذریعے دیے گئے ڈومین کو بھی echo
   کرتا ہے۔ Incluye `kind` `local12` y CLI stderr en formato JSON.
   resumen وہی رہنمائی دہراتا ہے تاکہ CI pipelines اور SDK اسے superficie کر سکیں۔
   جب بھی آپ convertir شدہ codificación کو `<ih58>@<domain>` کی صورت میں reproducir کرنا
   چاہیں تو `--append-domain` دیں۔
2. SDK اسی وارننگ/resumen کو Ayudante de JavaScript کے ذریعے دکھا سکتے ہیں:

   ```js
   import { inspectAccountId } from "@iroha/iroha-js";

   const summary = inspectAccountId("sora...");
   if (summary.domain.warning) {
     console.warn(summary.domain.warning);
   }
   console.log(summary.ih58.value, summary.compressed (`sora`));
   ```
  literal auxiliar سے detectar کیا گیا prefijo IH58 محفوظ رکھتا ہے جب تک آپ
  `networkPrefix` واضح طور پر فراہم نہ کریں؛ اس لئے redes no predeterminadas کے
  resúmenes خاموشی سے prefijo predeterminado کے ساتھ دوبارہ render نہیں ہوتے۔3. carga útil canónica کو `ih58.value` یا `compressed (`sora`)` فیلڈز سے reutilización کر کے تبدیل
   کریں (یا `--format` کے ذریعے دوسری codificación مانگیں)۔ یہ cuerdas پہلے سے
   بیرونی شیئرنگ کے لئے محفوظ ہیں۔
4. manifiestos, registros y documentos de cara al cliente y canónicos فارم سے اپ ڈیٹ
   کریں اور فریقین کو مطلع کریں کہ corte مکمل ہونے پر Selectores locales ریجیکٹ
   ہوں گے۔
5. بلک ڈیٹا سیٹس کے لئے
   `iroha tools address audit --input addresses.txt --network-prefix 753` چلائیں۔ کمانڈ
   literales separados por nueva línea پڑھتی ہے ( `#` سے شروع ہونے والے comentarios نظرانداز
   ہوتے ہیں، اور `--input -` یا کوئی فلیگ نہ ہو تو STDIN استعمال ہوتا ہے), ہر
   اندراج کے لئے canonical/IH58 (ترجیحی)/compressed (`sora`) (`sora`, segundo mejor) resúmenes کے ساتھ JSON رپورٹ بناتی
   filas ہوں تو `--allow-errors` استعمال کریں، اور جب آپریٹرز Selectores locales کو
   CI میں بلاک کرنے کے لئے تیار ہوں تو `--fail-on-warning` سے آٹومیشن گیٹ کریں۔
6. Reescritura de nueva línea a nueva línea چاہیے تو
  Hojas de cálculo de corrección del selector local کے لئے
  استعمال کریں تاکہ `input,status,format,...` CSV برآمد ہو جو codificaciones canónicas,
  advertencias y errores de análisis کو ایک پاس میں نمایاں کرے۔ ayudante ڈیفالٹ طور پر
  filas no locales چھوڑ دیتا ہے، باقی entradas کو مطلوبہ codificación (IH58 ترجیحی/comprimido (`sora`) segundo mejor/hexadecimal/JSON)
  میں بدلتا ہے، اور `--append-domain` پر اصل ڈومین محفوظ رکھتا ہے۔ `--allow-errors`کے ساتھ جوڑیں تاکہ خراب literales والے volcados پر بھی escaneo جاری رہے۔
7. Automatización CI/lint `ci/check_address_normalize.sh` چلا سکتی ہے، جو
   `fixtures/account/address_vectors.json` سے Selectores locales نکال کر
   `iroha tools address normalize` سے تبدیل کرتی ہے، اور
   `iroha tools address audit --fail-on-warning` دوبارہ چلاتی ہے تاکہ ثابت ہو کہ
   lanzamientos اب Resúmenes locales نہیں نکالتے۔

`torii_address_local8_total{endpoint}` کے ساتھ
`torii_address_collision_total{endpoint,kind="local12_digest"}`,
Placa `torii_address_collision_domain_total{endpoint,domain}` y Grafana
Señal de cumplimiento `dashboards/grafana/address_ingest.json` دیتے ہیں: جب
paneles de producción مسلسل 30 دن تک صفر Envíos locales legítimos اور صفر
Colisiones Local-12 en la red principal Torii Puerta Local-8 en la red principal en caso de fallo total
گا، پھر Local-12 کو اس وقت جب dominios globales میں entradas de registro coincidentes ہوں۔
Congelar salida CLI y cadena de advertencia orientada al operador
Información sobre herramientas del SDK y automatización میں استعمال ہوتی ہے تاکہ criterios de salida de la hoja de ruta سے
ہے؛ diagnóstico de regresiones کرتے وقت صرف dev/test clusters میں اسے `false` کریں۔
`torii_address_domain_total{domain_kind}` y Grafana (`dashboards/grafana/address_ingest.json`)
میں mirror کرتے رہیں تاکہ Paquete de evidencia ADDR-7 یہ ثابت کر سکے کہ
selectores کو desactivar کرے۔ Paquete de administrador de alertas
(`dashboards/alerts/address_ingest_rules.yml`) تین barandillas شامل کرتا ہے:- `AddressLocal8Resurgence` اس وقت página کرتا ہے جب کوئی contexto نیا Local-8
  incremento رپورٹ کرے۔ implementaciones en modo estricto روکیں، panel de control میں que infringe el SDK
  کریں جب تک señal صفر نہ ہو جائے، پھر default (`true`) بحال کریں۔
- `AddressLocal12Collision` تب فائر ہوتا ہے جب دو Local-12 etiquetas ایک ہی resumen
  پر hash ہوں۔ promociones de manifiesto روکیں، Local -> Kit de herramientas global چلا کر resumen
  mapeo آڈٹ کریں، اور Nexus gobernanza کے ساتھ coordinar کریں اس سے پہلے کہ
  entrada de registro دوبارہ جاری ہو یا implementaciones posteriores بحال ہوں۔
- `AddressInvalidRatioSlo` خبردار کرتا ہے جب relación no válida para toda la flota (Local-8/
  rechazos en modo estricto کے بغیر) 10 منٹ تک 0.1% SLO سے بڑھ جائے۔
  `torii_address_invalid_total` استعمال کر کے متعلقہ contexto/razón کی نشاندہی
  کریں اور poseer SDK ٹیم کے ساتھ coordinar کر کے modo estricto دوبارہ فعال کریں۔

### ریلیز نوٹ اسنیپٹ (والٹ اور ایکسپلورر)

cutover کے وقت والٹ/ایکسپلورر ریلیز نوٹس میں درج ذیل bala شامل کریں:

> **Direcciones:** `iroha tools address normalize --only-local --append-domain` ayudante شامل
> کیا گیا اور اسے CI (`ci/check_address_normalize.sh`) میں وائر کیا گیا تاکہ
> میں تبدیل کر سکیں، قبل اس کے کہ Red principal Local-8/Local-12 پر بلاک ہوں۔ کسی بھی
> exportaciones personalizadas کو اپ ڈیٹ کریں تاکہ کمانڈ چلائی جائے اور lista normalizada کو
> publicar paquete de pruebas کے ساتھ منسلک کریں۔