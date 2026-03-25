---
lang: es
direction: ltr
source: docs/portal/docs/sns/bulk-onboarding-toolkit.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::nota Fuente canónica
Espelha `docs/source/sns/bulk_onboarding_toolkit.md` para operadores externos
vejam a mesma orientacao SN-3b sem clonar o repositorio.
:::

# Kit de herramientas de incorporación en masa SNS (SN-3b)

**Referencia de la hoja de ruta:** SN-3b "Herramientas de incorporación masiva"  
**Artefatos:** `scripts/sns_bulk_onboard.py`, `scripts/tests/test_sns_bulk_onboard.py`,
`docs/portal/scripts/sns_bulk_release.sh`

Registradores grandes frecuentemente preparan centenares de registros `.sora` o
`.nexus` com as mesmas aprovacoes de Governanca e Rails de Settlement. Montar
payloads JSON manualmente o reejecutar a CLI nao escalar, entonces SN-3b entrega um
constructor determinístico de CSV para Norito que prepara estructuras
`RegisterNameRequestV1` para Torii o para una CLI. O ayudante valida cada línea
con antecedencia, emite tanto un manifiesto agregado cuanto JSON delimitado por
Quebra de línea opcional y puede enviar las cargas útiles automáticamente en cuanto
registra recibos estructurados para auditorios.

## 1. Esquema CSV

El analizador exige la siguiente línea de cabecalho (a orden y flexivel):| Coluna | Obrigatorio | Descripción |
|--------|-------------|-----------|
| `label` | Sim | Etiqueta solicitada (aceita de caso mixto; a ferramenta normaliza conforme Norma v1 e UTS-46). |
| `suffix_id` | Sim | Identificador numérico de sufijo (decimal o `0x` hexadecimal). |
| `owner` | Sim | AccountId string (domainless encoded literal; canonical I105 only; no `@<domain>` suffix). |
| `term_years` | Sim | Inteiro `1..=255`. |
| `payment_asset_id` | Sim | Activo de liquidación (por ejemplo `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `payment_gross` / `payment_net` | Sim | Inteiros sinal representando unidades nativas do ativo. |
| `settlement_tx` | Sim | Valor JSON o cadena literal que describe una transacción de pago o hash. |
| `payment_payer` | Sim | AccountId que autoriza el pago. |
| `payment_signature` | Sim | JSON o cadena literal afirman ser una prueba de assinatura del administrador o tesouraria. |
| `controllers` | Opcional | Lista separada por ponto e virgula o virgula de enderecos de conta controlador. Padrao `[owner]` cuando omitido. |
| `metadata` | Opcional | JSON en línea o `@path/to/file.json` para obtener sugerencias de resolución, registros TXT, etc. Padrao `{}`. |
| `governance` | Opcional | JSON en línea o `@path` activando para `GovernanceHookV1`. `--require-governance` exige esta columna. |Cualquier columna puede referenciar un archivo externo prefijando el valor de la celda con `@`.
Los caminos son resueltos en relación al archivo CSV.

## 2. Ejecutar o ayudar

```bash
python3 scripts/sns_bulk_onboard.py registrations.csv \
  --output artifacts/sns_bulk_manifest.json \
  --ndjson artifacts/sns_bulk_requests.ndjson
```

Principales operaciones:

- `--require-governance` rejeita linhas sin um gancho de gobierno (util para
  leiloes premium o atribuicoes reservadas).
- `--default-controllers {owner,none}` decide se celulas de controladores vazias
  voltam para un dueño de cuenta.
- `--controllers-column`, `--metadata-column`, e `--governance-column` permiten
  renomear colunas opcionais ao trabalhar com exportaciones upstream.

En caso de éxito del script se grava un manifiesto agregado:

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
        "asset_id":"61CtjvNd9T3THAR65GsMVHr82Bjc",
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
```

Se `--ndjson` for fornecido, cada `RegisterNameRequestV1` tambem e escrito como
Un documento JSON de línea única para que automáticamente puedan transmitir solicitudes.
directamente ao Torii:

```bash
jq -c '.requests[]' artifacts/sns_bulk_manifest.json |
  while read -r payload; do
    curl -H "Authorization: Bearer $TOKEN" \
         -H "Content-Type: application/json" \
         -d "$payload" \
         https://torii.sora.net/v1/sns/names
  done
```

## 3. Sumisiones automatizadas

### 3.1 Modo Torii DESCANSO

Especifique `--submit-torii-url` o `--submit-token` o `--submit-token-file`.
para enviar cada entrada del manifiesto directamente a Torii:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-torii-url https://torii.sora.net \
  --submit-token-file ~/.config/sora/tokens/registrar.token \
  --poll-status \
  --suffix-map configs/sns_suffix_map.json \
  --submission-log artifacts/sns_bulk_submit.log
```- O helper emite un `POST /v1/sns/names` por solicitud y aborta no primero
  error HTTP. As respostas sao anexadas ao log como registros NDJSON.
- `--poll-status` reconsulta `/v1/sns/names/{namespace}/{literal}` apos cada envío
  (ate `--poll-attempts`, predeterminado 5) para confirmar que o registro esta visible.
  Forneca `--suffix-map` (JSON de `suffix_id` para valores "suffix") para que a
  La herramienta deriva la literatura `{label}.{suffix}` para el sondeo.
- Ajustes: `--submit-timeout`, `--poll-attempts`, e `--poll-interval`.

### 3.2 Modo iroha CLI

Para rotar cada entrada del manifiesto pela CLI, forneca o caminho do binario:

```bash
python3 scripts/sns_bulk_onboard.py --manifest artifacts/sns_bulk_manifest.json \
  --submit-cli-path ./target/release/iroha \
  --submit-cli-config configs/registrar.toml \
  --submit-cli-extra-arg --chain-id=devnet \
  --submission-log artifacts/sns_bulk_submit.log
```

- Los controladores deben ser entradas `Account` (`controller_type.kind = "Account"`)
  porque una CLI actualmente expone controladores basados en contactos.
- Blobs de metadatos y gobierno gravados en archivos temporales por solicitud
  e encaminados para `iroha sns register --metadata-json ... --governance-json ...`.
- Stdout y stderr da CLI, mais os codigos de saya, sao registrados; codigos nao
  zero abortam a execucao.

Ambos os modos de submissao podem rodar juntos (Torii e CLI) para comprobar
Las implementaciones registran o prueban respaldos.

### 3.3 Recibos de sumisión

Cuando `--submission-log <path>` y fornecido, o script anexa entradas NDJSON que
capturam:

```json
{"timestamp":"2026-03-30T07:22:04.123Z","mode":"torii","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"..."}
{"timestamp":"2026-03-30T07:22:05.456Z","mode":"torii-poll","index":12,"selector":"1:alpha","status":200,"success":true,"detail":"{...}","attempt":2}
{"timestamp":"2026-03-30T07:22:06.789Z","mode":"cli","index":12,"selector":"1:alpha","status":0,"success":true,"detail":"Registration accepted"}
```Respostas Torii bem-sucedidas incluyen campos estruturados extraidos de
`NameRecordV1` o `RegisterNameResponseV1` (por ejemplo, `record_status`,
`record_pricing_class`, `record_owner`, `record_expires_at_ms`,
`registry_event_version`, `suffix_id`, `label`) para paneles y relatorios
de Governanca possam parsear o log sem inspeccionar texto livre. Anexe este registro
as tickets de registrador junto com o manifesto para evidencia reproduzivel.

## 4. Automacao de lanzamiento del portal

Empleos de CI y del portal chamam `docs/portal/scripts/sns_bulk_release.sh`, que
encapsula o helper e armazena artefatos sollozo
`artifacts/sns/releases/<timestamp>/`:

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

Oh guión:

1. Construya `registrations.manifest.json`, `registrations.ndjson`, y copie o CSV
   original para el directorio de liberación.
2. Submete el manifiesto usando Torii y/o una CLI (cuando esté configurado), gravando
   `submissions.log` com os recibos estruturados acima.
3. Emite `summary.json` descrevendo o release (caminhos, URL do Torii, caminho da
   CLI, marca de tiempo) para que un portal automático pueda enviar el paquete para o
   almacenamiento de artefactos.
4. Produz `metrics.prom` (anular vía `--metrics`) contando contadores comparativos
   com Prometheus para el total de solicitudes, distribución de sufijos, total de activos
   y resultados de envío. O JSON de resumen aponta para este archivo.Los flujos de trabajo simplemente archivam o diretorio de liberación como un único artefato,
que ahora contem todo o que un gobierno precisa para auditorios.

## 5. Telemetría y paneles de control

El archivo de métricas generado por `sns_bulk_release.sh` expone las siguientes series:

```
# HELP sns_bulk_release_requests_total Number of registration requests per release and suffix.
# TYPE sns_bulk_release_requests_total gauge
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="all"} 120
sns_bulk_release_requests_total{release="2026q2-beta",suffix_id="1"} 118
sns_bulk_release_payment_gross_units{release="2026q2-beta",asset_id="61CtjvNd9T3THAR65GsMVHr82Bjc"} 28800
sns_bulk_release_submission_events_total{release="2026q2-beta",mode="torii",success="true"} 118
```

Alimente `metrics.prom` no tiene sidecar de Prometheus (por ejemplo a través de Promtail o
um importador lote) para manter registradores, mayordomos y pares de gobierno
alinhados sobre o Progresso em Massa. El cuadro Grafana
`dashboards/grafana/sns_bulk_release.json` visualización de los datos con dolor
para contagens por sufixo, volume de pago e ratios de sucesso/falha de
sumisos. El cuadro filtrado por `release` para que los auditores puedan enfocarse en una
única ejecución de CSV.

## 6. Validacao e modos de falha- **Canonización de etiquetas:** entradas normalizadas con Python IDNA más
  minúsculas y filtros de caracteres Norm v1. Etiquetas invalidas falham rapido
  antes de qualquer chamada de rede.
- **Guardrails numéricos:** identificadores de sufijo, años de plazo y sugerencias de precios devem ficar
  dentro de los límites `u16` e `u8`. Campos de pago aceitam enteros décimais
  Te comiste el maleficio `i64::MAX`.
- **Análisis de metadatos o gobernanza:** JSON en línea y analizado directamente;
  referencias a arquivos sao resolvidas relativas a localizacao do CSV. Metadatos
  nao objeto produz um error de validación.
- **Controladores:** celulas em branco respeitam `--default-controllers`. Forneca
  listas explícitas (por ejemplo `i105...;i105...`) ao delegar para atores nao propietario.

Falhas sao reportadas com numeros de linha contextuais (por ejemplo
`error: row 12 term_years must be between 1 and 255`). O script sai com código
`1` em errores de validación e `2` cuando el camino CSV está ausente.

## 7. Testículos y procedencia

- `python3 -m pytest scripts/tests/test_sns_bulk_onboard.py` cobre analizando CSV,
  emissao NDJSON, aplicación de la gobernanza y caminos de presentación pela CLI o Torii.
- O helper e Python puro (sin dependencias adicionales) e roda em qualquer lugar
  onde `python3` estiver disponivel. El historico de commits e rastreado junto
  a CLI no repositorio principal para reproducibilidad.Para las ejecuciones de producción, anexo o manifiesto elaborado y paquete NDJSON ao ticket do
registrador para que stewards puedan reproducir las cargas útiles exatos que foram
submetidos ao Torii.