---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano de registro de pinos
título: SoraFS Pin Registry نفاذی منصوبہ
sidebar_label: Pin Registry منصوبہ
descrição: SF-4 نفاذی منصوبہ جو registro کی máquina de estado, fachada Torii, ferramentas e observabilidade کو کور کرتا ہے۔
---

:::nota مستند ماخذ
یہ صفحہ `docs/source/sorafs/pin_registry_plan.md` کی عکاسی کرتا ہے۔ جب تک پرانی دستاویزات فعال ہیں دونوں نقول ہم آہنگ رکھیں۔
:::

# SoraFS Pin Registry نفاذی منصوبہ (SF-4)

Registro SF-4 Pin
políticas de pin نافذ کرتی ہیں, اور Torii, gateways e orquestradores کے لیے APIs ظاہر کرتی ہیں۔
یہ دستاویز plano de validação کو ٹھوس tarefas de implementação سے بڑھاتی ہے، جس میں lógica on-chain،
serviços do lado do host, luminárias, e outros serviços de hospedagem

## دائرہ کار

1. **máquina de estado de registro**: registros definidos por Norito برائے manifestos, aliases, cadeias de sucessores,
   épocas de retenção e metadados de governança.
2. **کنٹریکٹ نفاذ**: ciclo de vida do pino کے لیے operações CRUD determinísticas (`ReplicationOrder`,
   `Precommit`, `Completion`, despejo).
3. **fachada **: endpoints gRPC/REST e registro سے بیکڈ ہوں اور Torii e SDKs انہیں استعمال کریں
   جن میں paginação e atestado شامل ہے۔
4. **ferramentas e acessórios**: auxiliares CLI, vetores de teste, documentação, manifestos, aliases e
   envelopes de governança
5. **telemetria e operações**: registro de métricas, alertas e runbooks.

## ڈیٹا ماڈل

### بنیادی ریکارڈز (Norito)

| Estrutura | وضاحت | فیلڈز |
|----|-------|-------|
| `PinRecordV1` | entrada de manifesto canônico. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | alias -> mapeamento CID do manifesto. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | provedores کو pin de manifesto کرنے کی ہدایت. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | provider acknowledgement. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | instantâneo da política de governação. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Referência de implementação: esquemas `crates/sorafs_manifest/src/pin_registry.rs` دیکھیں جہاں Rust Norito
Ajudantes de validação موجود ہیں۔ Ferramentas de manifesto de validação (pesquisa de registro de chunker, controle de política de pinos)
کو آئینہ کرتی ہے تاکہ کنٹریکٹ, Fachadas Torii, اور CLI ایک جیسے invariantes شیئر کریں۔

Tarefas:
- `crates/sorafs_manifest/src/pin_registry.rs` میں Esquemas Norito مکمل کریں۔
- Macros Norito سے geração de código کریں (Rust + دیگر SDKs)۔
- esquemas آ جانے کے بعد docs (`sorafs_architecture_rfc.md`) اپڈیٹ کریں۔

## کنٹریکٹ نفاذ| کام | مالک/مالکان | Não |
|-----|-------------|------|
| armazenamento de registro (sled/sqlite/off-chain) یا módulo de contrato inteligente نافذ کریں۔ | Equipe Core de Infra/Contrato Inteligente | hashing determinístico فراہم کریں, ponto flutuante سے بچیں۔ |
| Pontos de entrada: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Infra principal | plano de validação سے `ManifestValidator` استعمال کریں۔ ligação de alias اب `RegisterPinManifest` (Torii DTO) لیے منصوبہ بند ہے۔ |
| Transições de estado: sucessão (manifesto A -> B), épocas de retenção, e exclusividade do alias نافذ کریں۔ | Conselho de Governança / Core Infra | exclusividade do alias, limites de retenção, e verificações de aprovação/retirada do antecessor اب `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` میں ہیں؛ detecção de sucessão multi-hop e escrituração contábil de replicação |
| Parâmetros governados: `ManifestPolicyV1` کو estado de configuração/governança سے لوڈ کریں؛ eventos de governança | Conselho de Governança | atualizações de política کے لیے CLI فراہم کریں۔ |
| Emissão de eventos: telemetria کے لیے Eventos Norito (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`) جاری کریں۔ | Observabilidade | esquema de evento + registro متعین کریں۔ |

Teste:
- ہر ponto de entrada کے لیے testes unitários (positivo + rejeição).
- cadeia de sucessão کے لیے testes de propriedades (sem ciclos, épocas monotônicas).
- manifestos aleatórios (limitados) ou validação fuzz.

## Fachada transparente (Torii/SDK انضمام)

| sim | کام | مالک/مالکان |
|------|-----|------------|
| Serviço Torii | `/v1/sorafs/pin` (enviar), `/v1/sorafs/pin/{cid}` (pesquisa), `/v1/sorafs/aliases` (listar/vincular)، `/v1/sorafs/replication` (pedidos/recebimentos) فراہم کریں۔ paginação + filtragem مہیا کریں۔ | Rede TL / Core Infra |
| Atestado | respostas میں altura / hash do registro شامل کریں؛ Estrutura de atestado Norito شامل کریں جسے SDKs consomem کریں۔ | Infra principal |
| CLI | `sorafs_manifest_stub` میں توسیع یا نئی `sorafs_pin` CLI بنائیں جس میں `pin submit`, `alias bind`, `order issue`, `registry export` ہو۔ | GT Ferramentaria |
| SDK | Esquema Norito سے ligações de cliente (Rust/Go/TS) geram کریں؛ testes de integração | Equipes SDK |

Operações:
- GET endpoints کے لیے cache/ETag camada شامل کریں۔
- Políticas Torii کے مطابق limitação de taxa / autenticação فراہم کریں۔

## Calendário do CI- Diretório de luminárias: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` میں manifesto/alias/instantâneos de pedido assinados محفوظ ہوتے ہیں جو `cargo run -p iroha_core --example gen_pin_snapshot` سے regenerar ہوتے ہیں۔
- Etapa CI: `ci/check_sorafs_fixtures.sh` regeneração de snapshot کرتا ہے اور diff ہونے پر fail کرتا ہے تاکہ CI fixtures alinhados رہیں۔
- Testes de integração (`crates/iroha_core/tests/pin_registry.rs`) caminho feliz کے ساتھ rejeição de alias duplicados, aprovação de alias/guardas de retenção, identificadores de chunker incompatíveis, validação de contagem de réplicas, اور falhas de proteção de sucessão (desconhecido/pré-aprovado/aposentado/auto ponteiros) کور کرتے ہیں؛ تفصیل کے لیے `register_manifest_rejects_*` casos دیکھیں۔
- Testes de unidade اب `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` میں validação de alias, guardas de retenção, e verificações de sucessor کور کرتے ہیں؛ detecção de sucessão multi-hop تب آئے گا جب máquina de estado
- Pipelines de observabilidade para eventos JSON dourados۔

## Telemetria e Observabilidade

Métricas (Prometheus):
-`torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
-`torii_sorafs_registry_aliases_total`
-`torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
-`torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
-`torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
-`torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- Telemetria do provedor موجودہ (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) painéis de ponta a ponta کے لیے escopo میں رہے گی۔

Registros:
- auditorias de governança کے لیے fluxo de eventos Norito estruturado (assinado?).

Alertas:
- SLA سے زیادہ ordens de replicação pendentes.
- limite de expiração do alias سے کم.
- violações de retenção (renovação manifesta وقت سے پہلے نہ ہو).

Painéis:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` totais do ciclo de vida do manifesto, cobertura de alias, saturação de pendências, proporção de SLA, latência vs sobreposições de folga, taxas de pedidos perdidos e revisão de plantão کے لیے دکھاتا ہے۔

## Runbooks e documentação

- `docs/source/sorafs/migration_ledger.md` کو atualizações de status do registro شامل کرنے کے لیے اپڈیٹ کریں۔
- Guia do operador: métricas `docs/source/sorafs/runbooks/pin_registry_ops.md` (referências), alertas, implantação, backup, fluxos de recuperação e fluxos de recuperação کور کرتا ہے۔
- Guia de governança: parâmetros de política, fluxo de trabalho de aprovação, tratamento de disputas بیان کریں۔
- ہر endpoint کے لیے páginas de referência da API (documentos Docusaurus).

## Dependências e Sequenciamento

1. tarefas do plano de validação مکمل کریں (integração do ManifestValidator).
2. Esquema Norito + padrões de política
3. contrato + serviço نافذ کریں اور fio de telemetria کریں۔
4. fixtures regeneram کریں اور suítes de integração چلائیں۔
5. docs/runbooks اپڈیٹ کریں اور itens de roteiro کو مکمل مارک کریں۔

Lista de verificação SF-4 کے ہر
Fachada REST اب endpoints de listagem atestados کے ساتھ آتی ہے:

- `GET /v1/sorafs/pin` e `GET /v1/sorafs/pin/{digest}` manifesta واپس کرتے ہیں جن میں
  ligações de alias, ordens de replicação, اور تازہ ترین bloco hash سے ماخوذ objeto de atestado شامل ہے۔
- `GET /v1/sorafs/aliases` e `GET /v1/sorafs/replication` no catálogo de alias
  backlog de pedidos de replicação کو paginação consistente اور filtros de status کے ساتھ ظاہر کرتے ہیں۔

CLI não chama کو wrap کرتی ہے (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) تاکہ operadores کم سطحی APIs کو چھوئے بغیر auditorias de registro خودکار بنا سکیں۔