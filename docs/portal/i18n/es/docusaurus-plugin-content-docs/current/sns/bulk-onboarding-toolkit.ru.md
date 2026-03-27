---
lang: es
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Канонический источник
Esta página aparece `docs/source/sns/bulk_onboarding_toolkit.md`, qué ropas hay
Los operadores deben utilizar las recomendaciones SN-3b antes del registro de limpieza.
:::

# Kit de herramientas de incorporación masiva de SNS (SN-3b)

**Hoja de ruta de Ссылка:** SN-3b "Herramientas de incorporación masiva"  
**Artículos:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Крупные registradores часто заранее подготавливают сотни регистраций `.sora` или
`.nexus` с одинаковыми одобрениями управления и asentamiento rieles. Ручная сборка
Las cargas útiles JSON y la CLI no se pueden instalar en el dispositivo SN-3b
детерминированный CSV-to-Norito constructor, который готовит структуры
`RegisterNameRequestV1` para Torii o CLI. Хелпер заранее валидирует каждую строку,
Utilice un manifiesto agregado y un archivo JSON opcional y pueda implementarlo.
cargas útiles автоматически, записывая структурированные recibos для аудитов.

## 1. Схема CSV

Парсер требует следующую строку заголовка (порядок гибкий):| Colonia | Обязательно | Descripción |
|---------|-------------|----------|
| `label` | Да | Запрошенная метка (doпускается caso mixto; instrumento normalizado según Norm v1 y UTS-46). |
| `suffix_id` | Да | Числовой идентификатор суффикса (десятичный или `0x` hex). |
| `owner` | Да | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | Да | Целое число `1..=255`. |
| `payment_asset_id` | Да | Liquidación activa (nombre `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | Да | Беззнаковые целые, представляющие единицы актива. |
| `settlement_tx` | Да | JSON значение или строка, описывающая платежную транзакцию или hash. |
| `payment_payer` | Да | AccountId, placa de autorización. |
| `payment_signature` | Да | JSON o строка с доказательством подписи Steward o Treasury. |
| `controllers` | Opcional | Controlador de dirección principal, respectivamente `;` o `,`. По умолчанию `[owner]`. |
| `metadata` | Opcional | JSON en línea y `@path/to/file.json` con sugerencias de resolución, mensajes TXT y т. д. Por ejemplo `{}`. |
| `governance` | Opcional | JSON en línea o `@path` a `GovernanceHookV1`. `--require-governance` делает колонку обязательной. |

Una columna de Polonia puede conectarse a un archivo de texto único o incluir `@` en la configuración anterior.
Пути разрешаются относительно файла CSV.

## 2. Запуск хелпера```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Otras opciones:

- `--require-governance` отклоняет строки без gancho de gobernanza (полезно для
  premium аукционов или asignaciones reservadas).
- `--default-controllers {owner,none}` решает, будут ли пустые controlador ячейки
  падать обратно на propietario.
- `--controllers-column`, `--metadata-column` y `--governance-column`.
  переименовать опциональные колонки при работе с upstream exports.

En el siguiente script se muestra un manifiesto agregado:

```json
{
  "schema_version": 1,
  "generated_at": "2026-03-30T06:48:00.123456Z",
  "source_csv": "/abs/path/registrations.csv",
  "requests": [
    {
      "selector": {"version":1,"suffix_id":1,"label":"alpha"},
      "owner": "<i105-account-id>",
      "controllers": [
        {"controller_type":{"kind":"Account"},"account_address":"<i105-account-id>","resolver_template_id":null,"payload":{}}
      ],
      "term_years": 2,
      "pricing_class_hint": null,
      "payment": {
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
        "gross_amount":240,
        "net_amount":240,
        "settlement_tx":"alpha-settlement",
        "payer":"<i105-account-id>",
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

Si utiliza `--ndjson`, el código `RegisterNameRequestV1` también está disponible
El documento JSON original, que se puede automatizar, puede bloquear el contenido desde el primer momento.
Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. Автоматизированные отправки

### 3.1 Paso Torii REST

Coloque `--submit-torii-url` y el disco `--submit-token`, el disco `--submit-token-file`,
чтобы отправлять каждую запись manifest напрямую в Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```

- Ayudante del usuario `POST /v1/sns/names` en zapros y останавливается при
  первой HTTP ошибке. Ответы добавляются в лог как NDJSON записи.
- `--poll-status` повторно запрашивает `/v1/sns/names/{namespace}/{literal}` после
  каждой отправки (до `--poll-attempts`, по умолчанию 5), чтобы подтвердить
  видимость записи. Utilice `--suffix-map` (mapeo JSON `suffix_id` en la base
  "sufijo"), este instrumento es el más importante `{label}.{suffix}` para el sondeo.
- Números: `--submit-timeout`, `--poll-attempts` y `--poll-interval`.

### 3.2 Código iroha CLIPara programar el manifiesto desde la CLI, coloque el binario:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Controladores должны быть entradas `Account` (`controller_type.kind = "Account"`),
  Esta CLI permite acceder a controladores basados en cuentas.
- Metadatos y blobs de gobernanza incluyen archivos adicionales en archivos adjuntos y
  передаются в `iroha sns register --metadata-json ... --governance-json ...`.
- Stdout/stderr y códigos de registro CLI; ненулевые коды прерывают запуск.

Esta opción de configuración puede descargarse todo (Torii y CLI) para aplicaciones permanentes
implementaciones de registradores y sistemas de respaldo repetidos.

### 3.3 Квитанции отправки

En el script `--submission-log <path>` se incluyen los archivos NDJSON:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```

Las versiones anteriores Torii están configuradas según la estructura de `NameRecordV1` o
`RegisterNameResponseV1` (nombre `record_status`, `record_pricing_class`,
`record_owner`, `record_expires_at_ms`, `registry_event_version`, `suffix_id`,
`label`), los tableros de instrumentos y las versiones mejoradas pueden bloquear el registro sin problemas
texto. Прикрепите этот лог к registrador тикетам вместе с manifest для
воспроизводимого доказательства.

## 4. Автоматизация релизов портала

Trabajos de CI y portal вызывают `docs/portal/scripts/sns_bulk_release.sh`, который
оборачивает хелпер y сохраняет артефакты под `artifacts/sns/releases/<timestamp>/`:

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

Escritura:1. Создает `registrations.manifest.json`, `registrations.ndjson` y копирует
   Este archivo CSV está disponible en el directorio.
2. Configure el manifiesto con Torii y/o CLI (por defecto) y cierre
   `submissions.log` со структурированными квитанциями выше.
3. Forme el formulario `summary.json` con la versión de descripción (por ejemplo, URL Torii, por CLI,
   marca de tiempo), el portal automatizado puede descargar el paquete en formato exclusivo
   artefactos.
4. Generar `metrics.prom` (anular desde `--metrics`) con Prometheus-soвместимыми
   счетчиками для общего числа запросов, распределения суффиксов, сумм по активам
   и результатов отправки. Resumen JSON creado en este archivo.

Flujos de trabajo en el directorio de arхивируют релиза как единый артефакт, содержащий
все необходимое для аудита.

## 5. Telemetría y tableros

La métrica de tamaño original `sns_bulk_release.sh` corresponde a la serie de series:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

Coloque el sidecar `metrics.prom` en Prometheus (por ejemplo, Promtail o lote)
importador), чтобы registradores, administradores y pares de gobernanza видели согласованный
прогресс a granel процессов. Tablero Grafana
`dashboards/grafana/sns_bulk_release.json` visualiza este día: количество
по суффиксам, объем платежей и соотношение успешных/неуспешных отправок. tablero
Los filtros de `release`, los auditores pueden utilizar el programa CSV.

## 6. Validación y reservas- **Canonicalización de etiquetas:** входы нормализуются Python IDNA + minúsculas
  фильтрами символов Norma v1. Nevalydnye метки быстро падают до сетевых вызовов.
- **Valores de seguridad numéricos:** identificadores de sufijo, años de plazo y sugerencias de precios должны быть в
  Antes de `u16` y `u8`. Поля платежей принимают десятичные или hex числа до
  `i64::MAX`.
- **Análisis de metadatos/gobernanza:** JSON парсится напрямую en línea; ссылки на файлы
  разрешаются относительно CSV. Los metadatos no están disponibles para las validaciones.
- **Controladores:** пустые ячейки соблюдают `--default-controllers`. Указывайте
  явные списки controladores (nombre `<i105-account-id>;<i105-account-id>`) при делегировании не-owner.

Ошибки сообщаются с контекстными номерами строк (por ejemplo
`error: row 12 term_years must be between 1 and 255`). Script de escritura con código `1`
Después de verificar las validaciones y `2`, o coloque el archivo CSV.

## 7. Pruebas y procesos

- Archivo CSV `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py`,
  вывод NDJSON, gobernanza de cumplimiento y пути отправки через CLI или Torii.
- Ayudante de usuario de Python (sin controladores remotos) y robot
  Aquí está el archivo `python3`. Historia de los enlaces conectados a la CLI en
  основном репозитории для воспроизводимости.

Los programas de software incluyen un manifiesto de usuario y un paquete NDJSON
registrador тикету, чтобы mayordomos могли воспроизвести точные cargas útiles, отправленные
en Torii.