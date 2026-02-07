---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano nexus-bootstrap
título: Bootstrap e observação Sora Nexus
description: Planeje operações para fornecer on-line o cluster central de validação Nexus antes de adicionar os serviços SoraFS e SoraNet.
---

:::nota Fonte canônica
Esta página reflete `docs/source/soranexus_bootstrap_plan.md`. Gardez as duas cópias alinhadas até que as versões localizadas cheguem ao portal.
:::

# Plano de inicialização e observação Sora Nexus

## Objetivos
- Coloque no local a reserva de validadores/observadores de base Sora Nexus com controles de governança, APIs Torii e monitoramento de consenso.
- Valide os serviços coeur (Torii, consenso, persistência) antes de ativar as implantações nas costas do SoraFS/SoraNet.
- Defina fluxos de trabalho CI/CD e painéis/alertas de observação para garantir a integridade do seu patrimônio.

## Pré-requisito
- Material de cles de gouvernance (multisig du conseil, cles de comite) disponível em HSM ou Vault.
- Infraestrutura de base (clusters Kubernetes ou noeuds bare-metal) nas regiões primária/secundária.
- A configuração do bootstrap atual (`configs/nexus/bootstrap/*.toml`) reflete os parâmetros de consenso mais recentes.

## Rede de ambientes
- Operar dois ambientes Nexus com prefixos de recursos distintos:
- **Sora Nexus (mainnet)** - prefixo reseau de produção `nexus`, incluindo o governo canônico e os serviços nas costas SoraFS/SoraNet (chain ID `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** - prefixo reseau de staging `testus`, espelho da configuração mainnet para testes de integração e validação de pré-lançamento (cadeia UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- Manter a gênese dos arquivos, os códigos de governo e as empresas de infraestrutura separadas para cada ambiente. O teste é um terreno de teste para todos os lançamentos SoraFS/SoraNet antes da promoção vers Nexus.
- Os pipelines CI/CD devem ser implantados a bordo do Testus, executores de testes de fumaça automatizados e exigem uma promoção manual para Nexus uma vez que as verificações são aprovadas.
- Os pacotes de configuração de referência foram encontrados sob `configs/soranexus/nexus/` (mainnet) e `configs/soranexus/testus/` (testnet), cada um contendo um exemplo `config.toml`, `genesis.json` e os repertórios de admissão Torii.

## Etapa 1 - Revisão de configuração
1. Auditar a documentação existente:
   - `docs/source/nexus/architecture.md` (consenso, layout Torii).
   - `docs/source/nexus/deployment_checklist.md` (exigências infra).
   - `docs/source/nexus/governance_keys.md` (procedures de garde des cles).
2. Valide os arquivos genesis (`configs/nexus/genesis/*.json`) alinhados com a lista atual de validadores e os pesos de staking.
3. Confirme os parâmetros restantes:
   - Taille du comité de consenso e quorum.
   - Intervalle de bloc / seusils de finalite.
   - Portas de serviço Torii e certificados TLS.

## Etapa 2 - Implantação do cluster bootstrap
1. Fornecer noeus validadores:
   - Implementador de instâncias `irohad` (validadores) com volumes persistentes.
   - Verificador de que as regras do firewall autorizam o consenso de tráfego e Torii entre nós.
2. Demarque os serviços Torii (REST/WebSocket) em cada validador com TLS.
3. Implantar novos observadores (palestra única) para adicionar resiliência.
4. Execute os scripts bootstrap (`scripts/nexus_bootstrap.sh`) para a gênese do distribuidor, demarque o consenso e registre os noeuds.
5. Execução de testes de fumaça:
   - Realização de transações de teste via Torii (`iroha_cli tx submit`).
   - Verificador da produção/finalização dos blocos via telemetria.
   - Verifica a replicação do livro-razão entre validadores/observadores.

## Etapa 3 - Governança e gestão dos cles
1. Carregador com configuração multisig du conseil; confirmar que as propostas de governo podem ser aprovadas e ratificadas.
2. Stocker de maniere securisee les cles de consenso/comité; configurar segurança automaticamente com diário de acesso.
3. Coloque os procedimentos de rotação dos códigos de urgência (`docs/source/nexus/key_rotation.md`) e verifique o runbook.## Etapa 4 - Integração CI/CD
1. Configure os pipelines:
   - Construção e publicação do validador de imagens/Torii (GitHub Actions ou GitLab CI).
   - Validação automática de configuração (gênese de lint, verificação de assinaturas).
   - Pipelines de implantação (Helm/Kustomize) para preparação e produção de clusters.
2. Implementar testes de fumaça em CI (demarcar um cluster efêmero, executar o conjunto canônico de transações).
3. Adicione scripts de reversão para taxas de implantação e documente runbooks.

## Etapa 5 - Observabilidade e alertas
1. Implante a pilha de monitoramento (Prometheus + Grafana + Alertmanager) por região.
2. Colete as métricas do coração:
  -`nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - Logs via Loki/ELK para os serviços Torii e consenso.
3. Painéis:
   - Sante du consenso (hauteur de bloc, finalité, statut des peers).
   - Latência/taxa de erro da API Torii.
   - Transações de governo e estatuto de propostas.
4. Alertas:
   - Interrupção da produção de blocos (>2 intervalos de bloco).
   - Baisse du nombre de peers sous le quorum.
   - Fotos do erro Torii.
   - Backlog do arquivo de propostas de governo.

## Etapa 6 - Validação e transferência
1. Executar a validação ponta a ponta:
   - Soumettre uneposition de gouvernance (p. ex. changement de parametre).
   - Faça a aprovação do conselho para garantir que o pipeline de governança funcione.
   - Executar uma diferença de estado do livro-razão para garantir a coerência.
2. Documente o runbook para plantão (incidente de resposta, failover, escalonamento).
3. Comunique a disponibilidade às equipes SoraFS/SoraNet; Confirme que as implantações piggyback podem apontar para os noeuds Nexus.

## Checklist de implementação
- [ ] Término da gênese/configuração da auditoria.
- [] Noeuds validadores e observadores implantam com um consenso sain.
- [ ] Cles de gouvernance chargees, proposição testada.
- [ ] Pipelines CI/CD em marcha (construção + implantação + testes de fumaça).
- [] Painéis de observação de atividades com alertas.
- [ ] Documentação de transferência gratuita para equipes downstream.