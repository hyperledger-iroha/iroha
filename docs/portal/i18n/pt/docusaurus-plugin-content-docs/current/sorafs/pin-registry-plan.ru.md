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
| `PinRecordV1` | Manifesto Каноническая запись. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | Сопоставляет alias -> manifesto CID. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instruções para provedores закрепить manifesto. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Verifique a prova. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Atualização da Política de Privacidade. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Ссылка на реализацию: см. `crates/sorafs_manifest/src/pin_registry.rs` para o quadro Norito
em Rust e ajudantes provém, которые лежат в основе этих записей. Validação
повторяет ferramentas de manifesto (registro de chunker de pesquisa, controle de política de pin), чтобы
O contrato, as especificações Torii e CLI identificam os ingredientes ativos.

O que você precisa:
- Coloque os números Norito em `crates/sorafs_manifest/src/pin_registry.rs`.
- Сгенерировать код (Rust + другие SDK) com o macro Norito.
- Обновить документацию (`sorafs_architecture_rfc.md`) após o esquema.

## Realização do contrato| Bem | Exibição | Nomeação |
|--------|---------------|-----------|
| Registro de segurança real (sled/sqlite/off-chain) ou módulo smart-контракта. | Equipe Core de Infra/Contrato Inteligente | Definimos hashing e definimos ponto flutuante. |
| Pontos de entrada: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Infra principal | Use `ManifestValidator` para validar o plano. Биндинг alias теперь проходит через `RegisterPinManifest` (DTO Torii), тогда как выделенный `bind_alias` остается в plano para uma possível implementação. |
| Переходы состояния: обеспечивать преемственность (manifesto A -> B), эпохи хранения, уникальность alias. | Conselho de Governança / Core Infra | Уникальность alias, лимиты хранения и проверки одобрения/вывода предшественников живут в `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`; Você pode obter muitas configurações e usar réplicas para abrir. |
| Parâmetros de atualização: Selecione `ManifestPolicyV1` em config/состояния управления; разрешить обновления через события управления. | Conselho de Governança | Use CLI para política de implementação. |
| Solução de problemas: use a opção Norito para telemetria (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | Observabilidade | Определить схему событий + logging. |

Teste:
- Юнит-тесты для каждой ponto de entrada (позитивные + отказные сценарии).
- Propriedades para цепочки преемственности (não циклов, монотонные эпохи).
- Fuzz valida com генерацией случайных manifestos (com ограничениями).

## Fachada do serviço (Integração Torii/SDK)

| Componente | Bem | Exibição |
|-----------|--------|---------------|
| Serviço Torii | Экспонировать `/v1/sorafs/pin` (enviar), `/v1/sorafs/pin/{cid}` (pesquisa), `/v1/sorafs/aliases` (listar/vincular), `/v1/sorafs/replication` (pedidos/recebimentos). Selecione a página + filtro. | Rede TL / Core Infra |
| Atestado | Включать высоту/хэш registro em ответы; Obtenha a estrutura de referência Norito, SDK fornecido. | Infra principal |
| CLI | Selecione `sorafs_manifest_stub` ou novo CLI `sorafs_pin` com `pin submit`, `alias bind`, `order issue`, `registry export`. | GT Ferramentaria |
| SDK | Сгенерировать клиентские bindings (Rust/Go/TS) do esquema Norito; Faça testes de integração. | Equipes SDK |

Operação:
- Crie um cache/ETag para endpoints GET.
- Limite de taxa / autenticação de acordo com a política Torii.

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

- `GET /v1/sorafs/pin` e `GET /v1/sorafs/pin/{digest}` manifestam-se
  ligações de alias, replicações de pedidos e certificados de obtenção, proизводным от
  хэша последнего блока.
- `GET /v1/sorafs/aliases` e `GET /v1/sorafs/replication` publicados ativos
  alias de catálogo e backlog cria replicações com página de consistência e
  estado do filtro.

CLI оборачивает эти вызовы (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`), чтобы операторы могли автоматизировать аудиты registro
Não é uma API aprimorada.