---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/node-client-protocol.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# פרוטוקול דה לא <-> cliente da SoraFS

Este guia resume a definicao canonica do protocolo em
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
השתמש ב-especificacao upstream para layouts Norito em nivel de byte e changelogs;
a copia do portal mantem os destaques operacionais perto do restante dos runbooks
SoraFS.

## פרסומות מוכיחות

Provedores SoraFS הפצת מטענים `ProviderAdvertV1` (veja
`crates/sorafs_manifest::provider_advert`) assinados pelo operator governado. Os
פרסומות fixam os metadados de descoberta e os מעקות בטיחות que o orquestrador
זמן ריצה של aplica em multi-source.

- **ויג'נסיה** - `issued_at < expires_at <= issued_at + 86,400 s`. פרובדורס
  devem renovar a cada 12 horas.
- **TLVs de capacidade** - רשימה TLV anuncia recursos de transporte (Torii,
  QUIC+Noise, ממסר SoraNet, extensoes de fornecedor). Codigos desconhecidos
  podem ser ignorados quando `allow_unknown_capabilities = true`, seguindo a
  orientacao GREASE.
- **רמזים ל-QoS** - tier de `availability` (חם/חם/קר), לטציה מקסימה דה
  recuperacao, limite de concorrencia e budget de stream אופציונלי. QoS מפתח
  alinhar com a telemetria observada e e auditada na admissao.
- **נקודות קצה ונושאי מפגש** - כתובות URL de servico concretas com metadados
  TLS/ALPN mais OS topics de descoberta aos quais os clientes devem se inscrever
  ao construir guard סטים.
- **Politica de diversidade de caminho** - `min_guard_weight`, caps de fan-out de
  AS/pool e `provider_failure_threshold` טורnam possiveis מביאה דטרמיניסטים
  ריבוי עמיתים.
- **זיהויים של פרפיל** - מוכיחים את הפיתוח או מטפל בקנוניקו (לדוגמה.
  `sorafs.sf1@1.0.0`); `profile_aliases` אופציונאי אגודם לקוחות אנטיגוס למגר.

Regras de validacao Rejeitam הימור אפס, רשימות וזיהיות של יכולות/נקודות קצה/נושאים,
vigencias fora de ordem ou targets de QoS ausentes. השוואת מעטפות כניסה
os corpos do advert e da proposta (`compare_core_fields`) antes de disseminar
atualizacoes.

### הרחבות של טווח אחזור

טווח הבדיקה כולל את המפרטים הבאים:

| קמפו | פרופוזיטו |
|-------|--------|
| `CapabilityType::ChunkRangeFetch` | Declara `max_chunk_span`, `min_granularity` e flags de alinhamento/prova. |
| `StreamBudgetV1` | Envelope optional de concorrencia/תפוקה (`max_in_flight`, `max_bytes_per_sec`, `burst` אופציונלי). דרוש טווח קבלה. |
| `TransportHintV1` | Preferencias de transporte ordenadas (לדוגמה `torii_http_range`, `quic_stream`, `soranet_relay`). Prioridades sao `0-15` e duplicados sao rejeitados. |

תומך כלי עבודה:

- צינורות פרסום של ספק מפתחים טווח תקינות, תקציב זרם ה
  רמזים לתחבורה אנטה דה אמיטיר מטענים קובעים עבור אודיטוריאס.
- `cargo xtask sorafs-admission-fixtures` agrupa פרסומות קנוניקוס רב מקורות
  גופי שדרוג לאחור של junto com em `fixtures/sorafs_manifest/provider_admission/`.
- טווח פרסומות שממנו `stream_budget` או `transport_hints`
  מעמיסים pelos CLI/SDK אנטים לעשות סדר היום, מנטנדו או לרתום ריבוי מקורות
  alinhado com as expectativas de admissao do Torii.

## נקודות קצה של טווח עושים שערGateways aceitam requisicoes HTTP deterministas que espelham os metadados do
פרסומת.

### `GET /v1/sorafs/storage/car/{manifest_id}`

| Requisito | פרטים |
|----------------|--------|
| **כותרות** | `Range` (Janela unica alinhada aos offsets de chunk), `dag-scope: block`, `X-SoraFS-Chunker`, `X-SoraFS-Nonce` אופציונלי ו `X-SoraFS-Stream-Token` obrigatorio base64. |
| **Respostas** | `206` com `Content-Type: application/vnd.ipld.car`, `Content-Range` descrevendo a janela servida, metadados `X-Sora-Chunk-Range` e headers de chunker/token ecoados. |
| **פלחס** | `416` עבור טווחי desalinhados, `401` עבור אסימונים ausentes/invalidos, `429` quando budgets de stream/bytes sao excedidos. |

### `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

Fetch de chunk unico com os mesmos headers mais o digest determinista do chunk.
השתמש בניסיון חוזר או הורדות פורסמות quando slices de CAR דרושות.

## זרימת עבודה לעשות orquestrador ריבוי מקורות

Quando או להביא ריבוי מקורות SF-6 esta habilitado (CLI Rust דרך `sorafs_fetch`,
ערכות SDK דרך `sorafs_orchestrator`):

1. **Coletar entradas** - decodificar o plano de chunks do manifest, puxar os
   פרסומות חדשות, אופציונליות, מתאים לצילום מצב טלמטריה
   (`--telemetry-json` או `TelemetrySnapshot`).
2. **קונסטרויר או לוח תוצאות** - `Orchestrator::build_scoreboard` avalia a
   elegibilidade e registra razoes de rejeicao; `sorafs_fetch --scoreboard-out`
   להתמיד ב-JSON.
3. **נתחי סדר יום** - `fetch_with_scoreboard` (ou `--plan`) reforca restricoes
   de range, budgets de stream, caps de rery/peer (`--retry-budget`, `--max-peers`)
   e emite um stream token com escopo de manifest para cada requisicao.
4. **אימות recibos** - כפי שנאמר כולל `chunk_receipts` e `provider_reports`;
   sumarios do CLI persistem `provider_reports`, `chunk_receipts` e
   `ineligible_providers` חבילות ראיות עזר.

Erros comuns apresentados a operators/SDKs:

| שגיאה | תיאור |
|------|--------|
| `no providers were supplied` | Nenhuma entrada elegivel apos o filtro. |
| `no compatible providers available for chunk {index}` | חוסר התאמה לטווח או תקציב עבור נתח ספציפי. |
| `retry budget exhausted after {attempts}` | Aumente `--retry-budget` ou remova peers com falha. |
| `no healthy providers remaining` | Todos os provedores foram desabilitados apos falhas repetidas. |
| `streaming observer failed` | הו סופר CAR במורד הזרם אבורטו. |
| `orchestrator invariant violated` | צילום מניפסט, לוח תוצאות, תמונת מצב של טלמטריה ו-CLI JSON לטריאג'. |

## טלמטריה וראיות

- Metricas emitidas pelo orquestrador:  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (tagueadas por manifest/region/ספק). הגדרה `telemetry_region` עם תצורה
  או דרך flags de CLI עבור לוחות מחוונים שותפים פורטה.
- סיכומים ללא CLI/SDK כולל לוח תוצאות JSON מתמיד, קבלות חתיכות
  דוחות ספקים שפיתחו חבילות פרסום עבור שערים SF-6/SF-7.
- מטפלי שערים מציגים את `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  para que לוחות מחוונים SRE correlacionem decisoes do orquestrador com o comportamento
  לעשות servidor.

## Helpers de CLI e REST- `iroha app sorafs pin list|show`, `alias list` ו-`replication list` envolvem OS
  נקודות קצה REST ל-Pin-Registry e Imprimem Norito JSON bruto com blocos de
  אישור על הוכחות דה אודיטוריה.
- `iroha app sorafs storage pin` e `torii /v1/sorafs/pin/register` aceitam manifests
  Norito או JSON com alias הוכחות ואופציות של יורשים; הוכחה למלפורמדוס
  geram `400`, הוכחות מעופשות retornam `503` com `Warning: 110`, e proofs expirados
  retornam `412`.
- נקודות קצה REST (`/v1/sorafs/pin`, `/v1/sorafs/aliases`, `/v1/sorafs/replication`)
  כולל אישורים עבור לקוחות אימות קונטרה אוס
  Ultimos headers de bloco antes de agir.

## אסמכתאות

- מפרט קנוניקה:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- טיפוס Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- Helpers de CLI: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- ארגז לעשות orquestrador: `crates/sorafs_orchestrator`
- חבילת לוחות מחוונים: `dashboards/grafana/sorafs_fetch_observability.json`