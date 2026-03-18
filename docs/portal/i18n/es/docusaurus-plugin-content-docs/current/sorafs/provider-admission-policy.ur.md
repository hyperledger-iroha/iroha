---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/provider-admission-policy.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> [`docs/source/sorafs/provider_admission_policy.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_admission_policy.md) سے ماخوذ۔

# SoraFS فراہم کنندگان کی قبولیت اور شناخت کی پالیسی (SF-2b مسودہ)

یہ نوٹ **SF-2b** کے لیے قابلِ عمل ڈیلیوریبلز کو سمیٹتا ہے: SoraFS اسٹوریج فراہم Cargas útiles y cargas útiles یہ SoraFS Arquitectura RFC میں بیان کردہ اعلی سطح کے عمل کو وسعت دیتا ہے اور باقی کام کو قابلِ ٹریک انجینئرنگ ٹاسکس میں تقسیم کرتا ہے۔

## پالیسی کے اہداف

- یہ یقینی بنانا کہ صرف تصدیق شدہ آپریٹرز ہی `ProviderAdvertV1` ریکارڈز شائع کر سکیں جنہیں نیٹ ورک قبول کرے۔
- ہر اعلان کلید کو گورننس سے منظور شدہ شناختی دستاویز، توثیق شدہ endpoints, اور کم از کم شراکت کے ساتھ باندھنا۔
- Torii, puertas de enlace, اور `sorafs-node` ایک ہی چیک نافذ کریں اس کے لیے ڈیٹرمنسٹک ویریفیکیشن ٹولنگ فراہم کرنا۔
- ڈیٹرمنزم یا ٹولنگ ergonomía کو توڑے بغیر تجدید اور ہنگامی منسوخی کی حمایت کرنا۔

## شناخت اور participación تقاضے| تقاضا | وضاحت | ڈیلیوریبل |
|-------|-------|-----------|
| اعلان کلید کا ماخذ | فراہم کنندگان کو Ed25519 keypair رجسٹر کرنا ہوگا جو ہر anuncio پر دستخط کرے۔ paquete de admisión گورننس دستخط کے ساتھ پبلک key محفوظ کرتا ہے۔ | `ProviderAdmissionProposalV1` اسکیمہ میں `advert_key` (32 bytes) شامل کریں اور اسے رجسٹری (`sorafs_manifest::provider_admission`) سے ریفرنس کریں۔ |
| Estaca پوائنٹر | قبولیت کے لیے فعال grupo de apuestas کی طرف اشارہ کرنے والا غیر صفر `StakePointer` درکار ہے۔ | `sorafs_manifest::provider_advert::StakePointer::validate()` Errores de configuración y pruebas de CLI/pruebas |
| Jurisdicción ٹیگز | فراہم کنندگان jurisdicción + قانونی رابطہ ظاہر کرتے ہیں۔ | پروپوزل اسکیمہ میں `jurisdiction_code` (ISO 3166-1 alfa-2) اور اختیاری `contact_uri` شامل کریں۔ |
| Punto final توثیق | ہر اعلان شدہ endpoint کو mTLS یا QUIC سرٹیفکیٹ رپورٹ سے سپورٹ کرنا لازم ہے۔ | Norito carga útil `EndpointAttestationV1` کی تعریف کریں اور اسے paquete de admisión کے اندر ہر punto final کے ساتھ اسٹور کریں۔ |

## قبولیت کا ورک فلو1. **پروپوزل تیار کرنا**
   - CLI: `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission proposal ...`
     Paquete de software `ProviderAdmissionProposalV1` + paquete de software بنائے۔
   - ویلیڈیشن: ضروری فیلڈز، apuesta > 0, اور `profile_id` میں mango de fragmentador canónico کو یقینی بنائیں۔
2. **گورننس کی منظوری**
   - کونسل موجودہ herramientas de envolvente (ماڈیول `sorafs_manifest::governance`) استعمال کرتے ہوئے
     `blake3("sorafs-provider-admission-v1" || canonical_bytes)` پر دستخط کرتی ہے۔
   - Sobre کو `governance/providers/<provider_id>/admission.json` میں محفوظ کیا جاتا ہے۔
3. **رجسٹری میں اندراج**
   - ایک مشترکہ verifier (`sorafs_manifest::provider_admission::validate_envelope`) نافذ کریں جسے Torii/gateways/CLI دوبارہ استعمال کریں۔
   - Torii کے admisión راستے کو اپڈیٹ کریں تاکہ وہ ایسے anuncios کو مسترد کرے جن کا resumen یا sobre de vencimiento سے مختلف ہو۔
4. **تجدید اور منسوخی**
   - Punto final/participación اپڈیٹس کے ساتھ `ProviderAdmissionRenewalV1` شامل کریں۔
   - ایک CLI راستہ `--revoke` فراہم کریں جو منسوخی کی وجہ ریکارڈ کرے اور گورننس ایونٹ بھیجے۔

## عمل درآمد کے کام

| علاقہ | کام | Propietario(s) | حالت |
|-------|-----|----------|------|
| اسکیمہ | `crates/sorafs_manifest/src/provider_admission.rs` کے تحت `ProviderAdmissionProposalV1`, `ProviderAdmissionEnvelopeV1`, `EndpointAttestationV1` (Norito) کی تعریف کریں۔ `sorafs_manifest::provider_admission` Ayudantes de میں ویلیڈیشن کے ساتھ نافذ کیا گیا ہے۔【F:crates/sorafs_manifest/src/provider_admission.rs#L1】 | Almacenamiento / Gobernanza | ✅ مکمل |
| CLI ٹولنگ | `sorafs_manifest_stub` Número de modelo: `provider-admission proposal`, `provider-admission sign`, `provider-admission verify`. | Grupo de Trabajo sobre Herramientas | ✅ مکمل |Paquetes CLI para درمیانی سرٹیفکیٹ (`--endpoint-attestation-intermediate`) قبول کرتا ہے،
bytes canónicos de propuesta/sobre آپریٹرز cuerpos de anuncios براہ راست فراہم کر سکتے ہیں یا anuncios firmados دوبارہ استعمال کر سکتے ہیں، اور archivos de firma کو `--council-signature-public-key` کے ساتھ `--council-signature-file` جوڑ کر فراہم کیا جا سکتا ہے تاکہ automatización آسان ہو۔

### CLI حوالہ

ہر کمانڈ کو `cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission ...` کے ذریعے چلائیں۔- `proposal`
  - Número de modelo: `--provider-id=<hex32>`, `--chunker-profile=<namespace.name@semver>`,
    `--stake-pool-id=<hex32>`, `--stake-amount=<amount>`, `--advert-key=<hex32>`,
    `--jurisdiction-code=<ISO3166-1>`, اور کم از کم ایک `--endpoint=<kind:host>`۔
  - ہر punto final کے لیے توثیق میں `--endpoint-attestation-attested-at=<secs>`,
    `--endpoint-attestation-expires-at=<secs>`, ایک سرٹیفکیٹ بذریعہ
    `--endpoint-attestation-leaf=<path>` (ہر چین عنصر کے لیے اختیاری `--endpoint-attestation-intermediate=<path>`) اور
    کوئی بھی مذاکرات شدہ ALPN ID (`--endpoint-attestation-alpn=<token>`) شامل ہیں۔ Puntos finales QUIC نقل و حمل رپورٹس
    `--endpoint-attestation-report[-hex]=...` کے ذریعے فراہم کر سکتے ہیں۔
  - Ejemplo: bytes de propuesta canónica Norito (`--proposal-out`) en formato JSON
    (ڈیفالٹ salida estándar یا `--json-out`).
- `sign`
  - ان پٹس: ایک پروپوزل (`--proposal`), ایک anuncio firmado (`--advert`), اختیاری cuerpo del anuncio
    (`--advert-body`), época de retención, اور کم از کم ایک کونسل firma۔ firmas کو en línea
    (`--council-signature=<signer_hex:signature_hex>`) یا فائلز کے ذریعے فراہم کیا جا سکتا ہے جب
    `--council-signature-public-key` کو `--council-signature-file=<path>` کے ساتھ ملایا جائے۔
  - Sobre validado (`--envelope-out`) y formato JSON, y enlaces de resumen, recuento de firmantes, y rutas de entrada.
- `verify`
  - Sobre (`--envelope`) کی تصدیق کرتا ہے، اور اختیاری طور پر propuesta coincidente, anuncio, یا cuerpo del anuncio چیک کرتا ہے۔ Valores de resumen JSON, estado de verificación de firma, artefactos opcionales دکھاتی ہے جو coincidencia ہوئے۔
- `renewal`- نئے منظور شدہ sobre کو پہلے سے منظور شدہ resumen کے ساتھ جوڑتا ہے۔ اس کے لیے
    `--previous-envelope=<path>` اور جانشین `--envelope=<path>` (دونوں Norito cargas útiles) درکار ہیں۔
    CLI incluye alias de perfil, capacidades, claves de publicidad, puntos finales, metadatos اپڈیٹس کی اجازت دیتا ہے۔ bytes canónicos `ProviderAdmissionRenewalV1` (`--renewal-out`) y JSON خلاصہ آؤٹ پٹ ہوتا ہے۔
- `revoke`
  - ایسے proveedor کے لیے ہنگامی `ProviderAdmissionRevocationV1` paquete جاری کرتا ہے جس کا sobre واپس لینا ضروری ہو۔
    `--envelope=<path>`, `--reason=<text>`, کم از کم ایک `--council-signature` درکار ہے، اور
    `--revoked-at`/`--notes` اختیاری ہیں۔ Resumen de revocación de CLI para firmar/verificar carga útil Norito
    `--revocation-out` کے ذریعے لکھتا ہے، اور digest اور recuento de firmas کے ساتھ JSON رپورٹ پرنٹ کرتا ہے۔
| ویریفیکیشن | Torii, puertas de enlace, اور `sorafs-node` کے لیے مشترکہ verificador نافذ کریں۔ unidad + pruebas de integración CLI فراہم کریں۔【F:crates/sorafs_manifest/src/provider_admission.rs#L1】【F:crates/iroha_torii/src/sorafs/admission.rs#L1】 | Redes TL / Almacenamiento | ✅ مکمل || Torii Hombre | verificador کو Torii میں ingestión de anuncios کرنے کے دوران شامل کریں، پالیسی سے باہر anuncios مسترد کریں، اور telemetría جاری کریں۔ | Redes TL | ✅ مکمل | Torii اب sobres de gobernanza (`torii.sorafs.admission_envelopes_dir`) لوڈ کرتا ہے، ingest کے دوران resumen/coincidencia de firma کی تصدیق کرتا ہے، اور telemetría de admisión ظاہر کرتا ہے。【F:crates/iroha_torii/src/sorafs/admission.rs#L1】【F:crates/iroha_torii/src/sorafs/discovery.rs#L1】【F:crates/iroha_torii/src/sorafs/api.rs#L1】 |
| تجدید | تجدید/منسوخی اسکیمہ + CLI helpers شامل کریں، اور guía del ciclo de vida کو docs میں شائع کریں (نیچے runbook اور `provider-admission renewal`/`revoke` Tarjeta CLI دیکھیں)。【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【docs/source/sorafs/provider_admission_policy.md:120】 | Almacenamiento / Gobernanza | ✅ مکمل |
| ٹیلیمیٹری | Paneles de control `provider_admission` اور alertas کی تعریف کریں (تجدید کی کمی، vencimiento del sobre)۔ | Observabilidad | 🟠 جاری | کاؤنٹر `torii_sorafs_admission_total{result,reason}` موجود ہے؛ paneles/alertas زیر التوا ہیں۔【F:crates/iroha_telemetry/src/metrics.rs#L3798】【F:docs/source/telemetry.md#L614】 |

### تجدید اور منسوخی کا رن بُک#### شیڈول شدہ تجدید (participación/topología اپڈیٹس)
1. `provider-admission proposal` اور `provider-admission sign` کے ذریعے جانشین propuesta/anuncio جوڑا بنائیں،
   `--retention-epoch` بڑھائیں اور ضرورت کے مطابق estacas/puntos finales اپڈیٹ کریں۔
2. چلائیں
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     renewal \
     --previous-envelope=governance/providers/<id>/envelope.to \
     --envelope=governance/providers/<id>/envelope_next.to \
     --renewal-out=governance/providers/<id>/renewal.to \
     --json-out=governance/providers/<id>/renewal.json \
     --notes="stake top-up 2025-03"
   ```
   یہ کمانڈ `AdmissionRecord::apply_renewal` کے ذریعے capacidad/perfil فیلڈز کو غیر تبدیل شدہ ہونے کی تصدیق کرتی ہے،
   `ProviderAdmissionRenewalV1` جاری کرتی ہے، اور گورننس لاگ کے لیے digests پرنٹ کرتی ہے۔【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L477】【F:crates/sorafs_manifest/src/provider_admission.rs#L422】
3. `torii.sorafs.admission_envelopes_dir` میں پچھلا sobre تبدیل کریں، renovación Norito/JSON کو گورننس ریپوزٹری میں commit کریں،
   Hash de renovación + época de retención کو `docs/source/sorafs/migration_ledger.md` میں شامل کریں۔
4. آپریٹرز کو بتائیں کہ نیا sobre فعال ہے اور ingest کی تصدیق کے لیے
   `torii_sorafs_admission_total{result="accepted",reason="stored"}` Pantalla táctil
5. `cargo run -p sorafs_car --bin provider_admission_fixtures --features cli` کے ذریعے accesorios canónicos دوبارہ بنائیں اور commit کریں؛
   CI (`ci/check_sorafs_fixtures.sh`) Norito آؤٹ پٹس کی estabilidad چیک کرتا ہے۔#### ہنگامی منسوخی
1. متاثرہ sobre کی شناخت کریں اور منسوخی جاری کریں:
   ```bash
   cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- provider-admission \
     revoke \
     --envelope=governance/providers/<id>/envelope.to \
     --reason="endpoint compromise" \
     --revoked-at=$(date +%s) \
     --notes="incident-456" \
     --council-signature=<signer_hex:signature_hex> \
     --revocation-out=governance/providers/<id>/revocation.to \
     --json-out=governance/providers/<id>/revocation.json
   ```
   CLI `ProviderAdmissionRevocationV1` پر دستخط کرتا ہے، `verify_revocation_signatures` کے ذریعے firmas کا سیٹ ویریفائی کرتا ہے،
   اور resumen de revocación رپورٹ کرتا ہے۔【crates/sorafs_car/src/bin/sorafs_manifest_stub/provider_admission.rs#L593】【F:crates/sorafs_manifest/src/provider_admission.rs#L486】
2. `torii.sorafs.admission_envelopes_dir` سے sobre ہٹا دیں، revocación Norito/JSON کو cachés de admisión میں تقسیم کریں،
   اور وجہ کا hash گورننس منٹس میں ریکارڈ کریں۔
3. `torii_sorafs_admission_total{result="rejected",reason="admission_missing"}` دیکھیں تاکہ تصدیق ہو سکے کہ cachés نے anuncio revocado کو soltar کر دیا ہے؛
   artefactos de revocación کو retrospectivas de incidentes میں محفوظ رکھیں۔

## ٹیسٹنگ اور ٹیلیمیٹری- propuestas de admisión اور sobres کے لیے accesorios dorados کو `fixtures/sorafs_manifest/provider_admission/` کے تحت شامل کریں۔
- CI (`ci/check_sorafs_fixtures.sh`) کو وسعت دیں تاکہ propuestas دوبارہ بنائے جائیں اور sobres ویریفائی ہوں۔
- تیار شدہ accesorios میں resúmenes canónicos کے ساتھ `metadata.json` شامل ہوتا ہے؛ pruebas posteriores یہ afirmar کرتے ہیں کہ
  `proposal_digest_hex` == `ca8e73a1f319ae83d7bd958ccb143f9b790c7e4d9c8dfe1f6ad37fa29facf936`.
- pruebas de integración فراہم کریں:
  - Torii ایسے anuncios مسترد کرتا ہے جن کے sobres de admisión غائب یا میعاد ختم ہو چکے ہوں۔
  - Propuesta CLI → sobre → verificación کا ida y vuelta چلاتا ہے۔
  - گورننس تجدید ID de proveedor تبدیل کیے بغیر punto final توثیق کو rotar کرتی ہے۔
- ٹیلیمیٹری کی ضروریات:
  - Torii میں `provider_admission_envelope_{accepted,rejected}` کاؤنٹرز emite کریں۔ ✅ `torii_sorafs_admission_total{result,reason}` اب aceptado/rechazado نتائج دکھاتا ہے۔
  - paneles de observabilidad میں advertencias de vencimiento شامل کریں (7 دن کے اندر تجدید درکار ہونے پر)۔

## اگلے اقدامات1. ✅ Norito اسکیمہ تبدیلیاں حتمی ہو چکی ہیں اور `sorafs_manifest::provider_admission` میں ayudantes de validación شامل ہو گئے ہیں۔ کسی banderas de funciones کی ضرورت نہیں۔
2. ✅ CLI فلو (`proposal`, `sign`, `verify`, `renewal`, `revoke`) دستاویزی ہیں اور pruebas de integración گزارے گئے ہیں؛ گورننس اسکرپٹس کو runbook کے ساتھ ہم آہنگ رکھیں۔
3. ✅ Torii ingesta de sobres de admisión/descubrimiento کرتا ہے اور قبولیت/رد کے contadores de telemetría دکھاتا ہے۔
4. observabilidad پر توجہ: paneles de control/alertas de admisión مکمل کریں تاکہ سات دن کے اندر واجب التجدید معاملات پر وارننگ ہو (`torii_sorafs_admission_total`, medidores de caducidad)۔