---
id: bulk-onboarding-toolkit
lang: es
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Fuente canonica
Refleja `docs/source/sns/bulk_onboarding_toolkit.md` para que los operadores externos vean
la misma guia SN-3b sin clonar el repositorio.
:::

# Toolkit de onboarding masivo SNS (SN-3b)

**Referencia del roadmap:** SN-3b "Bulk onboarding tooling"  
**Artefactos:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Los registrars grandes a menudo pre-preparan cientos de registros `.sora` o `.nexus`
con las mismas aprobaciones de gobernanza y rails de settlement. Armar payloads JSON
a mano o volver a ejecutar la CLI no escala, asi que SN-3b entrega un builder
determinista de CSV a Norito que prepara estructuras `RegisterNameRequestV1` para
Torii o la CLI. El helper valida cada fila de antemano, emite tanto un manifiesto
agregado como JSON delimitado por saltos de linea opcional, y puede enviar los
payloads automaticamente mientras registra recibos estructurados para auditorias.

## 1. Esquema CSV

El parser requiere la siguiente fila de encabezado (el orden es flexible):

| Columna | Requerido | Descripcion |
|---------|-----------|-------------|
| `label` | Si | Etiqueta solicitada (se acepta mayus/minus; la herramienta normaliza segun Norm v1 y UTS-46). |
| `suffix_id` | Si | Identificador numerico de sufijo (decimal o `0x` hex). |
| `owner` | Si | AccountId string (domainless encoded literal; canonical Katakana i105 only; no `@<domain>` suffix). |
| `term_years` | Si | Entero `1..=255`. |
| `payment_asset_id` | Si | Activo de settlement (por ejemplo `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | Si | Enteros sin signo que representan unidades nativas del activo. |
| `settlement_tx` | Si | Valor JSON o cadena literal que describe la transaccion de pago o hash. |
| `payment_payer` | Si | AccountId que autorizo el pago. |
| `payment_signature` | Si | JSON o cadena literal con la prueba de firma de steward o tesoreria. |
| `controllers` | Opcional | Lista separada por punto y coma o coma de direcciones de cuenta controller. Por defecto `[owner]` cuando se omite. |
| `metadata` | Opcional | JSON inline o `@path/to/file.json` que provee hints de resolver, registros TXT, etc. Por defecto `{}`. |
| `governance` | Opcional | JSON inline o `@path` apuntando a un `GovernanceHookV1`. `--require-governance` exige esta columna. |

Cualquier columna puede referenciar un archivo externo prefijando el valor de la celda con `@`.
Las rutas se resuelven relativo al archivo CSV.

## 2. Ejecutar el helper

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Opciones clave:

- `--require-governance` rechaza filas sin un hook de gobernanza (util para
  subastas premium o asignaciones reservadas).
- `--default-controllers {owner,none}` decide si las celdas vacias de controllers
  vuelven a la cuenta owner.
- `--controllers-column`, `--metadata-column`, y `--governance-column` permiten
  renombrar columnas opcionales cuando se trabaja con exports upstream.

En caso de exito el script escribe un manifiesto agregado:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "<katakana-i105-account-id>",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"<katakana-i105-account-id>","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"<katakana-i105-account-id>",
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

Si se proporciona `--ndjson`, cada `RegisterNameRequestV1` tambien se escribe como un
documento JSON de una sola linea para que las automatizaciones puedan transmitir
solicitudes directamente a Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. Envios automatizados

### 3.1 Modo Torii REST

Especifique `--submit-torii-url` mas `--submit-token` o `--submit-token-file` para
empujar cada entrada del manifiesto directamente a Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- El helper emite un `POST /v1/sns/names` por solicitud y aborta ante el
  primer error HTTP. Las respuestas se anexan a la ruta del log como registros
  NDJSON.
- `--poll-status` vuelve a consultar `/v1/sns/names/{namespace}/{literal}` despues de
  cada envio (hasta `--poll-attempts`, default 5) para confirmar que el registro
  es visible. Proporcione `--suffix-map` (JSON de `suffix_id` a valores "suffix")
  para que la herramienta derive literales `{label}.{suffix}` al hacer polling.
- Ajustes: `--submit-timeout`, `--poll-attempts`, y `--poll-interval`.

### 3.2 Modo iroha CLI

Para enrutar cada entrada del manifiesto por la CLI, indique la ruta del binario:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Los controllers deben ser entradas `Account` (`controller_type.kind = "Account"`)
  porque la CLI actualmente solo expone controllers basados en cuentas.
- Los blobs de metadata y governance se escriben a archivos temporales por
  solicitud y se pasan a `iroha sns register --metadata-json ... --governance-json ...`.
- El stdout y stderr de la CLI mas los codigos de salida se registran; los codigos
  no cero abortan la ejecucion.

Ambos modos de envio pueden ejecutarse juntos (Torii y CLI) para verificar
despliegues del registrar o ensayar fallbacks.

### 3.3 Recibos de envio

Cuando se proporciona `--submission-log <path>`, el script anexa entradas NDJSON que
capturan:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Las respuestas exitosas de Torii incluyen campos estructurados extraidos de
`NameRecordV1` o `RegisterNameResponseV1` (por ejemplo `record_status`,
`record_pricing_class`, `record_owner`, `record_expires_at_ms`,
`registry_event_version`, `suffix_id`, `label`) para que dashboards y reportes de
gobernanza puedan parsear el log sin inspeccionar texto libre. Adjunte este log a
los tickets del registrar junto con el manifiesto para evidencia reproducible.

## 4. Automatizacion de release del portal

Los trabajos de CI y del portal llaman a `docs/portal/scripts/sns_bulk_release.sh`,
que envuelve el helper y guarda artefactos bajo `artifacts/sns/releases/<timestamp>/`:

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

El script:

1. Construye `registrations.manifest.json`, `registrations.ndjson`, y copia el
   CSV original en el directorio de release.
2. Envia el manifiesto usando Torii y/o la CLI (cuando se configura), escribiendo
   `submissions.log` con los recibos estructurados de arriba.
3. Emite `summary.json` describiendo el release (rutas, URL Torii, ruta CLI,
   timestamp) para que la automatizacion del portal pueda cargar el bundle a
   almacenamiento de artefactos.
4. Produce `metrics.prom` (override via `--metrics`) que contiene contadores
   en formato Prometheus para total de solicitudes, distribucion de sufijos,
   totales de asset y resultados de envio. El JSON de resumen enlaza a este
   archivo.

Los workflows simplemente archivan el directorio de release como un solo artefacto,
que ahora contiene todo lo que la gobernanza necesita para auditoria.

## 5. Telemetria y dashboards

El archivo de metricas generado por `sns_bulk_release.sh` expone las siguientes
series:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

Alimente `metrics.prom` en su sidecar de Prometheus (por ejemplo via Promtail o
un importador batch) para mantener registrars, stewards y pares de gobernanza
alineados sobre el progreso masivo. El tablero Grafana
`dashboards/grafana/sns_bulk_release.json` visualiza los mismos datos con paneles
para conteos por sufijo, volumen de pago y ratios de exito/fallo de envios. El
tablero filtra por `release` para que los auditores puedan entrar en una sola
corrida de CSV.

## 6. Validacion y modos de fallo

- **Normalizacion de label:** las entradas se normalizan con Python IDNA mas
  lowercase y filtros de caracteres Norm v1. Labels invalidos fallan rapido antes
  de cualquier llamada de red.
- **Guardrails numericos:** suffix ids, term years, y pricing hints deben caer
  dentro de limites `u16` y `u8`. Los campos de pago aceptan enteros decimales o
  hex hasta `i64::MAX`.
- **Parsing de metadata o governance:** JSON inline se analiza directo; las
  referencias a archivos se resuelven relativo a la ubicacion del CSV. Metadata
  que no sea objeto produce un error de validacion.
- **Controllers:** celdas en blanco respetan `--default-controllers`. Proporcione
  listas de controller explicitas (por ejemplo `<katakana-i105-account-id>;<katakana-i105-account-id>`) al delegar a
  actores no owner.

Los fallos se reportan con numeros de fila contextuales (por ejemplo
`error: row 12 term_years must be between 1 and 255`). El script sale con codigo
`1` en errores de validacion y `2` cuando falta la ruta del CSV.

## 7. Testing y procedencia

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` cubre parsing CSV,
  emision NDJSON, enforcement de governance y los caminos de envio por CLI o Torii.
- El helper es Python puro (sin dependencias adicionales) y corre en cualquier
  lugar donde `python3` este disponible. El historial de commits se rastrea junto
  a la CLI en el repositorio principal para reproducibilidad.

Para corridas de produccion, adjunte el manifiesto generado y el bundle NDJSON al
ticket del registrar para que los stewards puedan reproducir los payloads exactos
que se enviaron a Torii.
