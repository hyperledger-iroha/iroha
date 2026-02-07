---
lang: pt
direction: ltr
source: docs/portal/docs/soranet/testnet-rollout.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: lançamento testnet
título: Implementação do testnet SoraNet (SNNet-10)
sidebar_label: implementação do Testnet (SNNet-10)
descrição: plano de ativação do plano de ativação, kit de integração, promoções da rede de teste SoraNet e portões de telemetria.
---

:::nota Fonte Canônica
یہ صفحہ `docs/source/soranet/testnet_rollout_plan.md` میں Plano de implementação SNNet-10 کی عکاسی کرتا ہے۔ جب تک پرانا docs set retirar نہ ہو, دونوں کاپیاں sincronização رکھیں۔
:::

SNNet-10 نیٹ ورک بھر میں Sobreposição de anonimato SoraNet کی مرحلہ وار ativação کو coordenada کرتا ہے۔ اس plano کو استعمال کریں تاکہ roteiro bullet کو entregas concretas, runbooks, اور portas de telemetria میں بدلا جا سکے تاکہ ہر operador توقعات سمجھ لے اس سے پہلے کہ Transporte padrão SoraNet بنے۔

## Fases de lançamento

| Fase | Linha do tempo (alvo) | Escopo | Artefactos necessários |
|-------|-------------------|-------|---------|
| **T0 - Testnet Fechado** | 4º trimestre de 2026 | >=3 ASNs entre 20-50 relés e contribuidores principais | Kit de integração Testnet, suíte de fumaça de fixação de proteção, latência de linha de base + métricas PoW, registro de perfuração de brownout. |
| **T1 - Beta Público** | 1º trimestre de 2027 | >=100 relés, rotação de guarda habilitada, ligação de saída aplicada, SDK betas padrão طور پر SoraNet کے ساتھ `anon-guard-pq`. | Kit de integração atualizado, lista de verificação de verificação do operador, SOP de publicação de diretório, pacote de painel de telemetria, relatórios de ensaio de incidentes. |
| **T2 - Mainnet Padrão** | 2º trimestre de 2027 (conclusão SNNet-6/7/9 fechada) | Rede de produção SoraNet é padrão; transportes obfs/MASQUE e aplicação de catraca PQ habilitada۔ | Minutas de aprovação de governança, procedimento de reversão somente direto, alarmes de downgrade, relatório de métricas de sucesso assinado. |

**کوئی pular caminho نہیں** - ہر fase کو پچھلے مرحلے کی telemetria اور artefatos de governança navio کرنا لازمی ہے قبل از promoção۔

## Kit de integração Testnet

O operador de retransmissão کو درج ذیل فائلوں کے ساتھ ایک pacote determinístico ملتا ہے:

| Artefato | Descrição |
|----------|------------|
| `01-readme.md` | Visão geral, pontos de contato, cronograma e cronograma |
| `02-checklist.md` | Lista de verificação pré-voo (hardware, acessibilidade da rede, verificação da política de proteção). |
| `03-config-example.toml` | Blocos de conformidade SNNet-9 کے ساتھ alinhar کیا ہوا relé SoraNet mínimo + configuração do orquestrador, جس میں bloco `guard_directory` شامل ہے جو تازہ ترین guard snapshot hash کو pin کرتا ہے۔ |
| `04-telemetry.md` | Painéis de métricas de privacidade SoraNet e limite de alerta fio کرنے کی ہدایات۔ |
| `05-incident-playbook.md` | Procedimento de resposta de brownout/downgrade e matriz de escalonamento۔ |
| `06-verification-report.md` | Modelo جسے operadores testes de fumaça |

Cópia renderizada `docs/examples/soranet_testnet_operator_kit/` میں موجود ہے۔ ہر promoção میں atualização do kit ہوتا ہے؛ fase de números de versão کو siga کرتے ہیں (مثال کے طور پر `testnet-kit-vT0.1`).

Operadores beta público (T1) کے لئے `docs/source/soranet/snnet10_beta_onboarding.md` میں pré-requisitos breves de integração concisos, resultados de telemetria e fluxo de trabalho de envio خلاصہ کرتا ہے اور kit determinístico اور auxiliares de validação کی طرف اشارہ کرتا ہے۔Feed JSON `cargo xtask soranet-testnet-feed` بناتا ہے جو janela de promoção, lista de retransmissão, relatório de métricas, evidência de perfuração e hashes de anexo کو agregado کرتا ہے جنہیں referência de modelo de portão de estágio کرتا ہے۔ پہلے `cargo xtask soranet-testnet-drill-bundle` سے registros de perfuração اور sinal de anexos کریں تاکہ feed `drill_log.signed = true` registro کر سکے۔

## Métricas de sucesso

Fases کے درمیان promoção درج ذیل telemetria پر fechado ہے، جو کم از کم دو ہفتے جمع کی جاتی ہے:

- `soranet_privacy_circuit_events_total`: 95% de brownout dos circuitos یا downgrade کے بغیر مکمل ہوں؛ Mais de 5% de fornecimento PQ
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}`: روزانہ buscar sessões کا =99%; `soranet_privacy_throttles_total{scope="congestion"}` کے ذریعے relatório ہوتا ہے۔
- Latência (percentil 95) por região: circuitos مکمل ہونے کے بعد <200 ms، `soranet_privacy_rtt_millis{percentile="p95"}` کے ذریعے captura ہوتی ہے۔

Dashboard para modelos de alerta `dashboard_templates/` e `alert_templates/` میں موجود ہیں؛ انہیں اپنے repositório de telemetria میں espelho کریں اور verificações de lint CI میں شامل کریں۔ Promoção کی درخواست سے پہلے relatório voltado para governança بنانے کے لئے `cargo xtask soranet-testnet-metrics` استعمال کریں۔

Envios Stage-gate کو `docs/source/soranet/snnet10_stage_gate_template.md` فالو کرنا ہوگا، جو `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md` میں موجود formulário Markdown pronto para copiar کی طرف لنک کرتا ہے۔

## Lista de verificação de verificação

A fase میں داخل ہونے سے پہلے operadores کو درج ذیل پر sign-off کرنا ہوگا:

- ✅ Anúncio de retransmissão موجودہ envelope de admissão کے ساتھ assinado ہو۔
- ✅ Teste de fumaça de rotação da proteção (`tools/soranet-relay --check-rotation`) پاس ہو۔
- ✅ `guard_directory` تازہ ترین `GuardDirectorySnapshotV2` artefato کی طرف اشارہ کرے اور `expected_directory_hash_hex` resumo do comitê سے match ہو (relay startup valided hash log کرتا ہے)۔
- ✅ Métricas de catraca PQ (`sorafs_orchestrator_pq_ratio`) مطلوبہ estágio کے limites alvo سے اوپر رہیں۔
- ✅ Configuração de conformidade GAR تازہ ترین tag سے match کرے (catálogo SNNet-9 دیکھیں)۔
- ✅ Simulação de alarme de downgrade (coletores desabilitam کریں، 5 منٹ میں alerta esperado کریں)۔
- ✅ Etapas de mitigação documentadas do exercício PoW/DoS کے ساتھ executar ہو۔

Kit de integração de modelo pré-preenchido میں شامل ہے۔ Operadores مکمل رپورٹ helpdesk de governança کو enviar کرتے ہیں, پھر credenciais de produção ملتے ہیں۔

## Governança e relatórios

- **Controle de alterações:** promoções کے لئے Aprovação do Conselho de Governança درکار ہے جو ata do conselho میں درج ہو اور página de status کے ساتھ anexar ہو۔
- **Resumo de status:** ہفتہ وار atualizações شائع کریں جن میں contagem de relés, relação PQ, incidentes de queda de energia e itens de ação pendentes شامل ہوں (cadência شروع ہونے کے بعد `docs/source/status/soranet_testnet_digest.md` میں محفوظ)۔
- **Reversões:** Plano de reversão assinado میں Invalidação de DNS/guard cache e modelos de comunicação do cliente شامل ہوں۔

## Ativos de apoio- `cargo xtask soranet-testnet-kit [--out <dir>]` `xtask/templates/soranet_testnet/` سے onboarding kit کو diretório de destino میں materialize کرتا ہے (padrão `docs/examples/soranet_testnet_operator_kit/`).
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` Métricas de sucesso do SNNet-10 avaliam کرتا ہے اور análises de governança کے لئے relatório estruturado de aprovação/reprovação emite کرتا ہے۔ Exemplo de instantâneo `docs/examples/soranet_testnet_metrics_sample.json` میں موجود ہے۔
- Grafana e Alertmanager templates `dashboard_templates/soranet_testnet_overview.json` e `alert_templates/soranet_testnet_rules.yml` میں موجود ہیں؛ انہیں repositório de telemetria میں copiar کریں یا verificações de fiapos CI میں fio کریں۔
- SDK/mensagens do portal کے لئے modelo de comunicação de downgrade `docs/source/soranet/templates/downgrade_communication_template.md` میں ہے۔
- Resumos semanais de status کو forma canônica کے طور پر `docs/source/status/soranet_testnet_weekly_digest.md` استعمال کرنا چاہئے۔

Solicitações pull کو اس صفحے کو artefatos یا telemetria میں کسی بھی تبدیلی کے ساتھ atualização کرنا چاہئے تاکہ plano de implementação canônico رہے۔