---
lang: he
direction: rtl
source: docs/portal/docs/sns/bulk-onboarding-toolkit.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::שימו לב פואנטה קנוניקה
Refleja `docs/source/sns/bulk_onboarding_toolkit.md` para que los operadores externos vean
la misma guia SN-3b sin clonar el repositorio.
:::

# ערכת כלים ל- onboarding masivo SNS (SN-3b)

**התייחסות למפת הדרכים:** SN-3b "כלי עבודה בכמות גדולה"  
**חפצים:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Los Registrars grandes a menudo הכנה מראש cientos de registros `.sora` או `.nexus`
con las mismas aprobaciones de gobernanza y rails de settlement. Armar מטען JSON
a mano o volver a ejecutar la CLI no escala, asi que SN-3b entrega un Builder
determinista de CSV a Norito que prepara estructuras `RegisterNameRequestV1` para
Torii או לה CLI. El helper valida cada fila de antemano, emite tanto un manifiesto
אגרגאדו כמו JSON delimitado por saltos de linea אופציונלי, y puede enviar los
מטענים אוטומטיים מרישום הרצאות estructurados עבור אודיטוריאס.

## 1. Esquema CSV

El parser requiere la suuiente fila de encabezado (el orden es גמיש):

| Columna | Requerido | תיאור |
|--------|--------|-------------|
| `label` | סי | כללי התנהגות (se acepta mayus/minus; la herramienta normaliza segun Norm v1 y UTS-46). |
| `suffix_id` | סי | מזהה מספרי סופיו (עשרוני או `0x` hex). |
| `owner` | סי | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | סי | Entero `1..=255`. |
| `payment_asset_id` | סי | Activo de Settlement (por ejemplo `xor#sora`). |
| `payment_gross` / `payment_net` | סי | Enteros sin signno que representan unidades nativas del activo. |
| `settlement_tx` | סי | Valor JSON או cadena מילולית que describe la transaccion de pago o hash. |
| `payment_payer` | סי | AccountId que autorizo ​​el pago. |
| `payment_signature` | סי | JSON o cadena literal con la prueba de firma de steward o tesoreria. |
| `controllers` | אופציונלי | Lista separada por punto y coma o coma de direcciones de cuenta controller. Por defecto `[owner]` cuando se omite. |
| `metadata` | אופציונלי | JSON מוטבע ב-`@path/to/file.json` כדי להוכיח רמזים ל-Resolver, registros TXT, וכו'. על ידי `{}`. |
| `governance` | אופציונלי | JSON מוטבע או `@path` אפואנטו ו-`GovernanceHookV1`. `--require-governance` exige esta columna. |

Cualquier columna puede referenciar un archivo externo prefijando el valor de la celda con `@`.
Las rutas se resuelven relativo al archivo CSV.

## 2. Ejecutar el helper

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

אופציות קלאב:

- `--require-governance` rechaza filas sin un hook de gobernanza (util para
  subastas premium או asignaciones reservadas).
- `--default-controllers {owner,none}` החליטו סי לאס celdas vacias de controllers
  הבעלים של vuelven a la cuenta.
- `--controllers-column`, `--metadata-column`, y `--governance-column` אישורים
  renombrar columnas opcionales cuando se trabaja con יצוא במעלה הזרם.

En caso de exito el script escribe un manifiesto agregado:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "i105...",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"i105...","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"xor#sora",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"i105...",
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
```Si se proporciona `--ndjson`, cada `RegisterNameRequestV1` tambien se escribe como un
מסמך JSON de una sola linea para que las automatizaciones puedan transmitir
פניות מכוונות ל-Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v2/sns/registrations
  done
```

## 3. Envios automatizados

### 3.1 Modo Torii REST

ספציפית `--submit-torii-url` mas `--submit-token` או `--submit-token-file` para
empujar cada entrada del manifiesto directamente a Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- El helper emite un `POST /v2/sns/registrations` por solicitud y aborta ante el
  שגיאת primer HTTP. תשובות לרישום
  NDJSON.
- `--poll-status` ראה יועץ `/v2/sns/registrations/{selector}` מבטל את
  cada envio (hasta `--poll-attempts`, ברירת מחדל 5) לאישור הרשמה
  זה גלוי. Proporcione `--suffix-map` (JSON de `suffix_id` a valores "סיומת")
  para que la herramienta derive literales `{label}.{suffix}` al hacer polling.
- התאמה: `--submit-timeout`, `--poll-attempts`, y `--poll-interval`.

### 3.2 מצב CLI

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
- קובצי מטא נתונים וממשל כתובים בארכיון הזמן
  תבקשו מכם `iroha sns register --metadata-json ... --governance-json ...`.
- El stdout y stderr de la CLI mas los codigos de salida se registran; לוס קודיגוס
  no cero abortan la ejecucion.

Ambos modos de envio pueden ejecutarse juntos (Torii y CLI) עבור אימות
despliegues del registrar o ensayar fallbacks.

### 3.3 Recibos de envio

Cuando se proporciona `--submission-log <path>`, el script anexa entradas NDJSON que
capturan:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Las respuestas exitosas de Torii כולל את העזרה של ה-Campos estructurados extraidos de
`NameRecordV1` o `RegisterNameResponseV1` (לפי `record_status`,
`record_pricing_class`, `record_owner`, `record_expires_at_ms`,
`registry_event_version`, `suffix_id`, `label`) עבור לוחות מחוונים y reportes de
gobernanza puedan parsear el log sin inspeccionar texto free. Adjunte este log a
los tickets del registrar Junto con el manifiesto para Evidencia לשחזור.

## 4. אוטומטיזציה לשחרור של הפורטל

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

תסריט אל:1. Construye `registrations.manifest.json`, `registrations.ndjson`, y copia el
   CSV מקורי במדריך לשחרור.
2. Envia el manifiesto usando Torii y/o la CLI (cuando se configura), escribiendo
   `submissions.log` con los recibos estructurados de arriba.
3. Emite `summary.json` מתאר את ההפצה (rutas, URL Torii, ruta CLI,
   חותמת זמן) para que la automatizacion del portal pueda cargar el bundle a
   almacenamiento de artefactos.
4. הפק `metrics.prom` (עקיפה באמצעות `--metrics`) que contiene contadores
   en formato Prometheus עבור סך של שידולים, הפצת סופיות,
   totals de asset y resultados de envio. El JSON de resumen enlaza a este
   ארכיון.

זרימות עבודה פשוטות לארכיון של מדריך לשחרור כמו אמנות סולו,
que ahora contiene todo lo que la gobernanza necesita para auditoria.

## 5. לוחות מחוונים של טלמטריה

El archivo de metricas generado por `sns_bulk_release.sh` expone las suientes
סדרה:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="xor#sora"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

Alimente `metrics.prom` en su sidecar de Prometheus (לדוגמה דרך Promtail o
un importador batch) para mantener רשמים, דיילים y pares de gobernanza
alineados sobre el progreso masivo. El tablero Grafana
`dashboards/grafana/sns_bulk_release.json` הצגת לוחות נתונים של מיסמוס
para conteos por sufijo, volumen de pago y ratios de exito/fallo de envios. אל
tablero filtra por `release` para que los auditores puedan entrar en una sola
corrida de CSV.

## 6. אימות ודפוס

- **Normalizacion de label:** las entradas se normalizan con Python IDNA mas
  אותיות קטנות y filters de caracteres Norm v1. תוויות invalidos fallan rapido antes
  de cualquier llamada de red.
- **מספרי מעקות בטיחות:** מזהי סיומת, שנות טווח, רמזים לתמחור deben caer
  dentro de limites `u16` y `u8`. Los campos de pago aceptan enteros decimales o
  hex hasta `i64::MAX`.
- **ניתוח מטא-נתונים של ממשל:** JSON מוטבע בכיוון הניתוח; לאס
  רפרנסים לארכיון של מערכות יחסים ל-CSV. מטא נתונים
  que no sea objeto produce un error de validacion.
- **בקרים:** celdas en blanco respetan `--default-controllers`. פרופורציון
  רשימה מפורשת של בקר (לפי דוגמה `i105...;i105...`)
  שחקנים אין בעלים.

Los fallos se reportan con numeros de fila contextuales (por ejemplo
`error: row 12 term_years must be between 1 and 255`). מכירת תסריט עם קודיגו
`1` en errores de validacion y `2` cuando falta la ruta del CSV.

## 7. בדיקת פרוצדורה

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` ניתוח קובייה CSV,
  פליטה NDJSON, אכיפה של ממשל y los caminos de envio por CLI o Torii.
- El helper es Python puro (sin dependencias adicionales) y corre en cualquier
  lugar donde `python3` esponible. אל היסטוריון דה מבצע את סרסטרה חונטו
  a la CLI en el repositorio principal para reproducibilidad.Para corridas de produccion, adjunte el manifiesto generado y el bundle NDJSON al
כרטיס רשם עבור לוס דיילים Puedan משחזר לוס מטענים מדויקים
que se enviaron a Torii.