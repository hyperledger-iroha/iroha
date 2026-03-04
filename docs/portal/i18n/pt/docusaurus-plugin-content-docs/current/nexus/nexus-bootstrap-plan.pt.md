---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano nexus-bootstrap
título: Bootstrap e observabilidade do Sora Nexus
description: Plano operacional para colocar o cluster central de validadores Nexus online antes de adicionar os serviços SoraFS e SoraNet.
---

:::nota Fonte canônica
Esta página reflete `docs/source/soranexus_bootstrap_plan.md`. Mantenha as duas cópias homologadas até que os versoes localizados cheguem ao portal.
:::

# Plano de bootstrap e observabilidade do Sora Nexus

## Objetivos
- Levantar a rede base de validadores/observadores Sora Nexus com chaves de governança, APIs Torii e monitoramento de consenso.
- Validar serviços centrais (Torii, consenso, persistência) antes de habilitar o deploys piggyback SoraFS/SoraNet.
- Definir fluxos de trabalho de CI/CD e dashboards/alertas de observabilidade para garantir a saúde da rede.

## Pré-requisitos
- Material de chaves de governança (multisig do conselho, chaves de comitê) disponível em HSM ou Vault.
- Base de infraestrutura (clusters Kubernetes ou nos bare-metal) em regiões primárias/secundárias.
- Configuração de bootstrap atualizada (`configs/nexus/bootstrap/*.toml`) refletindo os parâmetros de consenso mais recentes.

## Ambientes de rede
- Operar dois ambientes Nexus com prefixos de rede diferentes:
- **Sora Nexus (mainnet)** - prefixo de rede de produção `nexus`, hospedando governança canônica e serviços piggyback SoraFS/SoraNet (chain ID `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** - prefixo de rede de staging `testus`, espelhando a configuração da mainnet para testes de integração e validação de pré-lançamento (chain UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- Manter arquivos separados de gênese, chaves de governança e pegadas de infraestrutura para cada ambiente. Testus atua como campo de testes para lançamentos SoraFS/SoraNet antes de promoção para Nexus.
- Pipelines de CI/CD devem ser implantados primeiro em Testus, executar testes de fumaça automatizados e exigir manual de promoção para Nexus quando as verificações passarem.
- Bundles de configuração de referência residem em `configs/soranexus/nexus/` (mainnet) e `configs/soranexus/testus/` (testnet), cada um contendo `config.toml`, `genesis.json` e diretórios de admissão Torii de exemplo.

## Etapa 1 - Revisão de configuração
1. Auditar documentação existente:
   - `docs/source/nexus/architecture.md` (consenso, layout de Torii).
   - `docs/source/nexus/deployment_checklist.md` (requisitos de infraestrutura).
   - `docs/source/nexus/governance_keys.md` (procedimentos de custódia de chaves).
2. Validar que arquivos genesis (`configs/nexus/genesis/*.json`) alinham com a lista atual de validadores e pesos de staking.
3. Confirme os parâmetros da rede:
   - Tamanho do comitê de consenso e quorum.
   - Intervalo de blocos / limites específicos.
   - Portas do serviço Torii e certificados TLS.

## Etapa 2 - Implantar o bootstrap do cluster
1. Provisórios nos validadores:
   - Deploy de instâncias `irohad` (validadores) com volumes persistentes.
   - Garantir que regras de firewall permitam tráfego de consenso e Torii entre nós.
2. Inicie os serviços Torii (REST/WebSocket) em cada validador com TLS.
3. Implante nossos observadores (somente leitura) para resiliência adicional.
4. Executar scripts de bootstrap (`scripts/nexus_bootstrap.sh`) para distribuir genesis, iniciar consenso e registrador nos.
5. Executar testes de fumaça:
   - Enviar transações de teste via Torii (`iroha_cli tx submit`).
   - Verificar produção/finalidade de blocos via telemetria.
   - Checar replicacao do ledger entre validadores/observadores.

## Etapa 3 - Governança e gestão de chaves
1. Carregar configuração multisig do conselho; confirmar que as propostas de governança podem ser submetidas e ratificadas.
2. Armazenar com segurança chaves de consenso/comite; configurar backups automáticos com registro de acesso.
3. Configure procedimentos de rotação de chaves de emergência (`docs/source/nexus/key_rotation.md`) e verifique o runbook.

## Etapa 4 - Integração CI/CD
1. Configurar pipelines:
   - Construir e publicar o validador de imagens/Torii (GitHub Actions ou GitLab CI).
   - Validação automatizada de configuração (lint de gênese, verificação de assinaturas).
   - Pipelines de implantação (Helm/Kustomize) para clusters de staging e produção.
2. Implementar testes de fumaça no CI (subir cluster efemero, rodar suite canonica de transacoes).
3. Adicione scripts de reversão para implantações com falha e documente runbooks.## Etapa 5 - Observabilidade e alertas
1. Implante a pilha de monitoramento (Prometheus + Grafana + Alertmanager) por região.
2. Coletar métricas centrais:
  -`nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - Logs via Loki/ELK para serviços Torii e consenso.
3. Painéis:
   - Saúde do consenso (altura de bloco, específica, status de pares).
   - Latência e taxa de erro da API Torii.
   - Transações de governança e status de propostas.
4. Alertas:
   - Parada de produção de blocos (>2 intervalos de bloco).
   - Queda no número de pares abaixo do quorum.
   - Picos na taxa de erro de Torii.
   - Backlog da fila de propostas de governança.

## Etapa 6 - Validação e transferência
1. Rodar validação ponta a ponta:
   - Submeter proposta de governança (ex. mudança de parâmetro).
   - Processar a aprovação do conselho para garantir que o pipeline de governança funcione.
   - Rodar diferença de estado do razão para garantir consistência.
2. Documentar o runbook para plantão (resposta a incidentes, failover, escalonamento).
3. Comunicar prontidão para equipes SoraFS/SoraNet; confirmar que implants piggyback podem indicar para nos Nexus.

## Checklist de implementação
- [ ] Auditoria de génese/configuração concluída.
- [ ] Nos validadores e observadores implantados com consenso saudável.
- [ ] Chaves de governança investidas, proposta testada.
- [ ] Pipelines CI/CD rodando (construção + implantação + testes de fumaça).
- [ ] Dashboards de observabilidade ativa com alertas.
- [ ] Documentação de entrega entregue aos tempos downstream.