---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Runbook de lancement Gateway и DNS SoraFS

Эта копия порта отражает канонический Runbook в
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
Захват надежных операций рабочего потока децентрализованного DNS и шлюза
Afin que les responsables reseau, ops et documentation, puissent repéter la свай
d'automatization avant le lancement 2025-03.

## Portée et livrables

- DNS-серверы (SF-4) и шлюз (SF-5) повторяют определенное определение.
  des hôtes, les Releases du Répertoire de Resolvers, l’automatization TLS/GAR и т. д.
  захват де преув.
- Garder les Entrées du Kickoff (повестка дня, приглашение, отслеживание присутствия, снимок
  телеметрия GAR) синхронизированы с последними проявлениями эмоций владельцев.
- Создание пакета проверяемых артефактов для рецензентов по управлению: примечания
  выпуск репертуара резольверов, журналы шлюза зондов, вылазка упряжи
  соответствие и синтез документов/DevRel.

## Роли и обязанности

| Рабочий поток | Обязанности | Необходимые артефакты |
|------------|------------------|------------------|
| Сетевые TL (куча DNS) | Ведение определенного плана домов, выполнение выпусков репертуара RAD, публикация входных данных телеметрии резольверов. | `artifacts/soradns_directory/<ts>/`, отличия от `docs/source/soradns/deterministic_hosts.md`, метадонные RAD. |
| Руководитель отдела автоматизации операций (шлюз) | Выполните настройки автоматизации TLS/ECH/GAR, lancer `sorafs-gateway-probe`, в течение дня используйте перехватчики PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`, JSON-зонд, входы `ops/drill-log.md`. |
| Рабочая группа по обеспечению качества и инструментарию | Lancer `ci/check_sorafs_gateway_conformance.sh`, лечение приборов, архивирование пакетов с самопроверкой Norito. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Документы / DevRel | Захватите минуты, за несколько дней до прочтения проекта + приложений, опубликуйте синтез данных на портале. | Fichiers `docs/source/sorafs_gateway_dns_design_*.md` в течение дня и примечания к выпуску. |

## Первые блюда и предпосылки

- Определенные спецификации (`docs/source/soradns/deterministic_hosts.md`) и др.
  леса аттестации резольвера (`docs/source/soradns/resolver_attestation_directory.md`).
- Шлюз артефактов: оператор вручную, помощники автоматизации TLS/ECH,
  руководство в прямом режиме, рабочий процесс с самопроверкой `docs/source/sorafs_gateway_*`.
- Инструменты: `cargo xtask soradns-directory-release`,
  И18НИ00000026Х, И18НИ00000027Х,
  `scripts/sorafs_gateway_self_cert.sh` и помощники CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Секреты: ключ выпуска GAR, учетные данные ACME DNS/TLS, ключ маршрутизации PagerDuty,
  token auth Torii для выборки резольверов.

## Контрольный список, предварительный выпуск1. Подтвердите участников и повестку дня в актуальном состоянии
   `docs/source/sorafs_gateway_dns_design_attendance.md` и распространяет повестку дня
   курант (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. Подготовьте раскраски артефактов, которые вам нужны.
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` и др.
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. Подготовьте светильники (демонстрирует GAR, предохраняет RAD, пакеты соответствия шлюза) и т. д.
   s'assurer que l'état `git submodule` соответствует последнему тегу репетиции.
4. Проверка секретов (клиент выпуска Ed25519, учетный документ ACME, токен PagerDuty)
   и переписка с контрольными суммами хранилища.
5. Faire un Smoke Test des Cibles de Telemétrie (конечная точка Pushgateway, плата GAR Grafana)
   аван ле дрель.

## Этапы репетиции автоматизации

### Определённая карта отелей и выпуск репертуара RAD

1. Исполнитель помощника по определению отелей в наборе манифестов
   Предлагаю и подтверждаю отсутствие дрейфа в взаимопонимании
   `docs/source/soradns/deterministic_hosts.md`.
2. Создайте набор репертуара резольверов:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Регистратор идентификатора репертуара, SHA-256 и шаблонов вылазок в нем.
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` и в первые минуты начала матча.

### Захват телеметрии DNS

- Поддержание прозрачности журналов резольверов в течение ≥10 минут в течение ≥10 минут.
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Экспорт метрик Pushgateway и архиватор снимков NDJSON на компьютере.
  репертуар бега ID.

### Упражнения по автоматизации шлюза

1. Исполнитель зонда TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. Комплект ремня безопасности Lancer (`ci/check_sorafs_gateway_conformance.sh`) и др.
   Самостоятельный сертификат помощника (`scripts/sorafs_gateway_self_cert.sh`) для заполнения пакета
   аттестации Norito.
3. Сбор событий PagerDuty/Webhook для проверки цепочки автоматизации
   функция боя в бою.

### Упаковка превью

- Mettre à jour `ops/drill-log.md` с метками времени, участниками и хэшами зондов.
- Складируйте артефакты в репертуарах запуска ID и публикуйте исполнительный синтез.
  в течение нескольких минут Docs/DevRel.
- Поместите пакет предварительных заявок в билет управления перед началом матча.

## Анимация сеанса и отмена превью- **Хронология модератора:**
  - T-24 h — Управление программой после спуска + снимок повестки дня/присутствия в `#nexus-steering`.
  - T-2 h — Сетевой TL создает снимок телеметрии GAR и отправляет изменения в `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 m — автоматизация эксплуатации проверяет подготовку датчиков и регистрирует активный идентификатор запуска в `artifacts/sorafs_gateway_dns/current`.
  - Подвеска l’appel — Le modérateur partage ce runbook et назначенный писец в прямом эфире; Документы/DevRel фиксируют действия в реальном времени.
- **Шаблон протокола:** Копирование squelette de
  `docs/source/sorafs_gateway_dns_design_minutes.md` (également miroir в комплекте
  du portail) и зафиксируйте экземпляр, повторяющий сеанс. Включить список
  участники, решения, действия, препятствия и риски выхода.
- **Загрузка превью:** Zipper le répertoire `runbook_bundle/` du rehearsal,
  объединение PDF-файлов в течение нескольких минут, регистрация хэшей SHA-256 в течение нескольких минут +
  повестка дня, вы можете нажать на псевдонимы рецензентов управления, чтобы загрузить файлы
  доступны в `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## Snapshot des preuves (начало с Марса 2025 г.)

Репетиция Les derniers Artefacts/живые референсы в рамках дорожной карты и нескольких минут
запасы в ведре `s3://sora-governance/sorafs/gateway_dns/`. Ле хэши
ci-dessous reflètent le манифест канонический (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).

- ** Пробный прогон — 2 марта 2025 г. (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Архив пакета: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - PDF-файл с протоколами: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Ателье в прямом эфире — 03.03.2025 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Загрузить с вниманием: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel добавляет SHA-256 из того PDF-файла, который будет передан в пакете.)_

## Материальное соединение

- [Шлюз Playbook d’operations] (./operations-playbook.md)
- [План наблюдения SoraFS](./observability-plan.md)
- [Децентрализованный DNS-шлюз и трекер] (https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)