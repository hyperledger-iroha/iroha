---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: implantação do desenvolvedor
título: notas de implantação SoraFS
sidebar_label: notas de implantação
descrição: CI سے produção تک SoraFS pipeline promover کرنے کی checklist۔
---

:::nota مستند ماخذ
:::

# Notas de implantação

SoraFS determinismo de fluxo de trabalho de empacotamento Existem vários gateways e provedores de armazenamento, implementação, lista de verificação e lista de verificação.

## Pré-voo

- **Alinhamento de registro** — تصدیق کریں کہ perfis chunker اور manifestos ایک ہی Tupla `namespace.name@semver` کو consulte کرتے ہیں (`docs/source/sorafs/chunker_registry.md`).
- **Política de admissão** — `manifest submit` کے لئے درکار anúncios de fornecedores assinados اور provas de alias کا جائزہ لیں (`docs/source/sorafs/provider_admission_policy.md`).
- **Runbook de registro PIN** — cenários de recuperação (rotação de alias, falhas de replicação) کے لئے `docs/source/sorafs/runbooks/pin_registry_ops.md` قریب رکھیں۔

## Configuração do ambiente

- Gateways e endpoint de streaming de prova (`POST /v2/sorafs/proof/stream`) habilitam کرنا ہوگا تاکہ Os resumos de telemetria CLI emitem کر سکے۔
- Política `sorafs_alias_cache` کو padrões `iroha_config` یا auxiliar CLI (`sorafs_cli manifest submit --alias-*`) کے ذریعے configurar کریں۔
- Tokens de fluxo (credenciais Torii) کو ایک محفوظ gerenciador secreto سے فراہم کریں۔
- Exportadores de telemetria (`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`) habilitam کریں اور انہیں اپنے Prometheus/OTel stack میں navio کریں۔

## Estratégia de implementação

1. **Manifestos azuis/verdes**
   - ہر implementação کے لئے arquivo de respostas کرنے کے لئے `manifest submit --summary-out` استعمال کریں۔
   - `torii_sorafs_gateway_refusals_total` پر نظر رکھیں تاکہ incompatibilidades de capacidade جلدی پکڑ لیں۔
2. **Validação da prova**
   - `sorafs_cli proof stream` میں falhas کو bloqueadores de implantação سمجھیں؛ picos de latência اکثر limitação do provedor یا camadas mal configuradas
   - Teste de fumaça pós-pin میں `proof verify` شامل کریں تاکہ یقینی ہو کہ provedores پر hospedado CAR اب بھی manifest digest سے match کرتا ہے۔
3. **Painéis de telemetria**
   - `docs/examples/sorafs_proof_streaming_dashboard.json` کو Grafana میں importação کریں۔
   - Integridade do registro de pinos (`docs/source/sorafs/runbooks/pin_registry_ops.md`) e estatísticas de intervalo de blocos کے لئے اضافی painéis لگائیں۔
4. **Ativação de múltiplas fontes**
   - Orchestrator آن کرتے وقت `docs/source/sorafs/runbooks/multi_source_rollout.md` کے etapas de implementação em etapas فالو کریں, اور auditorias کے لئے placar/arquivo de artefatos de telemetria کریں۔

## Tratamento de incidentes

- `docs/source/sorafs/runbooks/` caminhos de escalonamento de caminhos de escalonamento:
  - Interrupções do gateway `sorafs_gateway_operator_playbook.md` e esgotamento do token de fluxo کے لئے۔
  - `dispute_revocation_runbook.md` جب disputas de replicação ہوں۔
  - Manutenção em nível de nó `sorafs_node_ops.md` کے لئے۔
  - Substituições do orquestrador `multi_source_rollout.md`, lista negra de pares e implementações em etapas
- Prova de falhas e anomalias de latência کو GovernanceLog میں موجود PoR tracker APIs کے ذریعے registro کریں تاکہ avaliação de desempenho do provedor de governança کر سکے۔

##Próximas etapas

- Orquestrador de busca multifonte (SF-6b) e automação de orquestrador (`sorafs_car::multi_fetch`) integrado کریں۔
- Atualizações PDP/PoTR کو SF-13/SF-14 کے تحت faixa کریں؛ جب یہ as provas estabilizam ہوں تو CLI اور prazos de documentos اور superfície de seleção de nível کریں گے۔ان notas de implantação کو início rápido اور receitas CI کے ساتھ ملانے سے ٹیمیں experimentos locais سے pipelines SoraFS de nível de produção تک repetível اور processo observável کے ساتھ جا سکتی ہیں۔