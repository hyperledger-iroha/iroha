---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-multisource.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# פרסומות של רב-אורג'ים וסדר היום

דף קורות חיים אפרטיקאאו קאנוניקה
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
השתמש ב-esse documento para schemas Norito מילה במילה יומני שינויים; פורטל עותק דו
mantem a orientacao para operadores, notas de SDK e referencias de telemetria perto do restante
dos runbooks SoraFS.

## Adicoes ao esquema Norito

### יכולת טווח (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` - טווח טווח גדול (בתים) פור requisicao, `>= 1`.
- `min_granularity` - resolucao de seek, `1 <= valor <= max_chunk_span`.
- `supports_sparse_offsets` - היתר קיזוז nao contiguos em uma requisicao.
- `requires_alignment` - quando true, offsets devem alinhar com `min_granularity`.
- `supports_merkle_proof` - אינדיקציה תומכת ב-PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` קידוד אפליקם canonico
para que payloads de gossip permanecam deterministas.

### `StreamBudgetV1`
- קמפוס: `max_in_flight`, `max_bytes_per_sec`, `burst_bytes` אופציונלי.
- Regras de validacao (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes`, quando presente, deve ser `> 0` ו `<= max_bytes_per_sec`.

### `TransportHintV1`
- קמפוס: `protocol: TransportProtocol`, `priority: u8` (janela 0-15 aplicada por
  `TransportHintV1::validate`).
- פרוטוקולים: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Entradas duplicadas de protocolo por provedor sao rejeitadas.

### Adicoes a `ProviderAdvertBodyV1`
- `stream_budget` אופציונלי: `Option<StreamBudgetV1>`.
- `transport_hints` אופציונלי: `Option<Vec<TransportHintV1>>`.
- Ambos os campos agora passam por `ProviderAdmissionProposalV1`, envelopes de governanca,
  מתקנים של CLI e JSON de telemetria.

## Validacao e vinculacao com governanca

`ProviderAdvertBodyV1::validate` e `ProviderAdmissionProposalV1::validate`
rejeitam metadata malformada:

- יכולות טווח מפתחות פענוח ומגבלות טווח/גרגירים.
- זרם תקציבים / רמזים לתחבורה exigem um TLV `CapabilityType::ChunkRangeFetch`
  correspondente e list de hints nao vazia.
- Protocolos de transporte duplicados e prioridades invalidas geram erros de validacao
  אנטס דה פרסומות שרכלו.
- מעטפות קבלה להשוואת הצעות/פרסומות ל-metadata de range via
  `compare_core_fields` para que payloads de gossip divergentes sejam rejeitados cedo.

A cobertura de regressao vive em
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## מכשירים אלקטרוניים

- עומסי פרסומות המוכיחים את הפיתוח כוללים מטא נתונים `range_capability`,
  `stream_budget` e `transport_hints`. תקף דרך respostas de `/v1/sorafs/providers`
  e אביזרי קבלה; קורות חיים בפיתוח JSON כוללים יכולת ניתוח, או זרם
  budget e arrays de hints para ingestao de telemetria.
- `cargo xtask sorafs-admission-fixtures` תקציבי זרם מוסטרה ותחבורה רמזים dentro
  de seus artefatos JSON עבור לוחות מחוונים נלווים לתכונה אדוקאו.
- מתקנים יפחתי `fixtures/sorafs_manifest/provider_admission/` אגורה כולל:
  - מפרסמת קנוניקוס רב-אורג'מים,
  - `multi_fetch_plan.json` para que suites de SDK reproduzam um plano de fetch
    דטרמיניסטי מרובה עמיתים.

## Integracao com מתזמר e Torii- Torii `/v1/sorafs/providers` retorna metadata de range parseada junto com
  `stream_budget` e `transport_hints`. אביזוס דירוג לאחור disparam quando
  מוכיחים להשמיט מטא נתונים חדשניים, ונקודות קצה של טווח לעשות אפליקציית שער כמו
  mesmas restricoes para clientes diretos.
- O orchestrator multi-origem (`sorafs_car::multi_fetch`) agora aplica limites de
  טווח, ניהול יכולות ותקציבי זרם כגון אtribuir trabalho. יחידה
  בוחן את ה-cobrem cenarios de chunk muito grande, חיפוש דליל ומצערת.
- `sorafs_car::multi_fetch` שדר ירידה בדרגה (falhas de alinhamento,
  requisicoes throttled) para que operadores rastreiem por que provedores especificos
  foram ignorados durante o planejamento.

## Referencia de telemetria

מכשירי טווח להביא את Torii מזון ללוח המחוונים Grafana
**SoraFS תצפית אחזור** (`dashboards/grafana/sorafs_fetch_observability.json`) ה
כמו regras de alerta associadas (`dashboards/alerts/sorafs_fetch_rules.yml`).

| מטריקה | טיפו | תוויות | תיאור |
|--------|-------|--------|----------|
| `torii_sorafs_provider_range_capability_total` | מד | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | הוכחות que anunciam תכונות של יכולת טווח. |
| `torii_sorafs_range_fetch_throttle_events_total` | מונה | `reason` (`quota`, `concurrency`, `byte_rate`) | טנטטיביות של טווח להביא אגרופים מצערים לפוליטיקה. |
| `torii_sorafs_range_fetch_concurrency_current` | מד | - | זרמים אטיוווס protegidos consumindo או חלוקת תקציבים של קונקורנסיה. |

דוגמאות של PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

השתמש ב-o contador de throttling para confirmar aplicacao de quotas antes de ativar
ברירת המחדל של aos do orchestrator multi-origem e alerte quando a concorrencia se aproximar
dos maximos do stream budget da sua frota.