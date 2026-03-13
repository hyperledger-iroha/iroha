---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-cli.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: desarrollador-cli
título: Libro de cocina CLI SoraFS
sidebar_label: libro de recetas CLI
descripción: Tutorial centrado en tareas de superficie consolidada `sorafs_cli` ۔
---

:::nota مستند ماخذ
:::

Superficie consolidada `sorafs_cli` (جو `sorafs_car` caja کے ذریعے `cli` característica کے ساتھ فراہم ہوتا ہے) SoraFS artefactos تیار کرنے کے لیے درکار ہر قدم exponer کرتا ہے۔ اس libro de cocina کو عام flujos de trabajo پر سیدھا جانے کے لیے استعمال کریں؛ contexto operativo کے لیے اسے canalización de manifiesto اور runbooks del orquestador کے ساتھ par کریں۔

## Cargas útiles del paquete

Archivos CAR deterministas y planes de fragmentos بنانے کے لیے `car pack` استعمال کریں۔ اگر manejar فراہم نہ ہو تو comando خودکار طور پر SF-1 fragmentador منتخب کرتا ہے۔

```bash
sorafs_cli car pack \
  --input fixtures/video.mp4 \
  --car-out artifacts/video.car \
  --plan-out artifacts/video.plan.json \
  --summary-out artifacts/video.car.json
```

- Identificador de fragmentación predeterminado: `sorafs.sf1@1.0.0`.
- Directorio de entradas orden lexicográfico میں walk ہوتے ہیں تاکہ sumas de comprobación مختلف پلیٹ فارمز پر بھی stable رہیں۔
- Resumen JSON de resúmenes de carga útil, metadatos de fragmentos, registro/orquestador, registro/orquestador, CID raíz, etc.

## Construir manifiestos

```bash
sorafs_cli manifest build \
  --summary artifacts/video.car.json \
  --pin-min-replicas 4 \
  --pin-storage-class hot \
  --pin-retention-epoch 96 \
  --manifest-out artifacts/video.manifest.to \
  --manifest-json-out artifacts/video.manifest.json
```

- Opciones de `--pin-*` براہ راست `sorafs_manifest::ManifestBuilder` میں `PinPolicy` campos سے mapa ہوتے ہیں۔
- `--chunk-plan` تب دیں جب آپ چاہتے ہوں کہ Envío CLI سے پہلے Resumen de fragmentos SHA3 دوبارہ Computación کرے؛ ورنہ وہ resumen میں incrustar شدہ reutilizar resumen کرتا ہے۔
- Salida JSON Carga útil Norito کی عکاسی کرتا ہے تاکہ reviews کے دوران diffs سیدھے ہوں۔## Firmar manifiestos sin claves de larga duración

```bash
sorafs_cli manifest sign \
  --manifest artifacts/video.manifest.to \
  --bundle-out artifacts/video.manifest.bundle.json \
  --signature-out artifacts/video.manifest.sig \
  --identity-token-env SIGSTORE_ID_TOKEN
```

- Tokens en línea, variables de entorno y fuentes basadas en archivos قبول کرتا ہے۔
- Metadatos de procedencia (`token_source`, `token_hash_hex`, resumen de fragmentos) ہو۔
- CI میں بہتر کام کرتا ہے: GitHub Actions OIDC کے ساتھ `--identity-token-provider=github-actions` استعمال کریں۔

## Enviar manifiestos a Torii

```bash
sorafs_cli manifest submit \
  --manifest artifacts/video.manifest.to \
  --chunk-plan artifacts/video.plan.json \
  --torii-url https://gateway.example/v1 \
  --authority i105... \
  --private-key ed25519:0123...beef \
  --alias-namespace sora \
  --alias-name video::launch \
  --alias-proof fixtures/alias_proof.bin \
  --summary-out artifacts/video.submit.json
```

- Pruebas de alias کے لیے Norito decodificación کرتا ہے اور Torii کو POST کرنے سے پہلے انہیں resumen de manifiesto سے coincidencia کرتا ہے۔
- Planificar resumen SHA3 de fragmentos, cálculo de کرتا ہے تاکہ ataques de desajuste
- Resúmenes de respuestas بعد کی auditoría کے لیے Estado HTTP, encabezados, اور cargas útiles del registro محفوظ کرتی ہیں۔

## Verificar el contenido y las pruebas del CAR

```bash
sorafs_cli proof verify \
  --manifest artifacts/video.manifest.to \
  --car artifacts/video.car \
  --summary-out artifacts/video.verify.json
```

- Árbol PoR دوبارہ بناتا ہے اور resúmenes de carga útil کو resumen del manifiesto کے ساتھ comparar کرتا ہے۔
- Pruebas de replicación کو gobernanza میں enviar کرتے وقت مطلوبہ recuentos اور identificadores captura کرتا ہے۔

## Telemetría a prueba de transmisiones

```bash
sorafs_cli proof stream \
  --manifest artifacts/video.manifest.to \
  --gateway-url https://gateway.example/v2/sorafs/proof/stream \
  --provider-id provider::alpha \
  --samples 32 \
  --stream-token "$(cat stream.token)" \
  --summary-out artifacts/video.proof_stream.json \
  --governance-evidence-dir artifacts/video.proof_stream_evidence
```- ہر prueba transmitida کے لیے Los elementos NDJSON emiten کرتا ہے (`--emit-events=false` سے repetición بند کریں)۔
- Recuentos de éxito/fracaso, histogramas de latencia, errores de muestra, resumen JSON, agregado, registros de paneles de control, raspado, uso de datos
- Informe de fallas de puerta de enlace کرے یا verificación PoR local (`--por-root-hex` کے ذریعے) pruebas rechazadas کرے تو salida distinta de cero دیتا ہے۔ se ejecuta el ensayo کے لیے `--max-failures` اور `--max-verification-failures` سے los umbrales se ajustan کریں۔
- آج PoR کو soporte کرتا ہے؛ PDP y PoTR SF-13/SF-14 con sobre y reutilización de sobres
- `--governance-evidence-dir` resumen renderizado, metadatos (marca de tiempo, versión CLI, URL de puerta de enlace, resumen de manifiesto), copia del manifiesto, directorio de prueba, prueba de flujo de paquetes de gobernanza, ejecución de prueba کیے بغیر archivo کر سکیں۔

## Referencias adicionales

- `docs/source/sorafs_cli.md` — تمام flags کی جامع دستاویزات۔
- `docs/source/sorafs_proof_streaming.md`: esquema de telemetría de prueba y plantilla de panel Grafana.
- `docs/source/sorafs/manifest_pipeline.md` — fragmentación, composición manifiesta, manejo de CAR پر تفصیلی جائزہ۔