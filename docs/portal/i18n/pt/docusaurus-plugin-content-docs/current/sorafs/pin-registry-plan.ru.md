---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano de registro de pinos
título: Plano de realização do Pin Registry SoraFS
sidebar_label: Plano de registro de pin
descrição: Planeje a realização do SF-4, охватывающий машину состояний registro, фасад Torii, ferramentas e наблюдаемость.
---

:::nota História Canônica
Esta página contém `docs/source/sorafs/pin_registry_plan.md`. Selecione uma cópia sincronizada para que a documentação esteja ativa.
:::

# Plano de realização do Pin Registry SoraFS (SF-4)

SF-4 fornece contrato de registro de pinos e serviços de suporte, interface de usuário
обязательства manifest, применяют политики pin e предоставляют API para Torii,
шлюзов e оркестраторов. Este documento foi aprovado no plano de validação
задачами реализации, охватывая lógica on-chain, сервисы на стороне хоста,
luminárias e operação operacional.

##Oblado

1. **Registro de registro de registro**: записи Norito para manifestos, aliases,
   цепочек преемственности, эпох хранения e метаданных управления.
2. **Contrato de negociação**: determinação da operação CRUD para operação
   pino de цикла (`ReplicationOrder`, `Precommit`, `Completion`, despejo).
3. **Faixa de segurança**: endpoints gRPC/REST, verificação de registro e
   use Torii e SDK, verifique a página e ateste.
4. **Ferramentas e acessórios**: auxiliares CLI, vetores de teste e documentação para
   manifestos de sincronização, pseudônimos e envelopes de governança.
5. **Телеметрия и ops**: метрики, алерты и runbooks для здоровья registry.

## Модель данных

### Основные записи (Norito)

| Estrutura | Descrição | Política |
|----------|----------|------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | Сопоставляет alias -> manifesto CID. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instruções para provedores закрепить manifesto. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Verifique a prova. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Atualização da Política de Privacidade. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Implementation reference: the authoritative manifest lifecycle and finalized
read schemas live in `crates/iroha_data_model/src/sorafs/pin_registry.rs`.
Supporting alias, replication, and policy envelopes live in
`crates/sorafs_manifest/src/pin_registry.rs`. Consensus admission derives and
validates the stored commitments; Torii and operator tooling consume the exact
native finalized record rather than maintaining a second pin-record format.

Status:
- The native `PinManifestRecord` and `PinManifestFinalizedRecordV1` are the V1
  manifest-registry surface used by core, Torii, fixtures, and reference
  validators.
- Rust code generation uses Norito derives; SDK parity follows the normal guard
  lanes whenever the native schema changes.
- Architecture, manifest-pipeline, CLI, OpenAPI, status, and roadmap documents
  describe the shared validation path and endpoint behavior.

## Contract Implementation

| Task | Owner(s) | Notes |
|------|----------|-------|
| Registry storage and smart-contract state. | Core Infra / Smart Contract Team | Implemented in Iroha world state (`pin_manifests`, `manifest_aliases`, `replication_orders`) with deterministic Norito payload hashing and integer-only policy arithmetic. |
| Entry points: `RegisterPinManifest`, `ApprovePinManifest`, `RetirePinManifest`, `BindManifestAlias`, `IssueReplicationOrder`, `CompleteReplicationOrder`, `ExpireReplicationOrder`. | Core Infra | Registration carries the complete canonical manifest, resource-bounds and validates it in consensus, and derives all stored commitments. Core execution also validates aliases, council envelopes, governance permissions, canonical replication payloads, completion, and deadline-bound expiration. |
| State transitions: enforce succession (manifest A -> B), retention epochs, alias uniqueness, and replication status changes. | Governance Council / Core Infra | `ensure_successor_chain` enforces approved, non-retired, acyclic multi-hop lineage; alias uniqueness, retention, and replication issue/complete bookkeeping are covered by unit tests. |
| Governed parameters: load `ManifestPolicyV1` from config/governance state. | Governance Council | Runtime config maps pin-policy constraints into the shared validator. Live policy-change ceremonies are rollout governance evidence, not missing local contract code. |
| Registry telemetry and audit surface. | Observability | Torii exports registry metrics and attested REST snapshots. Additional signed event archives can be layered over those snapshots if governance requires them. |

Coverage:
- Unit tests cover registration, approval, retirement, alias binding, replication
  order issue/complete, permissions, duplicate rejection, and side-effect-free
  failure paths.
- Successor tests cover self references, unknown/pending/retired predecessors,
  cycle closure, and malformed existing predecessor cycles.
- `ci/check_sorafs_fixtures.sh` regenerates chunker, provider-admission, and pin
  registry fixtures and runs the parity checks that keep the canonical schema
  surface stable.

## Service Facade (Torii/SDK Integration)

| Component | Task | Owner(s) |
|-----------|------|----------|
| Torii Service | Ships `/v1/sorafs/pin`, `/v1/sorafs/pin/{digest_hex}`, `/v1/sorafs/aliases`, and `/v1/sorafs/replication`. The manifest-detail route returns exact native `PinManifestFinalizedRecordV1` JSON and accepts only the optional paired expected finalized height/hash precondition; pagination and filters remain on list routes. | Networking TL / Core Infra |
| Finality binding | Listing responses retain their listing attestation. A manifest-detail response carries the native `finalized_cursor` beside the authoritative `PinManifestRecord`; a stale requested cursor fails with HTTP 409. | Core Infra |
| CLI | `iroha app sorafs pin register`, `pin list`, `pin show`, `alias list`, and `replication list` wrap the REST and ISI surfaces for operator audits. | Tooling WG |
| SDK | Rust request builders and the JavaScript, Python, Swift, and C# guard lanes mirror the manifest payload and pin-register validation surface. | SDK Teams |

Operations:
- List endpoints use attested snapshots, deterministic pagination, and the cache
  behavior documented in the alias policy where alias proofs are involved.
- `GET /v1/sorafs/pin/{digest_hex}` returns only `finalized_cursor` and the
  native `manifest`. The retired `limit`, attestation, embedded alias/order
  arrays, counts, and truncation fields are absent; callers use
  `/v1/sorafs/aliases` and `/v1/sorafs/replication` for bounded list queries.
- Mutating operations go through ISI/governance permissions; REST handling keeps
  the same Torii auth and resource-guard model as the surrounding SoraFS APIs.

## Luminárias e CI- Fixações de catálogo: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` хранит подписанные snapshots manifest/alias/order, пересоздаваемые через `cargo run -p iroha_core --example gen_pin_snapshot`.
- Шаг CI: `ci/check_sorafs_fixtures.sh` пересоздает snapshot e падает при diff, удерживая fixtures CI синхронными.
- Интеграционные тесты (`crates/iroha_core/tests/pin_registry.rs`) покрывают caminho feliz плюс отказ при дублировании alias, guardas одобрения/хранения alias, несовпадающие lida com chunker, проверку числа реплик e отказы guardas преемственности (неизвестные/предодобренные/выведенные/самоссылки); sim. Use `register_manifest_rejects_*` para uma configuração detalhada.
- Юнит-тесты теперь покрывают валидацию alias, guards хранения и проверки преемника в `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; обнаружение многошаговой преемственности появится, когда заработает машина состояний.
- Golden JSON para ser usado, usado na configuração.

## Telemetria e instalação

Métricas (Prometheus):
-`torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
-`torii_sorafs_registry_aliases_total`
-`torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
-`torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
-`torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
-`torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- Существующая provedor-телеметрия (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) остается в области para clientes de ponta a ponta.

Logs:
- Структурированный поток событий Norito para auditoria управления (подписанные?).

Alertas:
- Verifique as cópias de segurança, verificando o SLA.
- Истечение срока alias ниже порога.
- Нарушения хранения (manifesto не продлен до истечения).

Dашборды:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` отслеживает totais жизненного цикла manifestos, покрытие alias, насыщение backlog, relação SLA, sobreposições latência vs folga e muito пропущенных заказов для ревью de plantão.

## Runbooks e documentação

- Обновить `docs/source/sorafs/migration_ledger.md`, чтобы включить обновления статуса registro.
- Operação operacional: `docs/source/sorafs/runbooks/pin_registry_ops.md` (opcional) com métricas, alertas, развертыванием, backup e recuperação.
- Руководство по управлению: описать параметры политики, fluxo de trabalho одобрения, обработку споров.
- Страницы справочника API para o endpoint (documentos Docusaurus).

## Зависимости и последовательность

1. Abra o plano de validação (integração do ManifestValidator).
2. Финализировать схему Norito + política padrão.
3. Realize o contrato + serviço, envie-nos o telefone.
4. Luminárias Перегенерировать, suíte de integração запустить.
5. Abra documentos/runbooks e obtenha o roteiro dos pontos para construção.

O ponto de conexão SF-4 está ajustado neste plano de progresso dinâmico.
REST é o momento em que a configuração de endpoints é especificada:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` e `GET /v1/sorafs/replication` publicados ativos
  alias de catálogo e backlog cria replicações com página de consistência e
  estado do filtro.

CLI оборачивает эти вызовы (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`), чтобы операторы могли автоматизировать аудиты registro
Não é uma API aprimorada.