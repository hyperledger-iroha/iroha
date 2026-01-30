---
lang: he
direction: rtl
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/nexus/nexus-bootstrap-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a2ffea15e332b6b8b31d27667784abdf6e87c701176041cad11e31b981da53ec
source_last_modified: "2025-11-14T04:43:20.364699+00:00"
translation_last_reviewed: 2026-01-30
---

:::note Fonte canonica
Esta pagina reflete `docs/source/soranexus_bootstrap_plan.md`. Mantenha as duas copias alinhadas ate que as versoes localizadas cheguem ao portal.
:::

# Plano de bootstrap e observabilidade do Sora Nexus

## Objetivos
- Levantar a rede base de validadores/observadores Sora Nexus com chaves de governanca, APIs Torii e monitoramento de consenso.
- Validar servicos centrais (Torii, consenso, persistencia) antes de habilitar deploys piggyback SoraFS/SoraNet.
- Estabelecer workflows de CI/CD e dashboards/alertas de observabilidade para garantir a saude da rede.

## Prerequisitos
- Material de chaves de governanca (multisig do conselho, chaves de comite) disponivel em HSM ou Vault.
- Infraestrutura base (clusters Kubernetes ou nos bare-metal) em regioes primaria/secundaria.
- Configuracao de bootstrap atualizada (`configs/nexus/bootstrap/*.toml`) refletindo os parametros de consenso mais recentes.

## Ambientes de rede
- Operar dois ambientes Nexus com prefixos de rede distintos:
- **Sora Nexus (mainnet)** - prefixo de rede de producao `nexus`, hospedando governanca canonica e servicos piggyback SoraFS/SoraNet (chain ID `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** - prefixo de rede de staging `testus`, espelhando a configuracao da mainnet para testes de integracao e validacao pre-release (chain UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- Manter arquivos genesis separados, chaves de governanca e footprints de infraestrutura para cada ambiente. Testus atua como campo de provas para rollouts SoraFS/SoraNet antes de promover para Nexus.
- Pipelines de CI/CD devem fazer deploy primeiro em Testus, executar smoke tests automatizados e exigir promocao manual para Nexus quando os checks passarem.
- Bundles de configuracao de referencia vivem em `configs/soranexus/nexus/` (mainnet) e `configs/soranexus/testus/` (testnet), cada um contendo `config.toml`, `genesis.json` e diretorios de admissao Torii de exemplo.

## Etapa 1 - Revisao de configuracao
1. Auditar documentacao existente:
   - `docs/source/nexus/architecture.md` (consenso, layout de Torii).
   - `docs/source/nexus/deployment_checklist.md` (requisitos de infraestrutura).
   - `docs/source/nexus/governance_keys.md` (procedimentos de custodia de chaves).
2. Validar que arquivos genesis (`configs/nexus/genesis/*.json`) alinham com o roster atual de validadores e pesos de staking.
3. Confirmar parametros de rede:
   - Tamanho do comite de consenso e quorum.
   - Intervalo de blocos / limites de finalidade.
   - Portas do servico Torii e certificados TLS.

## Etapa 2 - Deploy do cluster bootstrap
1. Provisionar nos validadores:
   - Deploy de instancias `irohad` (validadores) com volumes persistentes.
   - Garantir que regras de firewall permitam trafego de consenso e Torii entre nos.
2. Iniciar servicos Torii (REST/WebSocket) em cada validador com TLS.
3. Deploy de nos observadores (somente leitura) para resiliencia adicional.
4. Executar scripts de bootstrap (`scripts/nexus_bootstrap.sh`) para distribuir genesis, iniciar consenso e registrar nos.
5. Executar smoke tests:
   - Enviar transacoes de teste via Torii (`iroha_cli tx submit`).
   - Verificar producao/finalidade de blocos via telemetria.
   - Checar replicacao do ledger entre validadores/observadores.

## Etapa 3 - Governanca e gestao de chaves
1. Carregar configuracao multisig do conselho; confirmar que propostas de governanca podem ser submetidas e ratificadas.
2. Armazenar com seguranca chaves de consenso/comite; configurar backups automaticos com logging de acesso.
3. Configurar procedimentos de rotacao de chaves de emergencia (`docs/source/nexus/key_rotation.md`) e verificar o runbook.

## Etapa 4 - Integracao CI/CD
1. Configurar pipelines:
   - Build e publicacao de imagens validator/Torii (GitHub Actions ou GitLab CI).
   - Validacao automatizada de configuracao (lint de genesis, verificacao de assinaturas).
   - Pipelines de deploy (Helm/Kustomize) para clusters de staging e producao.
2. Implementar smoke tests no CI (subir cluster efemero, rodar suite canonica de transacoes).
3. Adicionar scripts de rollback para deploys com falha e documentar runbooks.

## Etapa 5 - Observabilidade e alertas
1. Deploy do stack de monitoramento (Prometheus + Grafana + Alertmanager) por regiao.
2. Coletar metricas centrais:
  - `nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - Logs via Loki/ELK para servicos Torii e consenso.
3. Dashboards:
   - Saude do consenso (altura de bloco, finalidade, status de peers).
   - Latencia e taxa de erro da API Torii.
   - Transacoes de governanca e status de propostas.
4. Alertas:
   - Parada de producao de blocos (>2 intervalos de bloco).
   - Queda no numero de peers abaixo do quorum.
   - Picos na taxa de erro de Torii.
   - Backlog da fila de propostas de governanca.

## Etapa 6 - Validacao e handoff
1. Rodar validacao end-to-end:
   - Submeter proposta de governanca (ex. mudanca de parametro).
   - Processar a aprovacao do conselho para garantir que o pipeline de governanca funciona.
   - Rodar diff de estado do ledger para garantir consistencia.
2. Documentar o runbook para on-call (resposta a incidentes, failover, scaling).
3. Comunicar prontidao para equipes SoraFS/SoraNet; confirmar que deploys piggyback podem apontar para nos Nexus.

## Checklist de implementacao
- [ ] Auditoria de genesis/configuracao concluida.
- [ ] Nos validadores e observadores deployados com consenso saudavel.
- [ ] Chaves de governanca carregadas, proposta testada.
- [ ] Pipelines CI/CD rodando (build + deploy + smoke tests).
- [ ] Dashboards de observabilidade ativos com alertas.
- [ ] Documentacao de handoff entregue aos times downstream.
