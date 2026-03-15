---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/node-operations.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: operaciones de nodo
título: نوڈ آپریشنز رن بک
sidebar_label: نوڈ آپریشنز رن بک
descripción: Torii کے اندر ایمبیڈڈ `sorafs-node` ڈیپلائمنٹ کی توثیق کریں۔
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sorafs/runbooks/sorafs_node_ops.md` کی عکاسی کرتا ہے۔ جب تک پرانا Sphinx ڈاکیومنٹیشن سیٹ مکمل طور پر منتقل نہ ہو جائے، دونوں نقول کو ہم آہنگ رکھیں۔
:::

## جائزہ

یہ رن بک آپریٹرز کو Torii کے اندر ایمبیڈڈ `sorafs-node` ڈیپلائمنٹ کی توثیق میں
رہنمائی کرتی ہے۔ ہر سیکشن براہِ راست Entregables SF-3 سے میپ ہوتا ہے: pin/fetch
راؤنڈ ٹرپس، ری اسٹارٹ ریکوری، کوٹا ریجیکشن، اور PoR سیمپلنگ۔

## 1. پیشگی تقاضے

- `torii.sorafs.storage` میں اسٹوریج ورکر کو فعال کریں:

  ```toml
  [torii.sorafs.storage]
  enabled = true
  data_dir = "./storage/sorafs"
  max_capacity_bytes = 21474836480    # 20 GiB
  max_parallel_fetches = 32
  max_pins = 1000
  por_sample_interval_secs = 600

  [torii.sorafs.storage.metering_smoothing]
  gib_hours_enabled = true
  gib_hours_alpha = 0.25
  por_success_enabled = true
  por_success_alpha = 0.25
  ```

- یقینی بنائیں کہ Torii پروسس کے پاس `data_dir` تک lectura/escritura رسائی ہو۔
- declaración ریکارڈ ہونے کے بعد `GET /v1/sorafs/capacity/state` کے ذریعے تصدیق
  کریں کہ نوڈ متوقع کپیسٹی اعلان کرتا ہے۔
- جب suavizado فعال ہو، ڈیش بورڈز raw اور suavizado GiB·hora/PoR کاؤنٹرز دونوں
  دکھاتے ہیں تاکہ sin fluctuaciones رجحانات کو valores puntuales کے ساتھ نمایاں کیا جا سکے۔

### CLI ڈرائی رن (اختیاری)

Puntos finales HTTP ایکسپوز کرنے سے پہلے آپ CLI incluido کے ذریعے اسٹوریج backend کا
control de cordura کر سکتے ہیں۔【crates/sorafs_node/src/bin/sorafs-node.rs#L1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin

cargo run -p sorafs_node --bin sorafs-node export \
  --data-dir ./storage/sorafs \
  --manifest-id <hex> \
  --manifest-out ./out/manifest.to \
  --payload-out ./out/payload.bin
```یہ کمانڈز Norito Resúmenes JSON پرنٹ کرتی ہیں اور fragment-profile یا no coincidencia de resumen
کو مسترد کرتی ہیں، جس سے یہ Torii cableado سے پہلے Comprobaciones de humo de CI کے لیے مفید بنتی
ہیں۔【crates/sorafs_node/tests/cli.rs#L1】

### PoR پروف کی مشق

آپریٹرز اب گورننس کی طرف سے جاری کردہ PoR artefactos کو Torii پر اپ لوڈ کرنے سے
پہلے لوکل طور پر ری پلے کر سکتے ہیں۔ CLI اسی `sorafs-node` ingestión y reutilización
کرتی ہے، اس لیے لوکل رنز وہی errores de validación ظاہر کرتے ہیں جو HTTP API y
لوٹاتی ہے۔

```bash
cargo run -p sorafs_node --bin sorafs-node ingest por \
  --data-dir ./storage/sorafs \
  --challenge ./fixtures/sorafs_manifest/por/challenge_v1.to \
  --proof ./fixtures/sorafs_manifest/por/proof_v1.to \
  --verdict ./fixtures/sorafs_manifest/por/verdict_v1.to
```

کمانڈ ایک El resumen JSON emite کرتی ہے (resumen de manifiesto, identificación del proveedor, resumen de prueba,
recuento de muestras, o resultado del veredicto opcional)۔ `--manifest-id=<hex>` فراہم کریں تاکہ
اسٹور شدہ manifiesto چیلنج resumen سے میچ کرے، اور `--json-out=<path>` استعمال کریں
جب آپ resumen کو اصل artefactos کے ساتھ evidencia de auditoría کے طور پر آرکائیو کرنا
چاہیں۔ `--verdict` Utilice la API HTTP para configurar la configuración de la API → پروف
→ veredicto لوپ کی پوری مشق کر سکتے ہیں۔

Torii Dispositivos HTTP کے ذریعے حاصل کر سکتے ہیں:

```bash
curl -s http://$TORII/v1/sorafs/storage/manifest/$MANIFEST_ID_HEX | jq .
curl -s http://$TORII/v1/sorafs/storage/plan/$MANIFEST_ID_HEX | jq .plan.chunk_count
```

Puntos finales adicionales ایمبیڈڈ اسٹوریج ورکر فراہم کرتا ہے، اس لیے CLI smoke tests اور
sondas de puerta de enlace ہم آہنگ رہتے ہیں۔【crates/iroha_torii/src/sorafs/api.rs#L1207】【crates/iroha_torii/src/sorafs/api.rs#L1259】

## 2. Pin → Recuperar راؤنڈ ٹرپ1. manifiesto + carga útil بنڈل تیار کریں (مثلاً
   `iroha app sorafs toolkit pack ./payload.bin --manifest-out manifest.to --car-out payload.car --json-out manifest_report.json` کے ذریعے)۔
2. manifiesto con codificación base64:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/pin \
     -H 'Content-Type: application/json' \
     -d @pin_request.json
   ```

   solicitud JSON میں `manifest_b64` اور `payload_b64` شامل ہونا چاہیے۔ کامیاب
   respuesta `manifest_id_hex` اور resumen de carga útil واپس کرتی ہے۔
3. fijado ڈیٹا buscar کریں:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/fetch \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "offset": 0,
       "length": <payload length>
     }'
   ```

   `data_b64` فیلڈ کو base64 decodificación کریں اور تصدیق کریں کہ یہ اصل bytes سے میچ کرتی ہے۔

## 3. ری اسٹارٹ ریکوری ڈرل

1. اوپر دیے گئے طریقے کے مطابق کم از کم ایک pin de manifiesto کریں۔
2. Torii پروسس (یا پورا نوڈ) ری اسٹارٹ کریں۔
3. buscar ریکوئسٹ دوبارہ جمع کریں۔ carga útil بدستور دستیاب رہے اور واپس آنے والا
   digerir ری اسٹارٹ سے پہلے والی قدر کے مطابق ہو۔
4. `GET /v1/sorafs/storage/state` چیک کریں تاکہ تصدیق ہو کہ `bytes_used` ری بوٹ کے
   بعد manifiestos persistentes کو ظاہر کرتا ہے۔

## 4. کوٹا ریجیکشن ٹیسٹ

1. عارضی طور پر `torii.sorafs.storage.max_capacity_bytes` کو چھوٹی قدر پر لائیں
   (مثلاً ایک manifiesto کے سائز کے برابر)۔
2. Pin de manifiesto کریں؛ ریکوئسٹ کامیاب ہونی چاہیے۔
3. اسی طرح کے سائز والا دوسرا pin de manifiesto کرنے کی کوشش کریں۔ Torii کو ریکوئسٹ
   HTTP `400` کے ساتھ مسترد کرنی چاہیے اور ایرر میسج میں `storage capacity exceeded`
   شامل ہونا چاہیے۔
4. ختم ہونے پر نارمل کپیسٹی حد بحال کریں۔

## 5. PoR سیمپلنگ پروب

1. Pin de manifiesto کریں۔
2. Muestra de PoR ریکوئسٹ کریں:

   ```bash
   curl -X POST http://$TORII/v1/sorafs/storage/por-sample \
     -H 'Content-Type: application/json' \
     -d '{
       "manifest_id_hex": "<hex id from pin>",
       "count": 4,
       "seed": 12345
     }'
   ```3. تصدیق کریں کہ respuesta میں `samples` مطلوبہ cuenta کے ساتھ موجود ہیں اور ہر
   prueba اسٹور شدہ raíz manifiesta کے خلاف ویریفائی ہوتا ہے۔

## 6. آٹومیشن ہکس

- Pruebas de CI/humo درج ذیل میں شامل ٹارگٹڈ چیکس دوبارہ استعمال کر سکتے ہیں:

  ```bash
  cargo test -p sorafs_node --test pin_workflows
  ```

  Aquí `pin_fetch_roundtrip`, `pin_survives_restart`, `pin_quota_rejection`, y
  `por_sampling_returns_verified_proofs` کو کور کرتے ہیں۔
- ڈیش بورڈز کو یہ ٹریک کرنا چاہیے:
  - `torii_sorafs_storage_bytes_used / torii_sorafs_storage_bytes_capacity`
  - `torii_sorafs_storage_pin_queue_depth` y `torii_sorafs_storage_fetch_inflight`
  - `/v1/sorafs/capacity/state` کے ذریعے ظاہر کیے گئے PoR کامیابی/ناکامی کاؤنٹرز
  - `sorafs_node_deal_publish_total{result=success|failure}` کے ذریعے intentos de publicación de liquidación

ان ejercicios کی پیروی سے یہ یقینی ہوتا ہے کہ ایمبیڈڈ اسٹوریج ورکر ڈیٹا ingerir کر
سکے، ری اسٹارٹس برداشت کرے، مقررہ کوٹاز کا احترام کرے، اور نوڈ کے وسیع نیٹ ورک
کو کپیسٹی publicidad کرنے سے پہلے ڈٹرمنسٹک Pruebas PoR تیار کرے۔