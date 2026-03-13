---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-plan
כותרת: Plano de implementacao do Pin Registry do SoraFS
sidebar_label: Plano do Pin Registry
תיאור: Plano de implementacao SF-4 cobrindo a maquina de estados do registry, a fachada Torii, tooling e observabilidade.
---

:::שים לב Fonte canonica
Esta pagina reflete `docs/source/sorafs/pin_registry_plan.md`. Mantenha ambas as copias sincronizadas enquanto a documentacao herdada permanecer ativa.
:::

# Plano de implementacao do Pin Registry do SoraFS (SF-4)

O SF-4 entrega o contrato do Pin Registry e os servicos de apoio que armazenam
פשרה של מניפסט, Impoem Politicas de Pin e Expoem APIs para Torii,
שערים e orquestradores. Este documento amplia o plano de validacao com
tarefas de implementacao concretas, cobrindo a logica on-chain, os servicos do
מארח, אביזרי OS ו-OS Requisitos Operacionais.

## אסקופו

1. **Maquina de estados do registry**: registros definidos por Norito para manifests,
   כינויים, קדנציות המשך, epocas de retencao e metadados de governanca.
2. **Implementacao do contrato**: operacoes CRUD deterministicas para o ciclo de vida
   פיני דוס (`ReplicationOrder`, `Precommit`, `Completion`, פינוי).
3. **Fachada de servico**: נקודות קצה gRPC/REST sustentados pelo registry que Torii
   e OS SDKs consomem, incluindo paginacao e atestacao.
4. **מכשירים אלקטרוניים**: עוזרים של CLI, וטריות ותעודות מסמכים למטרות
   מניפסטים, כינויים e envelopes de governanca sincronizados.
5. **Telemetria e Ops**: מדדים, התראות e runbooks para a saude do registry.

## Modelo de Dados

### רישום מרכזי (Norito)

| מבנה | תיאור | קמפוס |
|--------|--------|--------|
| `PinRecordV1` | Entrada canonica de manifest. | I18NIS `governance_envelope_hash`. |
| `AliasBindingV1` | Mapeia alias -> CID de manifest. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instrucao para providers fixarem o manifest. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | ספק Confirmacao do. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | תמונת מצב של פוליטיקה דה גוברננקה. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Referencia de implementacao: veja `crates/sorafs_manifest/src/pin_registry.rs` para os
esquemas Norito em Rust e os helpers de validacao que sustentam esses registros. א
validacao espelha o tooling de manifest (חיפוש לעשות רישום chunker, שער מדיניות סיכה)
para que o contrato, as fachadas Torii e a CLI compartilhem invariantes identicas.

Tarefas:
- Finalizar os esquemas Norito em `crates/sorafs_manifest/src/pin_registry.rs`.
- Gerar codigo (Rust + outros SDKs) usando מאקרו Norito.
- Atualizar a documentacao (`sorafs_architecture_rfc.md`) quando os esquemas estiverem prontos.## Implementacao do contrato

| טארפה | Responsavel(is) | Notas |
|--------|----------------|-------|
| יישום פעולות רישום (מזחלת/sqlite/מחוץ לשרשרת) או חוזה חכם. | Core Infra / צוות חוזה חכם | Fornecer hashing deterministico, evitar ponto flutuante. |
| נקודות כניסה: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | אינפרא ליבה | Aproveitar `ManifestValidator` do plano de validacao. O binding de alias agora fli via `RegisterPinManifest` (DTO do Torii) enquanto `bind_alias` dedicado segue planejado para atualizacoes successivas. |
| Transicoes de estado: impor sucessao (מניפסט A -> B), epocas de retencao, unicide de alias. | מועצת ממשל / Infra Core | Unicidade de alias, limites de retencao e checks de aprovacao/retirada de predecessores vivem em `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; a deteccao de sucessao multi-hop e o הנהלת חשבונות de replicacao permanecem em aberto. |
| Parametros governados: carregar `ManifestPolicyV1` de config/estado de governanca; permitir atualizacoes via eventos de governanca. | מועצת ממשל | Fornecer CLI para atualizacoes de politica. |
| Emisso de eventos: emitir eventos Norito para telemetria (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | צפייה | Definir esquema de eventos + רישום. |

אשכים:
- Testes unitarios para cada נקודת כניסה (positivo + rejeicao).
- Testes de propriedades para a cadeia de sucessao (sem ciclos, epocas monotonicamente crescentes).
- Fuzz de validacao gerando manifests aleatorios (limitados).

## Fachada de servico (Integracao Torii/SDK)

| רכיב | טארפה | Responsavel(is) |
|------------|--------|----------------|
| Servico Torii | Expor `/v2/sorafs/pin` (שלח), `/v2/sorafs/pin/{cid}` (חיפוש), `/v2/sorafs/aliases` (רשימה/כריכה), `/v2/sorafs/replication` (הזמנות/קבלות). Fornecer paginacao + פילטרגם. | Networking TL / Core Infra |
| אטסטקאו | כולל אפשרויות/hash לעשות רישום ותשובות; Adicionar estrutura de atestacao Norito SDK consumida pelos. | אינפרא ליבה |
| CLI | Estender `sorafs_manifest_stub` ou um novo CLI `sorafs_pin` com `pin submit`, `alias bind`, `order issue`, `registry export`. | Tooling WG |
| SDK | Gerar bindings de cliente (Rust/Go/TS) a partir do esquema Norito; אדיקיוניר אשכים דה אינטגראקאו. | צוותי SDK |

אופרות:
- הוספת מטמון/ETag לנקודות קצה GET.
- הגבלת שיעור Fornecer / אישור עקבי com כפי שהפוליטיקה עושה Torii.

## גופי CI- מדריך מתקנים: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` ארמזנה צילומי מצב assinados de manifest/alias/order regenerados por `cargo run -p iroha_core --example gen_pin_snapshot`.
- Etapa de CI: `ci/check_sorafs_fixtures.sh` regenera o תמונת מצב ו-falha se hover diffs, mantendo os fixtures de CI alinhados.
- Testes de integracao (`crates/iroha_core/tests/pin_registry.rs`) exercitam o fluxo feliz mais a rejeicao de alias duplicado, guards de aprovacao/retencao de alias, handles de chunker incompativeis, validacao de contagem de suurlicas de guard eas desconhecidos/preaprovados/retirados/autorreferencias); veja casos `register_manifest_rejects_*` לפרטי קוברטורה.
- Testes unitarios agora cobrem validacao de alias, guards de retencao e checks de sucessor em `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; a deteccao de sucessao multi-hop quando a maquina de estados chegar.
- JSON golden para eventos usados ​​pelos pipelines de observabilidade.

## Telemetria e Observabilidade

מדדים (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- קיימת קיימת טלמטריה של ספקים (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) מתמשכת עם לוחות מחוונים מקצה לקצה.

יומנים:
- Stream de eventos Norito estruturados para auditorias de governanca (אסינאדוס?).

התראות:
- Ordens de replicacao pendentes excedendo o SLA.
- Expiracao de alias abaixo do limiar.
- Violacoes de retencao (מניפסט נאו renovado antes de expirar).

לוחות מחוונים:
- O JSON do Grafana `docs/source/grafana_sorafs_pin_registry.json` rastreia totais do ciclo de vida dos manifests, cobertura de alias, saturacao do backlog, razao de SLA, overlays de latencia vs slack e taxas de ordenalls perdidas on- revis.

## ספרי הפעלה ומסמכים

- Atualizar `docs/source/sorafs/migration_ledger.md` כולל את הסטטוס של רישום.
- מפעיל פעולות: `docs/source/sorafs/runbooks/pin_registry_ops.md` (יא מפרסמים) מדדי קוברינדו, התראות, פריסה, גיבוי והחלמה.
- Guia de governanca: מסירת פרמטרים פוליטיים, זרימת עבודה של אפרובאקאו, טראטמנטו דה מחלוקת.
- Paginas de referencia de API para cada end point (docs Docusaurus).

## Dependencias e sequenciamento

1. Completar tarefas do plano de validacao (integracao do ManifestValidator).
2. Finalizar esquema Norito + ברירות מחדל של פוליטיקה.
3. קונטרטו מיושם + סרוויקו, קונקטר טלמטריה.
4. אביזרי Regenerar, Rodar Suites de integracao.
5. Atualizar docs/runbooks e marcar itens do מפת דרכים como completos.

רשימת בדיקה עבור SF-4 deve referenciar este plano quando howver progresso.
A Fachada REST agora entrega נקודות קצה de listgem com atestacao:

- `GET /v2/sorafs/pin` e `GET /v2/sorafs/pin/{digest}` retornam manifests com
  bindings de alias, ordens de replicacao e um objeto de atestacao derivado do
  hash do ultimo bloco.
- `GET /v2/sorafs/aliases` e `GET /v2/sorafs/replication` תערוכת או קטלוג
  כינוי ativo e o backlog de ordens de replicacao com paginacao consistente e
  filtros de status.A CLI encapsula essas chamadas (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) para que operadores possam automatizar auditorias do
Registry Sem tocar APIs de baixo nivel.