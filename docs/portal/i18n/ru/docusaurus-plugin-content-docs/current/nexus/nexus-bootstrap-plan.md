---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-bootstrap-plan
title: Bootstrap и наблюдаемость Sora Nexus
description: Операционный план вывода базового кластера валидаторов Nexus перед подключением сервисов SoraFS и SoraNet.
---

:::note Канонический источник
Эта страница отражает `docs/source/soranexus_bootstrap_plan.md`. Держите обе копии синхронизированными, пока локализованные версии не попадут в портал.
:::

# План Bootstrap и Observability Sora Nexus

## Цели
- Развернуть базовую сеть валидаторов/наблюдателей Sora Nexus с governance keys, Torii API и мониторингом консенсуса.
- Проверить ключевые сервисы (Torii, consensus, persistence) до включения piggyback деплоев SoraFS/SoraNet.
- Настроить CI/CD workflows и observability dashboards/alerts для здоровья сети.

## Предпосылки
- Материал governance keys (multisig совета, committee keys) доступен в HSM или Vault.
- Базовая инфраструктура (Kubernetes кластеры или bare-metal узлы) в primary/secondary регионах.
- Обновленная bootstrap конфигурация (`configs/nexus/bootstrap/*.toml`), отражающая актуальные параметры консенсуса.

## Сетевые окружения
- Эксплуатируйте два окружения Nexus с разными сетевыми префиксами:
- **Sora Nexus (mainnet)** - префикс продакшн сети `nexus`, размещает каноническое управление и piggyback сервисы SoraFS/SoraNet (chain ID `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`).
- **Sora Testus (testnet)** - префикс staging сети `testus`, зеркалирует mainnet конфигурацию для интеграционных тестов и pre-release валидации (chain UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`).
- Держите отдельные genesis файлы, governance keys и инфраструктурные footprints для каждого окружения. Testus служит полигоном для всех rollouts SoraFS/SoraNet перед продвижением в Nexus.
- CI/CD pipelines должны сначала деплоить в Testus, выполнять автоматические smoke tests и требовать ручной промоушн в Nexus после прохождения проверок.
- Референсные конфигурационные bundles лежат в `configs/soranexus/nexus/` (mainnet) и `configs/soranexus/testus/` (testnet), каждая содержит образцы `config.toml`, `genesis.json` и каталоги admission Torii.

## Шаг 1 - Ревью конфигурации
1. Аудировать существующую документацию:
   - `docs/source/nexus/architecture.md` (consensus, layout Torii).
   - `docs/source/nexus/deployment_checklist.md` (infra requirements).
   - `docs/source/nexus/governance_keys.md` (процедуры хранения ключей).
2. Проверить, что genesis файлы (`configs/nexus/genesis/*.json`) соответствуют текущему roster валидаторов и staking weights.
3. Подтвердить параметры сети:
   - Размер консенсусного комитета и quorum.
   - Интервал блоков / пороги finality.
   - Порты Torii сервисов и TLS сертификаты.

## Шаг 2 - Деплой bootstrap кластера
1. Подготовить валидаторские узлы:
   - Развернуть `irohad` инстансы (validators) с persistent volumes.
   - Убедиться, что firewall rules разрешают consensus & Torii трафик между узлами.
2. Запустить Torii сервисы (REST/WebSocket) на каждом валидаторе с TLS.
3. Развернуть observer узлы (read-only) для дополнительной устойчивости.
4. Запустить bootstrap скрипты (`scripts/nexus_bootstrap.sh`) для распространения genesis, старта консенсуса и регистрации узлов.
5. Выполнить smoke tests:
   - Отправить тестовые транзакции через Torii (`iroha_cli tx submit`).
   - Проверить production/finality блоков через телеметрию.
   - Проверить репликацию ledger между валидаторами/observer.

## Шаг 3 - Governance и управление ключами
1. Загрузить multisig конфигурацию совета; подтвердить, что governance предложения можно отправлять и ратифицировать.
2. Безопасно хранить consensus/committee keys; настроить автоматические бэкапы с access logging.
3. Настроить процедуры аварийной ротации ключей (`docs/source/nexus/key_rotation.md`) и проверить runbook.

## Шаг 4 - Интеграция CI/CD
1. Настроить pipelines:
   - Build и публикация validator/Torii images (GitHub Actions или GitLab CI).
   - Автоматическая валидация конфигурации (lint genesis, verify signatures).
   - Deployment pipelines (Helm/Kustomize) для staging и production кластеров.
2. Внедрить smoke tests в CI (поднять ephemeral cluster и запустить каноническую транзакционную suite).
3. Добавить rollback скрипты для неудачных деплоев и задокументировать runbooks.

## Шаг 5 - Observability и алерты
1. Развернуть мониторинговый стек (Prometheus + Grafana + Alertmanager) по регионам.
2. Собирать ключевые метрики:
  - `nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - Логи через Loki/ELK для Torii и consensus сервисов.
3. Дашборды:
   - Здоровье консенсуса (высота блоков, finality, статус peers).
   - Латентность/ошибки Torii API.
   - Governance транзакции и статусы предложений.
4. Алерты:
   - Остановка производства блоков (>2 block intervals).
   - Падение peer count ниже quorum.
   - Спайки ошибок Torii.
   - Backlog очереди governance предложений.

## Шаг 6 - Валидация и handoff
1. Запустить end-to-end валидацию:
   - Отправить governance proposal (например изменение параметра).
   - Пропустить через одобрение совета, чтобы убедиться, что pipeline governance работает.
   - Запустить ledger state diff для проверки консистентности.
2. Документировать runbook для on-call (incident response, failover, scaling).
3. Сообщить о готовности командам SoraFS/SoraNet; подтвердить, что piggyback деплои могут указывать на Nexus узлы.

## Чеклист реализации
- [ ] Аудит genesis/configuration завершен.
- [ ] Валидаторские и observer узлы развернуты с здоровым консенсусом.
- [ ] Governance keys загружены, proposal протестирован.
- [ ] CI/CD pipelines работают (build + deploy + smoke tests).
- [ ] Observability dashboards активны с алертами.
- [ ] Документация handoff передана downstream командам.
