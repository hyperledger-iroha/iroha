---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano nexus-bootstrap
título: Bootstrap e Sora Nexus
description: Plano de operação para o cluster de validação Nexus antes do serviço SoraFS e SoraNet.
---

:::nota História Canônica
Esta página é `docs/source/soranexus_bootstrap_plan.md`. Selecione uma cópia sincronizada, caso a versão local não seja exibida no portal.
:::

# Planeje Bootstrap e Observabilidade Sora Nexus

##Céli
- Развернуть базовую сеть валидаторов/наблюдателей Sora Nexus com chaves de governança, Torii API e monitoramento consciência.
- Проверить ключевые сервисы (Torii, consenso, persistência) para obter piggyback деплоев SoraFS/SoraNet.
- Gere fluxos de trabalho de CI/CD e painéis/alertas de observabilidade para a configuração.

## Por favor
- Chaves de governança materiais (multisig совета, chaves de comitê) fornecidas no HSM ou no Vault.
- Infraestrutura básica (cristalinos Kubernetes ou usos bare-metal) na região primária/secundária.
- Configuração de configuração de bootstrap (`configs/nexus/bootstrap/*.toml`), alterando a configuração de parâmetros atuais.

## Сетевые окружения
- Exclua a configuração Nexus com as seguintes configurações:
- **Sora Nexus (mainnet)** - Conjunto de produção profissional `nexus`, размещает каноническое управление и piggyback сервисы SoraFS/SoraNet (ID de cadeia `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** - site de teste profissional `testus`, configure a configuração da mainnet para testes de integração e validação de pré-lançamento (cadeia UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- Держите отдельные genesis файлы, chaves de governança e инфраструктурные pegadas para каждого окружения. Testus oferece uma política para todos os seus lançamentos SoraFS/SoraNet antes de serem implementados em Nexus.
- Pipelines CI/CD são implementados no Testus, testes de fumaça automatizados e testes de fumaça promovidos em Nexus после прохождения проверок.
- Pacotes de configuração de configuração regulares colocados em `configs/soranexus/nexus/` (mainnet) e `configs/soranexus/testus/` (testnet), каждая содержит образцы `config.toml`, `genesis.json` e catálogos de admissão Torii.

## Passo 1 - Configurações de configuração
1. Аудировать существующую документацию:
   - `docs/source/nexus/architecture.md` (consenso, layout Torii).
   - `docs/source/nexus/deployment_checklist.md` (requisitos de infra-estrutura).
   - `docs/source/nexus/governance_keys.md` (classe de configuração do fabricante).
2. Verifique se o genesis файлы (`configs/nexus/genesis/*.json`) fornece a validação da lista e o piquetamento de pesos.
3. Selecione a configuração dos parâmetros:
   - Размер консенсусного комитета e quórum.
   - Интервал блоков / пороги finalidade.
   - Portas Torii serviços e certificados TLS.

## Passo 2 - Desenvolvendo o cluster de bootstrap
1. Para validar o uso:
   - Развернуть `irohad` инстансы (validadores) com volumes persistentes.
   - Убедиться, что firewall regras разрешают consenso & Torii трафик между узлами.
2. Abra os serviços Torii (REST/WebSocket) para validá-los com TLS.
3. Use o observador (somente leitura) para usar o utilitário.
4. Abra os scripts de bootstrap (`scripts/nexus_bootstrap.sh`) para a expansão do genesis, início da consciência e registro.
5. Testes de fumaça de verificação:
   - Abra o teste de transação do Torii (`iroha_cli tx submit`).
   - Проверить produção/finalidade блоков через телеметрию.
   - Проверить репликацию ledger между валидаторами/observer.

## Capítulo 3 - Governança e atualização de classe
1. Config multisig конфигурацию совета; подтвердить, что governança предложения можно отправлять и ратифицировать.
2. Chaves de consenso/comitê Безопасно хранить; настроить автоматические бэкапы com registro de acesso.
3. Insira o programa de rotatividade padrão (`docs/source/nexus/key_rotation.md`) e forneça o runbook.

## Passo 4 - Integração CI/CD
1. Pipelines de construção:
   - Construir e publicar imagens validadoras/Torii (GitHub Actions ou GitLab CI).
   - Автоматическая валидация конфигурации (gênese de fiapos, verificação de assinaturas).
   - Pipelines de implantação (Helm/Kustomize) para grupos de preparação e produção.
2. Faça testes de fumaça no CI (conjunto de cluster efêmero e suíte de transação canônica).
3. Crie scripts de reversão para novos aplicativos e execute runbooks.## Passo 5 - Observabilidade e alertas
1. Selecione a seção de monitoramento (Prometheus + Grafana + Alertmanager) por região.
2. Definindo métricas de classe:
  -`nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - Логи через Loki/ELK para Torii e serviços de consenso.
3. Dашборды:
   - Здоровье консенсуса (высота блоков, finalidade, статус peers).
   - API Torii.
   - Governança, transações e status.
4. Alertas:
   - Остановка производства блоков (>2 intervalos de bloco).
   - A contagem de pares não é o quorum.
   - Espaçador Torii.
   - Backlog очереди governança предложений.

## Capítulo 6 - Validação e transferência
1. Verifique a validação de ponta a ponta:
   - Отправить proposta de governação (например изменение parâmetro).
   - Пропустить через одобрение совета, чтобы убедиться, что pipeline governança работает.
   - Verifique a diferença de estado do razão para a consistência de verificação.
2. Runbook de documentação para plantão (resposta a incidentes, failover, escalonamento).
3. Selecione o comando SoraFS/SoraNet; Portanto, esse conjunto de piggyback pode ser usado em Nexus.

## Verifique as realizações
- [ ] Аудит genesis/configuration завершен.
- [ ] Валидаторские и observer узлы развернуты с здоровым консенсусом.
- [] Chaves de governança загружены, proposta протестирован.
- [ ] Pipelines CI/CD работают (construir + implantar + testes de fumaça).
- [] Painéis de observabilidade ativos com alertas.
- [ ] Документация handoff передана downstream командам.