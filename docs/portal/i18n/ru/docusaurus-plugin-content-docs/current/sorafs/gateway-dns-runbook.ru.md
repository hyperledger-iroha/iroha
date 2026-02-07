---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Ранбук запуска шлюза и DNS SoraFS

Эта копия на портале отражает канонический ранбук в
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
Она фиксирует операционные ограждения для работы децентрализованного DNS и шлюза,
чтобы узнать о сети, эксплуатации и документации, можно было отрепетировать стек
автоматизация перед началом сезона 2025-03.

## Область и результаты

- Связать вехи DNS (SF-4) и шлюз (SF-5) через повтор детерминированную.
  деривации хостов, релизов каталога резольверов, автоматизации TLS/GAR и
  собрать доказательства.
- Держать стартовые артефакты (повестка дня, приглашение, трекер посещаемости, снимок)
  телеметрии ГАР) синхронизированными с последними назначениями владельцев.
- Подготовить аудируемый пакет документов для рецензентов корпоративного управления: примечания к выпуску.
  каталог резольверов, логи шлюзовых зондов, вывод жгутов соответствия и сводку
  Документы/DevRel.

## Роли и ответственность

| Рабочий поток | Ответственность | Требуемые документы |
|------------|-----------------|---------------------|
| Сетевой TL (стек DNS) | Поддерживать определенный план хостов, запускать выпуски каталогов RAD, публиковать входы преобразователей телеметрии. | `artifacts/soradns_directory/<ts>/`, различия для `docs/source/soradns/deterministic_hosts.md`, метаданные RAD. |
| Руководитель отдела автоматизации операций (шлюз) | Выполнять тренировку автоматизации TLS/ECH/GAR, запускать `sorafs-gateway-probe`, обновлять хуки PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`, зондирование JSON, записи в `ops/drill-log.md`. |
| Рабочая группа по обеспечению качества и инструментарию | Запуск `ci/check_sorafs_gateway_conformance.sh`, курировать светильники, архивировать Norito комплекты самопроверки. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Документы / DevRel | Фиксировать протоколы, обновлять предварительно прочитанные дизайны + приложения, публиковать сводку доказательств на портале. | Обновленные `docs/source/sorafs_gateway_dns_design_*.md` и примечания к выпуску. |

## Входы и предварительные условия

- Спецификации определенных хостов (`docs/source/soradns/deterministic_hosts.md`) и
  аттестация каркаса резольверов (`docs/source/soradns/resolver_attestation_directory.md`).
- Артефакты шлюза: руководство оператора, помощники по автоматизации TLS/ECH,
  руководство по прямому режиму и рабочему процессу самопроверки под `docs/source/sorafs_gateway_*`.
- Оснастка: `cargo xtask soradns-directory-release`,
  И18НИ00000026Х, И18НИ00000027Х,
  `scripts/sorafs_gateway_self_cert.sh` и помощники CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Секреты: ключ релиза GAR, учетные данные DNS/TLS ACME, ключ маршрутизации PagerDuty,
  Токен аутентификации Torii для получения резольверов.

## Предполетный контрольный список

1. Подтвердите участников и повестку дня, обновив
   `docs/source/sorafs_gateway_dns_design_attendance.md` и разослав текущую
   повестка дня (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. Подготовьте исходные документы, например.
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` и
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. Обновите приспособления (манифесты GAR, доказательства RAD, шлюз соответствия пакетов) и
   Убедитесь, что состояние `git submodule` соответствует последнему тегу репетиции.
4. Проверьте секреты (ключ выпуска Ed25519, файл учетной записи ACME, токен PagerDuty) и
   соответствие контрольных сумм в хранилище.
5. Цели телеметрии для дымового тестирования (конечная точка Pushgateway, плата GAR Grafana)
   перед дрелью.

## Шаги повторения автоматизации

### Детерминированная карта хостов и выпуск каталога RAD1. Запустите помощник определения деривации хостов на предложенном наборе.
   заявляет и подтверждает отсутствие дрейфа относительно
   `docs/source/soradns/deterministic_hosts.md`.
2. Сгенерируйте пакетный каталог резольверов:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Зафиксируйте напечатанный идентификационный каталог, SHA-256 и выходные дни внутри.
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` и начало игры на минуте.

### Захват телеметрии DNS

- Журналы прозрачности хвостового преобразователя в течение ≥10 минут с
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Экспортируйте метрики Pushgateway и архивируйте снимки NDJSON рядом с
  каталогами запуска ID.

### Шлюз автоматизации Drills

1. Выполните проверку TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. Запустите соответствующий жгут (`ci/check_sorafs_gateway_conformance.sh`) и
   помощник по самостоятельной сертификации (`scripts/sorafs_gateway_self_cert.sh`) для обновлений
   Пакет аттестации Norito.
3. Зафиксируйте события PagerDuty/Webhook, чтобы обеспечить сквозную работу.
   путь автоматизации.

### Упаковка доказательств

- Обновите `ops/drill-log.md` с метками времени, участниками и хеш-зондами.
- Сохраните документы в каталогах, удостоверяющие личность, и опубликуйте резюме.
  в минутах Документы/DevRel.
- Сошлитесь на пакет доказательств в управленческом билете до начала рассмотрения.

## Модерация сессии и доказательство передачи

- **Временная шкала модератора:**
  - T-24 h — Управление программой публикует напоминание + снимок повестки дня/посещаемости в `#nexus-steering`.
  - T-2 h — Networking TL обновляет снимки телеметрии GAR и фиксирует отклонения в `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - Т-15 м — Ops Automation теперь проверяет и записывает активный идентификатор запуска в `artifacts/sorafs_gateway_dns/current`.
  - Во время звонка — Модератор этим ранбуком и подключите живого писца; Документы/DevRel фиксируют действия на ходу.
- **Шаблон минут:** Скопируйте скелеты из
  `docs/source/sorafs_gateway_dns_design_minutes.md` (также отражение в пакете портала)
  и коммитируйте заполненный экземпляр на каждой сессии. Включите список участников,
  решения, действия, хеш-доказательства и открытые риски.
- **Загрузка доказательств:** Заархивируйте `runbook_bundle/` из репетиции,
  приложите отрендеренный PDF-минуту, запишите SHA-256 хешей в минуты + повестку дня,
  затем сообщите псевдоним проверяющего управления после загрузки в
  `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## Снимок доказательства (начало марта 2025 г.)

Последние репетиции/живые артефакты, упомянутые в дорожной карте и протоколах, оставлены в ведре
`s3://sora-governance/sorafs/gateway_dns/`. Хэши ниже отражает канонический
манифест (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).

- ** Пробный прогон — 2 марта 2025 г. (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Пакет Tarball: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - Протокол PDF: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Живой семинар — 03.03.2025 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Ожидает загрузки: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel добавить SHA-256 после получения PDF в комплекте.)_

## Связанные материалы

- [Справочник операций для шлюза] (./operations-playbook.md)
- [План наблюдения SoraFS](./observability-plan.md)
- [Трекер децентрализованного DNS и шлюза](https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)