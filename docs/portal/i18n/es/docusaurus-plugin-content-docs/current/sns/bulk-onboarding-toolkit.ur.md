---
lang: es
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sns/bulk_onboarding_toolkit.md` کی عکاسی کرتا ہے تاکہ بیرونی
آپریٹرز ریپو کلون کئے بغیر وہی SN-3b رہنمائی دیکھ سکیں۔
:::

# Kit de herramientas de incorporación masiva de SNS (SN-3b)

**روڈمیپ حوالہ:** SN-3b "Herramientas de incorporación masiva"  
**Artefactos:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

بڑے registradores اکثر `.sora` یا `.nexus` registros کو ایک ہی gobernanza
aprobaciones اور carriles de liquidación کے ساتھ سیکڑوں کی تعداد میں پہلے سے تیار کرتے
ہیں۔ دستی طور پر JSON payloads بنانا یا CLI دوبارہ چلانا scale نہیں کرتا، اس
SN-3b es un generador determinista de CSV a Norito basado en Torii y CLI
کے لئے `RegisterNameRequestV1` estructuras تیار کرتا ہے۔ ayudante ہر fila کو پہلے
validar el manifiesto agregado y la opción JSON delimitada por nueva línea opcional
کرتا ہے، اور auditorías کے لئے recibos estructurados ریکارڈ کرتے ہوئے cargas útiles کو
خودکار طور پر enviar کر سکتا ہے۔

## 1. CSV اسکیمہ

Analizador کو درج ذیل fila de encabezado درکار ہے (orden flexible ہے):| Columna | Requerido | Descripción |
|--------|----------|-------------|
| `label` | Sí | Etiqueta solicitada (se aceptan mayúsculas y minúsculas mixtas; herramienta Norm v1 اور UTS-46 کے مطابق normalize کرتا ہے). |
| `suffix_id` | Sí | Identificador de sufijo numérico (decimal یا `0x` hexadecimal). |
| `owner` | Sí | AccountId string (domainless encoded literal; canonical Katakana i105 only; no `@<domain>` suffix). |
| `term_years` | Sí | Entero `1..=255`. |
| `payment_asset_id` | Sí | Activo de liquidación (مثال `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | Sí | Los enteros sin signo y las unidades nativas de activos representan کریں۔ |
| `settlement_tx` | Sí | Valor JSON یا cadena literal جو transacción de pago یا hash بیان کرے۔ |
| `payment_payer` | Sí | AccountId جس نے autorización de pago کی۔ |
| `payment_signature` | Sí | JSON یا cadena literal جس میں prueba de firma de administrador/tesorería ہو۔ |
| `controllers` | Opcional | Direcciones de cuentas del controlador کی punto y coma/lista separada por comas ۔ خالی ہونے پر `[owner]`. |
| `metadata` | Opcional | JSON en línea `@path/to/file.json`, sugerencias de resolución, registros TXT y más Predeterminado `{}`۔ |
| `governance` | Opcional | JSON en línea como `@path` y `GovernanceHookV1` en línea `--require-governance` اس columna کو لازمی کرتا ہے۔ |

کوئی بھی columna سیل ویلیو میں `@` لگا کر archivo externo کو refer کر سکتا ہے۔
Rutas de archivo CSV کے resolución relativa ہوتے ہیں۔

## 2. Ayudante چلانا```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Opciones clave:

- `--require-governance` gancho de gobierno کے بغیر filas rechazan کرتا ہے (premium
  subastas یا asignaciones reservadas کے لئے مفید).
- `--default-controllers {owner,none}` طے کرتا ہے کہ خالی propietario de las celdas del controlador
  cuenta پر واپس جائیں یا نہیں.
- `--controllers-column`, `--metadata-column`, y `--governance-column` aguas arriba
  exportaciones کے ساتھ کام کرتے ہوئے columnas opcionales کے نام بدلنے دیتے ہیں.

کامیابی پر script de manifiesto agregado لکھتا ہے:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "soraカタカナ...",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"soraカタカナ...","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"soraカタカナ...",
        "signature":"alpha-signature"
      },
      "governance": null,
      "metadata":{"notes":"alpha cohort"}
    }
  ],
  "summary": {
    "total_requests": 120,
    "total_gross_amount": 28800,
    "total_net_amount": 28800,
    "suffix_breakdown": {"1":118,"42":2}
  }
}
```

اگر `--ndjson` دیا جائے تو ہر `RegisterNameRequestV1` کو JSON de una sola línea
documento کے طور پر بھی لکھا جاتا ہے تاکہ solicitudes de automatizaciones کو براہ راست Torii
میں corriente کر سکیں:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. Envíos automatizados

### 3.1 Torii Modo DESCANSO

`--submit-torii-url` کے ساتھ `--submit-token` یا `--submit-token-file` دیں تاکہ
manifiesto کی ہر entrada براہ راست Torii کو push ہو:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- Solicitud de ayuda کے لئے `POST /v1/sns/names` بھیجتا ہے اور پہلی HTTP
  error پر abortar کرتا ہے۔ Ruta del registro de respuestas میں registros NDJSON کے طور پر anexar
  ہوتے ہیں۔
- `--poll-status` ہر envío کے بعد `/v1/sns/names/{namespace}/{literal}` کو
  دوبارہ consulta کرتا ہے (زیادہ سے زیادہ `--poll-attempts`, predeterminado 5) تاکہ registro
  visible ہونے کی تصدیق ہو۔ `--suffix-map` (JSON y `suffix_id` y valores de "sufijo"
  سے map کرے) فراہم کریں تاکہ herramienta `{label}.{suffix}` literales derivan کر سکے۔
- Sintonizables: `--submit-timeout`, `--poll-attempts`, `--poll-interval`.

### 3.2 modo CLI de irohaLa entrada del manifiesto en la CLI requiere la ruta binaria siguiente:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Controladores کو `Account` entradas ہونا چاہئے (`controller_type.kind = "Account"`)
  کیونکہ CLI فی الحال صرف Los controladores basados en cuentas exponen کرتا ہے۔
- Metadatos اور blobs de gobernanza ہر solicitud کے لئے archivos temporales میں لکھے جاتے
  ہیں اور `iroha sns register --metadata-json ... --governance-json ...` کو پاس
  کئے جاتے ہیں۔
- CLI stdout/stderr y registro de códigos de salida ہوتے ہیں؛ se ejecutan códigos distintos de cero کو abortar کرتے ہیں۔

دونوں modos de envío ایک ساتھ چل سکتے ہیں (Torii اور CLI) تاکہ registrador
las implementaciones verifican ہوں یا las alternativas ensayan کئے جائیں۔

### 3.3 Recibos de envío

Aquí `--submission-log <path>` escribe las entradas NDJSON del script anexadas a:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Respuestas exitosas de Torii میں `NameRecordV1` یا `RegisterNameResponseV1` سے نکالے
گئے campos estructurados شامل ہوتے ہیں (مثال `record_status`, `record_pricing_class`,
`record_owner`, `record_expires_at_ms`, `registry_event_version`, `suffix_id`,
`label`) Paneles de control, registros de informes de gobernanza y texto de formato libre
analizar کر سکیں۔ اس registrar کو manifiesto کے ساتھ tickets de registrador پر adjuntar کریں تاکہ
evidencia reproducible رہے۔

## 4. Automatización del lanzamiento del portal de documentos

CI اور portal trabajos `docs/portal/scripts/sns_bulk_release.sh` کو llamada کرتے ہیں، جو
ayudante کو wrap کرتا ہے اور artefactos کو `artifacts/sns/releases/<timestamp>/` کے
Tienda تحت کرتا ہے:

```bash
docs/portal/scripts/sns_bulk_release.sh \
  --csv assets/sns/registrations_2026q2.csv \
  --torii-url https://torii.sora.network \
  --token-env SNS_TORII_TOKEN \
  --suffix-map configs/sns_suffix_map.json \
  --poll-status \
  --cli-path ./target/release/iroha \
  --cli-config configs/registrar.toml
```

Guión:1. `registrations.manifest.json`, `registrations.ndjson` بناتا ہے اور original
   CSV کو directorio de lanzamiento میں کاپی کرتا ہے۔
2. Torii اور/یا CLI کے ذریعے envío de manifiesto کرتا ہے (جب configure ہو)، اور
   `submissions.log` میں اوپر والے recibos estructurados لکھتا ہے۔
3. `summary.json` emite کرتا ہے جو libera کو describe کرتا ہے (rutas, Torii URL,
   Ruta CLI, marca de tiempo) Paquete de automatización del portal y almacenamiento de artefactos
   subir کر سکے۔
4. `metrics.prom` بناتا ہے (`--metrics` کے ذریعے anulación), جس میں Prometheus-
   contadores de formato ہوتے ہیں: solicitudes totales, distribución de sufijos, totales de activos,
   اور resultados de envío۔ resumen JSON archivo کی طرف enlace کرتا ہے۔

Flujos de trabajo صرف directorio de lanzamiento کو ایک artefacto کے طور پر archive کرتے ہیں، جس میں
اب وہ سب کچھ ہے جو gobernanza کو auditoría کے لئے درکار ہے۔

## 5. Telemetría y paneles de control

`sns_bulk_release.sh` کے ذریعہ بننے والی archivo de métricas درج ذیل series exponen کرتی ہے:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

`metrics.prom` کو اپنے Prometheus sidecar میں feed کریں (مثال کے طور پر Promtail
یا importador por lotes کے ذریعے) تاکہ registradores, administradores y pares de gobernanza a granel
progreso پر alineado رہیں۔ Placa Grafana
`dashboards/grafana/sns_bulk_release.json` وہی paneles de datos میں دکھاتا ہے: por sufijo
recuentos, volumen de pagos, ratios de éxito/fracaso de envíos ۔ Placa `release`
filtro کرتا ہے تاکہ auditores ایک CSV ejecutar پر taladro کر سکیں۔

## 6. Validación y modos de falla- **Canonicalización de etiquetas:** entradas Python IDNA en minúsculas y norma v1
  filtros de caracteres سے normalizar ہوتے ہیں۔ etiquetas no válidas llamadas de red سے پہلے
  fallar rápido ہوتے ہیں۔
- **Valores de seguridad numéricos:** identificadores de sufijo, años de plazo y sugerencias de precios `u16` y `u8`
  límites کے اندر ہونے چاہئیں۔ Campos de pago decimal یا enteros hexadecimales `i64::MAX`
  تک aceptar کرتے ہیں۔
- **Análisis de gobernanza de metadatos:** análisis de JSON en línea براہ راست ہوتا ہے؛ archivo
  referencias ubicación CSV کے resolución relativa ہوتی ہیں۔ Metadatos que no son objetos
  error de validación دیتا ہے۔
- **Controladores:** خالی celdas `--default-controllers` کو honor کرتے ہیں۔ no propietario
  actores کو delegado کرتے وقت listas de controladores explícitos دیں (مثال `soraカタカナ...;soraカタカナ...`)۔

Números de fila contextuales de fallas کے ساتھ informe ہوتے ہیں (مثال
`error: row 12 term_years must be between 1 and 255`). Errores de validación del script
`1` Falta la ruta CSV ہونے پر `2` کے ساتھ salir کرتا ہے۔

## 7. Pruebas y procedencia

- Análisis CSV `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py`, NDJSON
  emisión, cumplimiento de la gobernanza, اور CLI/Torii rutas de envío کو cubierta کرتا ہے۔
- Ayudante Python puro ہے (کوئی اضافی dependencias نہیں) اور جہاں `python3` دستیاب
  ہو وہاں چلتا ہے۔ Historial de confirmaciones CLI کے ساتھ repositorio principal میں pista ہوتی ہے
  تاکہ reproducibilidad ہو۔Ejecuciones de producción کے لئے، manifiesto generado اور Paquete NDJSON کو ticket de registrador کے
ساتھ adjuntar کریں تاکہ mayordomos Torii کو enviar ہونے والے reproducción de cargas útiles exactas
کر سکیں۔
