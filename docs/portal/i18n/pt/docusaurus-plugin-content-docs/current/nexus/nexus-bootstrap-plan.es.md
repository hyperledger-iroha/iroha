---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano nexus-bootstrap
título: Bootstrap e observação de Sora Nexus
description: Plano operacional para colocar em linha o cluster central de validadores Nexus antes de agregar serviços SoraFS e SoraNet.
---

:::nota Fonte canônica
Esta página reflete `docs/source/soranexus_bootstrap_plan.md`. Mantenha ambas as cópias alinhadas até que as versões localizadas cheguem ao portal.
:::

# Plano de inicialização e observação de Sora Nexus

## Objetivos
- Levantar a base vermelha de validadores/observadores de Sora Nexus com folhas de governo, APIs de Torii e monitorar o consenso.
- Validar serviços centrais (Torii, consenso, persistência) antes de habilitar despliegues piggyback de SoraFS/SoraNet.
- Estabeleça fluxos de trabalho de CI/CD e painéis/alertas de observação para garantir a saúde da rede.

## Pré-requisitos
- Material de folhas de governo (multisig del consejo, folhas de comitê) disponíveis em HSM ou Vault.
- Base de infraestrutura (clusters Kubernetes ou nodos bare-metal) em regiões primárias/secundárias.
- Configuração bootstrap atualizada (`configs/nexus/bootstrap/*.toml`) que reflete os últimos parâmetros de consenso.

## Entornos de vermelho
- Operar dois ambientes Nexus com diferentes configurações de vermelho:
- **Sora Nexus (mainnet)** - prefijo de red de produção `nexus`, hospedando a governança canônica e serviços piggyback de SoraFS/SoraNet (chain ID `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** - prefijo de red de staging `testus`, que especifica a configuração de mainnet para testes de integração e validação de pré-lançamento (chain UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- Manter arquivos de gênese separados, folhas de governo e casas de infraestrutura para cada ambiente. Teste atual como o banco de testes de todos os lançamentos SoraFS/SoraNet antes de promover um Nexus.
- As tubulações de CI/CD devem ser executadas primeiro em Testus, executando testes de fumaça automatizados e exigindo o manual de promoção Nexus uma vez que passam pelas verificações.
- Os pacotes de configuração de referência vivem em `configs/soranexus/nexus/` (mainnet) e `configs/soranexus/testus/` (testnet), cada um com `config.toml`, `genesis.json` e diretórios de admissão Torii de exemplo.

## Paso 1 - Revisão de configuração
1. Auditar a documentação existente:
   - `docs/source/nexus/architecture.md` (consenso, layout de Torii).
   - `docs/source/nexus/deployment_checklist.md` (requisitos de infraestrutura).
   - `docs/source/nexus/governance_keys.md` (procedimentos de custódia de folhas).
2. Validar que os arquivos genesis (`configs/nexus/genesis/*.json`) sejam alinhados com a lista atual de validadores e os pesos de staking.
3. Confirme os parâmetros de vermelho:
   - Tamano de comitê de consenso e quorum.
   - Intervalo de blocos / guarda-chuvas de finalização.
   - Portas de serviço Torii e certificados TLS.

## Paso 2 - Despliegue del cluster bootstrap
1. Aprovisionar nodos validadores:
   - Desinstalar instâncias `irohad` (validadores) com volumes persistentes.
   - Certifique-se de regras de firewall que permitem tráfego de consenso e Torii entre nós.
2. Inicie os serviços Torii (REST/WebSocket) em cada validador com TLS.
3. Desplegar nodos observadores (solo lectura) para resiliência adicional.
4. Executar scripts de bootstrap (`scripts/nexus_bootstrap.sh`) para distribuir genesis, iniciar consenso e registrar nós.
5. Ejecutar testes de fumaça:
   - Enviar transações de teste via Torii (`iroha_cli tx submit`).
   - Verificar produção/finalidade de blocos via telemetria.
   - Revisar replicação do razão entre validadores/observadores.

## Paso 3 - Governança e gestão de ilhas
1. Carregue a configuração multisig do conselho; confirmar que as propostas de governo podem ser enviadas e ratificadas.
2. Armazenar de forma segura as folhas de consenso/comite; configurar backups automáticos com registro de acesso.
3. Configure os procedimentos de rotação das chaves de emergência (`docs/source/nexus/key_rotation.md`) e verifique o runbook.

## Paso 4 - Integração de CI/CD
1. Configurar pipelines:
   - Construção e publicação de imagens do validador/Torii (GitHub Actions ou GitLab CI).
   - Validação automatizada de configuração (lint de gênese, verificação de firmas).
   - Pipelines de desenvolvimento (Helm/Kustomize) para clusters de preparação e produção.
2. Implementar testes de fumaça em CI (levantar cluster efimero, executar suíte canônica de transações).
3. Agregar scripts de reversão para executar falhas e documentar runbooks.## Paso 5 - Observabilidade e alertas
1. Desinstale a pilha de monitor (Prometheus + Grafana + Alertmanager) por região.
2. Métricas centrais recopilares:
  -`nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - Logs via Loki/ELK para serviços Torii e consenso.
3. Painéis:
   - Salud de consenso (altura de bloqueio, finalização, estado de pares).
   - Latência/taxa de erro da API Torii.
   - Transações de governo e estado de propuestas.
4. Alertas:
   - Paro de produção de blocos (>2 intervalos de bloco).
   - Conteúdo de pares por baixo do quórum.
   - Picos na tabela de erro de Torii.
   - Backlog da cola de propostas de governo.

## Paso 6 - Validação e transferência
1. Execução de validação ponta a ponta:
   - Enviar uma proposta de governo (p. ex., mudança de parâmetro).
   - Processar através da aprovação do conselho para garantir que o pipeline de governança funcione.
   - Executar diferenças no estado do razão para garantir a consistência.
2. Documente o runbook para plantão (resposta a incidentes, failover, escalada).
3. Comunicar a disponibilidade aos equipamentos SoraFS/SoraNet; confirme que os despliegues piggyback podem ser colocados nos nós Nexus.

## Checklist de implementação
- [ ] Auditório de gênese/configuração completada.
- [ ] Nodos validadores e observadores desplegados com consenso saludável.
- [ ] Folhas de governo carregadas, proposta provada.
- [ ] Pipelines CI/CD corriendo (construção + implantação + testes de fumaça).
- [ ] Dashboards de observação de ativos com alertas.
- [ ] Documentação de entrega entregue a equipamentos downstream.