---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: ceremonia de firma
título: دستخطی تقریب کی جگہ نیا عمل
descripción: Sora پارلیمنٹ کس طرح SoraFS accesorios fragmentadores کی منظوری اور تقسیم کرتی ہے (SF-1b).
sidebar_label: دستخطی تقریب
---

> Hoja de ruta: **SF-1b — Aprobaciones de accesorios del Parlamento de Sora.**
> پارلیمنٹ کا ورک فلو پرانی آف لائن "کونسل دستخطی تقریب" کی جگہ لیتا ہے۔

Accesorios de fragmentación SoraFS کے لیے دستی دستخطی رسم ریٹائر کر دی گئی ہے۔ اب تمام منظوری
**Parlamento de Sora** کے ذریعے ہوتی ہے، جو Nexus کو گورن کرنے y DAO basado en clasificación ہے۔
Bono XOR Bono XOR Bono XOR Bono XOR Bono XOR Bono XOR Bono XOR Bono XOR Bono XOR Bono XOR Bono XOR Bono XOR
اور en cadena ووٹس کے ذریعے accesorios ریلیزز کو منظور، مسترد یا رول بیک کرتے ہیں۔
یہ گائیڈ عمل اور herramientas de desarrollador کی وضاحت کرتی ہے۔

## پارلیمنٹ کا جائزہ

- **شہریت** — آپریٹرز مطلوبہ Enlace XOR کر کے شہری بنتے ہیں اور sortition کے اہل ہوتے ہیں۔
- **پینلز** — ذمہ داریاں گردش کرنے والے پینلز میں تقسیم ہیں (Infraestructura,
  Moderación, Hacienda, ...). Panel de infraestructura SoraFS aprobaciones de accesorios کا ذمہ دار ہے۔
- **Clasificación y rotación** — پینل سیٹس پارلیمنٹ دستور میں متعین cadencia پر دوبارہ
  قرعہ اندازی سے منتخب ہوتے ہیں تاکہ کوئی ایک گروہ منظوریوں پر اجارہ داری نہ رکھ سکے۔

## Flujo de aprobación de accesorios1. **Envío de propuesta**
   - Conjunto de herramientas WG `manifest_blake3.json` y diferenciación de accesorios `sorafs.fixtureProposal`
     کے ذریعے registro en cadena میں اپلوڈ کرتا ہے۔
   - پروپوزل BLAKE3 digest, versión semántica اور تبدیلی نوٹس ریکارڈ کرتا ہے۔
2. **Revisar y votar**
   - Panel de infraestructura پارلیمنٹ cola de tareas کے ذریعے اسائنمنٹ وصول کرتا ہے۔
   - پینل ممبرز artefactos de CI دیکھتے ہیں، pruebas de paridad چلاتے ہیں، اور votos ponderados en cadena ڈالتے ہیں۔
3. **Finalización**
   - جب quórum پورا ہو جائے تو runtime ایک evento de aprobación جاری کرتا ہے جس میں resumen de manifiesto canónico
     اور carga útil del accesorio کے لیے Compromiso de Merkle شامل ہوتا ہے۔
   - یہ evento SoraFS registro میں mirror کیا جاتا ہے تاکہ کلائنٹس تازہ ترین Manifiesto aprobado por el Parlamento حاصل کر سکیں۔
4. **Distribución**
   - Ayudantes de CLI (`cargo xtask sorafs-fetch-fixture`) Nexus RPC سے منظور شدہ manifest کھینچتے ہیں۔
     ریپو کے JSON/TS/Go constantes `export_vectors` دوبارہ چلا کر اور digest کو on-chain ریکارڈ کے
     مقابل validar کر کے sincronización رہتے ہیں۔

## Flujo de trabajo del desarrollador

- Calendario دوبارہ بنائیں:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- Ayudante de búsqueda del Parlamento استعمال کریں تاکہ منظور شدہ sobre ڈاؤن لوڈ ہو، firmas verificar ہوں،
  اور مقامی los accesorios se actualizan ہوں۔ `--signatures` کو Parlamento کے شائع کردہ sobre پر پوائنٹ کریں؛
  ayudante متعلقہ resolución manifiesta کرتا ہے، BLAKE3 digest دوبارہ حساب کرتا ہے، اور canónico
  `sorafs.sf1@1.0.0` perfil نافذ کرتا ہے۔```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

Manifiesto کسی اور URL پر ہو تو `--manifest` پاس کریں۔ غیر دستخط شدہ sobres رد کر دیے جاتے ہیں
جب تک مقامی humo corre کے لیے `--allow-unsigned` سیٹ نہ ہو۔

- Puerta de enlace de preparación کے ذریعے validación de manifiesto کرنے کے لیے مقامی cargas útiles کے بجائے Torii کو ہدف بنائیں:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- مقامی CI اب `signer.json` lista کا تقاضا نہیں کرتا۔
  `ci/check_sorafs_fixtures.sh` repositorio کی حالت کو تازہ ترین compromiso en cadena سے موازنہ کرتا ہے اور
  فرق ہونے پر fallar کر دیتا ہے۔

## Notas de gobernanza

- پارلیمنٹ دستور quórum, rotación اور escalada کو gobernar کرتا ہے — configuración a nivel de caja درکار نہیں۔
- ہنگامی reversiones پارلیمنٹ panel de moderación کے ذریعے سنبھالے جاتے ہیں۔ Panel de infraestructura ایک revertir
  propuesta فائل کرتا ہے جو پچھلے resumen manifiesto کو حوالہ دیتا ہے، اور منظوری کے بعد lanzamiento بدل دی جاتی ہے۔
- تاریخی aprobaciones SoraFS registro میں repetición forense کے لیے دستیاب رہتے ہیں۔

## Preguntas frecuentes

- **`signer.json` کہاں گیا؟**  
  اسے ہٹا دیا گیا ہے۔ تمام atribución del firmante en cadena موجود ہے؛ ریپو میں `manifest_signatures.json`
  صرف accesorio del desarrollador ہے جو آخری evento de aprobación سے میچ ہونا چاہیے۔

- **کیا اب بھی مقامی Ed25519 firmas درکار ہیں؟**  
  نہیں۔ El Parlamento aprueba artefactos en cadena کے طور پر محفوظ ہوتے ہیں۔ Calendario de مقامی
  reproducibilidad کے لیے ہوتے ہیں مگر Resumen del Parlamento کے خلاف validar کیے جاتے ہیں۔- **ٹیمیں aprobaciones کیسے مانیٹر کرتی ہیں؟**  
  `ParliamentFixtureApproved` evento کو suscripción کریں یا Nexus RPC کے ذریعے registro کو consulta کریں
  تاکہ موجودہ resumen de manifiesto اور pase de lista del panel حاصل کیا جا سکے۔