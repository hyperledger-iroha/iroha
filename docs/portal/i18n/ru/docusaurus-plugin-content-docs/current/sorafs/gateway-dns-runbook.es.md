---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Начальная книга запуска шлюза и DNS SoraFS

Эта копия портала отображает канонический журнал Runbook в
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
Узнайте, какие резервы работают в рабочем потоке DNS и децентрализованных шлюзах
для того, чтобы руководить сетевыми связями, работать и документировать, можно
автоматизация перед началом сезона 2025–03.

## Альканс и Entregables

- Выполните вывод DNS-адресов (SF-4) и шлюза (SF-5).
  определение хостов, освобождение директорий резольверов, автоматизация TLS/GAR
  и захват доказательств.
- Mantener los insumos del start (повестка дня, приглашение, отслеживание помощи, снимок)
  телеметрии GAR) синхронизированы с последними назначенными владельцами.
- Создание пакета проверяемых артефактов для пересмотра государственного управления: примечания к
  выпуск директории резольверов, журналы сигналов шлюза, удаление жгута
  согласование и возобновление документации/DevRel.

## Роли и обязанности

| Рабочий поток | Ответственность | Требуемые артефакты |
|------------|-------------------|-----------------------|
| Сетевой TL (стек DNS) | Управляйте планом определения хостов, выдавайте релизы директории RAD, публикуйте входные данные телеметрии резольверов. | `artifacts/soradns_directory/<ts>/`, отличия от `docs/source/soradns/deterministic_hosts.md`, метаданные RAD. |
| Руководитель отдела автоматизации операций (шлюз) | Выполните настройки автоматизации TLS/ECH/GAR, исправьте `sorafs-gateway-probe`, активизируйте перехватчики PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`, JSON-зонд, вход в `ops/drill-log.md`. |
| Рабочая группа по обеспечению качества и инструментарию | Ejecutar `ci/check_sorafs_gateway_conformance.sh`, светильники Curar, пакеты архивов с самосертификатом Norito. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Документы / DevRel | Зарегистрируйтесь за несколько минут, актуализируйте предварительно прочитанное изображение + приложения и опубликуйте резюме доказательств на этом портале. | Обновлены архивы `docs/source/sorafs_gateway_dns_design_*.md` и примечания к развертыванию. |

## Вход и предварительные условия

- Спецификация детерминированных хостов (`docs/source/soradns/deterministic_hosts.md`) y
  Андамская аттестация резольверов (`docs/source/soradns/resolver_attestation_directory.md`).
- Артефакты шлюза: руководство оператора, помощники по автоматизации TLS/ECH,
  руководство по прямому режиму и режим самодиагностики `docs/source/sorafs_gateway_*`.
- Оснастка: `cargo xtask soradns-directory-release`,
  И18НИ00000026Х, И18НИ00000027Х,
  `scripts/sorafs_gateway_self_cert.sh`, помощники CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Секреты: ключ выпуска GAR, учетные данные ACME DNS/TLS, ключ маршрутизации PagerDuty,
  токен аутентификации Torii для получения преобразователей.

## Контрольный список перед vuelo1. Подтвердите помощь и актуализацию повестки дня
   `docs/source/sorafs_gateway_dns_design_attendance.md` и циркулирует в повестке дня
   vigente (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. Подготовка предметов в виде артефактов.
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` у
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. Светильники Refresca (декларации GAR, Pruebas RAD, пакеты соответствия шлюза) и
   убедитесь, что состояние `git submodule` совпадает с последним тегом ensayo.
4. Секретная проверка (клавиатура выпуска Ed25519, архив файла ACME, токен PagerDuty)
   и это совпадение с контрольными суммами хранилища.
5. Проведите дымовую проверку целей телеметрии (конечная точка Pushgateway, таблица GAR Grafana)
   перед тренировкой.

## Шаги автоматизации

### Определение хостов и выпуск каталога RAD

1. Вызов помощника по определению хостов против набора манифестов
   Пропуесто и подтверждение того, что нет никакого дрейфа, уважение
   `docs/source/soradns/deterministic_hosts.md`.
2. Общий пакет каталогов резольверов:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Зарегистрируйте идентификатор каталога, SHA-256 и маршруты Salida Impresas Dentro de.
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` и через несколько минут после начала матча.

### Захват телеметрии DNS

- Хранение журналов прозрачности резольверов в течение ≥10 минут при использовании
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Экспорт показателей Pushgateway и архивирование снимков NDJSON вместе со всеми
  ID директории запуска.

### Упражнения по автоматизации шлюза

1. Выброс сигнала TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. Снимите соответствующий ремень безопасности (`ci/check_sorafs_gateway_conformance.sh`) y
   Помощник по самодиагностике (`scripts/sorafs_gateway_self_cert.sh`) для обновления
   комплект свидетельств Norito.
3. Запишите события PagerDuty/Webhook для демонстрации автоматизации.
   экстремальная функция — экстремальная.

### Упаковка доказательств

- Actualiza `ops/drill-log.md` с метками времени, участниками и хэшами зондов.
- Охраняйте артефакты в каталогах запуска ID и публикуйте возобновленные выпуски.
  в течение нескольких минут Документов/DevRel.
- Возьмите пакет доказательств и билет губернатора до пересмотра.
  дель начало.

## Содействие проведению заседаний и сбору доказательств- **Временная линия модератора:**
  - T-24 h — Публикация управления программой + снимок повестки дня/помощи в `#nexus-steering`.
  - T-2 h — Сетевой TL обновляет снимок телеметрии GAR и регистрирует изменения в `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 m — Ops Automation проверяет подготовку датчиков и записывает активный идентификатор запуска в `artifacts/sorafs_gateway_dns/current`.
  - Durante la lamada — El moderator comparte este runbook y asigna un escriba en vivo; Документы/DevRel собирают элементы онлайн-действий.
- **Plantilla de minutas:** Копия el esqueleto de
  `docs/source/sorafs_gateway_dns_design_minutes.md` (также доступен в комплекте
  дель портала) и комитет по созданию завершенного экземпляра для сессии. Включите список
  помощники, решения, элементы действий, хэши доказательств и ожидаемые риски.
- **Доказательства:** Найдите директорию `runbook_bundle/`,
  дополнение к минутному рендерингу PDF, регистрация хэшей SHA-256 в течение нескольких минут +
  повестка дня и уведомление под псевдонимом рецензентов губернатора, когда он до лас каргас
  aterricen en `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## Снимок доказательств (начало марта 2025 г.)

Последние артефакты искусства/производства, ссылки на дорожную карту и минуты
вивен в ведре `s3://sora-governance/sorafs/gateway_dns/`. Лос-хеши Абахо
Отобразить канонический манифест (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).

- ** Пробный прогон — 2 марта 2025 г. (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Тарбол пакета: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - PDF-файл с минутами: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Семинар en vivo — 03.03.2025 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Отложенная загрузка: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel добавляет SHA-256 при рендеринге PDF в пакете.)_

## Связи с материалами

- [Книга операций шлюза] (./operations-playbook.md)
- [План наблюдения SoraFS](./observability-plan.md)
- [Десцентрализованный трекер DNS и шлюз] (https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)