---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS Отправьте запрос DNS

یہ پورٹل کاپی کینونیکل رن بک کو منعکس کرتی ہے جو
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md) میں ہے۔
Децентрализованный DNS и шлюз
В календаре 2025–2003 гг., который будет готов к выпуску в 2025–03 гг. اسٹیک کی
مشقیق کر سکیں۔

## اسکوپ اور ڈلیوریبلز

- Основные этапы DNS (SF-4) и шлюза (SF-5) для детерминированного создания хоста.
  выпуск каталогов резольвера, автоматизация TLS/GAR, сбор доказательств и многое другое.
- Доступ к данным (повестка дня, приглашение, трекер посещаемости, снимок телеметрии GAR).
  Дополнительные назначения владельца
- Чтобы создать пакет артефактов, выберите каталог резольвера.
  примечания к выпуску, журналы проверки шлюза, выходные данные жгута соответствия, сводка документов/DevRel.

## رولز اور ذمہ داریاں

| ورک اسٹریم | ذمہ داریاں | مطلوبہ артефакты |
|------------|------------|------------------|
| Сетевой TL (стек DNS) | детерминированный план хоста Доступ к выпускам каталогов RAD Входы телеметрии сопоставителя | `artifacts/soradns_directory/<ts>/`, `docs/source/soradns/deterministic_hosts.md` — различия, метаданные RAD. |
| Руководитель отдела автоматизации операций (шлюз) | Инструменты автоматизации TLS/ECH/GAR `sorafs-gateway-probe` и перехватчики PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`, проверка JSON, записи `ops/drill-log.md`. |
| Рабочая группа по обеспечению качества и инструментарию | `ci/check_sorafs_gateway_conformance.sh` Дополнительные светильники Norito Пакеты для самостоятельной сертификации. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Документы / DevRel | минут ریکارڈ کرنا، предварительное чтение дизайна + приложения اپڈیٹ کرنا، اور اسی پورٹل میں резюме доказательств شائع کرنا۔ | Ниже приведены примечания к выпуску `docs/source/sorafs_gateway_dns_design_*.md`. |

## ان پٹس اور پری ریکوائرمنٹس

- детерминированная спецификация хоста (`docs/source/soradns/deterministic_hosts.md`) для преобразователя
  леса аттестации (`docs/source/soradns/resolver_attestation_directory.md`).
- артефакты шлюза: руководство оператора, помощники по автоматизации TLS/ECH, руководство в прямом режиме,
  Рабочий процесс самосертификации جو `docs/source/sorafs_gateway_*` کے تحت ہے۔
- Оснастка: `cargo xtask soradns-directory-release`,
  И18НИ00000026Х, И18НИ00000027Х,
  `scripts/sorafs_gateway_self_cert.sh`, помощники CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Секреты: ключ выпуска GAR, учетные данные DNS/TLS ACME, ключ маршрутизации PagerDuty,
  Резолвер извлекает токен аутентификации Torii.

## پری فلائٹ چیک لسٹ

1. `docs/source/sorafs_gateway_dns_design_attendance.md` اپڈیٹ کر کے شرکاء اور повестка дня
   Выберите повестку дня (`docs/source/sorafs_gateway_dns_design_agenda.md`) شیئر کریں۔
2. `artifacts/sorafs_gateway_dns/<YYYYMMDD>/`
   `artifacts/soradns_directory/<YYYYMMDD>/` جیسے корни артефактов تیار کریں۔
3. приспособления (манифесты GAR, доказательства RAD, пакеты соответствия шлюза).
   یقینی بنائیں کہ `git submodule` کی حالت تازہ ترین тег репетиции سے میچ کرتی ہے۔
4. секреты (ключ выпуска Ed25519, файл учетной записи ACME, токен PagerDuty).
   Проверка контрольных сумм хранилища
5. Уточнение объектов телеметрии (конечная точка Pushgateway, плата GAR Grafana) и дымовое тестирование.

## آٹومیشن этапы репетиции

### детерминированная карта хостов в выпуске каталога RAD1. Детерминированный помощник по созданию детерминированного хоста.
   تصدیق کریں کہ `docs/source/soradns/deterministic_hosts.md` کے مقابلے میں کوئی drift نہیں۔
2. Пакет каталогов резольвера.

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Укажите идентификатор каталога, SHA-256 и пути вывода.
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` Начальные минуты

### Захват телеметрии DNS

- Журналы прозрачности резольвера ≥10 минут или хвост:
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Экспорт метрик Pushgateway и снимки NDJSON, а также запуск каталога идентификаторов и возможность экспорта метрик Pushgateway.

### Учения по автоматизации шлюзов

1. Проверка TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. Ремень соответствия (`ci/check_sorafs_gateway_conformance.sh`) или помощник по самостоятельной сертификации
   (`scripts/sorafs_gateway_self_cert.sh`) Дополнительная информация Norito Пакет аттестации ریفریش ہو۔
3. События PagerDuty/Webhook фиксируют сквозную и сквозную обработку данных.

### Упаковка доказательств

- `ops/drill-log.md` — временные метки, хэши зондирования участников и другие.
- запускать каталоги идентификаторов и артефакты, а также минуты работы с документами/DevRel и краткое изложение результатов.
- начальный обзор سے پہلے билет на управление میں комплект доказательств لنک کریں۔

## سیشن فیسلیٹیشن اور передача доказательств

- **Время модератора:**
  - T-24 h — Управление программой `#nexus-steering` — напоминание + снимок повестки дня/посещаемости پوسٹ کرے۔
  - T-2 h — Снимок телеметрии сети TL GAR
  - Т-15 м — Готовность зонда автоматизации эксплуатации.
  - کال کے دوران — Модератор یہ رن بک شیئر کرے اور live писец назначить کرے؛ Встроенные действия Docs/DevRel
- **Шаблон протокола:**
  `docs/source/sorafs_gateway_dns_design_minutes.md` Скелет для создания (пакет порталов для создания) Для завершения создания экземпляра фиксации کریں۔ Список участников, решения, действия, хэши доказательств, невыполненные риски и т. д.
- **Загрузка доказательств:** репетиция `runbook_bundle/`, каталог, архивированный архив минут, прикреплённый PDF-файл, минуты + повестка дня, хэши SHA-256, возможность загрузки данных. псевдоним рецензента по управлению کو ping کریں جب فائلز `s3://sora-governance/sorafs/gateway_dns/<date>/` میں پہنچ جائیں۔

## Снимок доказательств (начало в марте 2025 г.)

дорожная карта اور минуты میں حوالہ دیے گئے تازہ ترین репетиция/живые артефакты
`s3://sora-governance/sorafs/gateway_dns/` ведро میں ہیں۔ نیچے گئے хеши
канонический манифест (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`) کو отражает کرتے ہیں۔

- ** Пробный прогон — 2 марта 2025 г. (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Архив пакета: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - Протокол PDF: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Живой семинар — 03.03.2025 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Ожидает загрузки: `gateway_dns_minutes_20250303.pdf` — Docs/DevRel PDF с SHA-256 для проверки)_

## Сопутствующий материал

- [Сборник операций шлюза] (./operations-playbook.md)
- [SoraFS план наблюдения](./observability-plan.md)
- [Децентрализованный трекер DNS и шлюза] (https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)