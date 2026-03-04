---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/gateway-dns-runbook.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Начальная книга запуска шлюза и DNS da SoraFS

Эста копия на портале или в runbook canonico em
[`docs/source/sorafs_gateway_dns_design_runbook.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_gateway_dns_design_runbook.md).
Захват рабочих ограждений децентрализованного рабочего потока DNS и шлюза
для руководства по работе в сети, работы и документации, которые могут быть вам предоставлены,
автоматизация перед началом сезона 2025–03.

## Эскопо и Entregaveis

- Полученное соединение между метками DNS (SF-4) и шлюзом (SF-5)
  детерминированность хостов, освобождение директорий резольверов, автоматизация TLS/GAR
  и захват доказательств.
- Manter os insumos do startoff (повестка дня, совещание, отслеживание присутствия, снимок
  телеметрия GAR) синхронизированы с последними данными владельцев.
- Создание пакета аудита артефактов для пересмотра управления: примечания к
  освободить директорию резольверов, журналы зонда шлюза, сказать, чтобы использовать
  соответствие и резюме Docs/DevRel.

## Папеис и ответственность

| Рабочий поток | Ответственность | Требуемые артефакты |
|------------|-------------------|----------------------|
| Сетевой TL (стек DNS) | Работая над детерминированным планом хостов, выполните выпуски директории RAD, опубликуйте входные данные телеметрии резольверов. | `artifacts/soradns_directory/<ts>/`, отличия от `docs/source/soradns/deterministic_hosts.md`, метаданные RAD. |
| Руководитель отдела автоматизации операций (шлюз) | Выполните настройки автоматизации TLS/ECH/GAR, радар `sorafs-gateway-probe`, настройте крючки для PagerDuty. | `artifacts/sorafs_gateway_probe/<ts>/`, зонд JSON, ввод `ops/drill-log.md`. |
| Рабочая группа по обеспечению качества и инструментарию | Rodar `ci/check_sorafs_gateway_conformance.sh`, светильники Curar, комплекты архиваторов с самосертификацией Norito. | `artifacts/sorafs_gateway_conformance/<ts>/`, `artifacts/sorafs_gateway_attest/<ts>/`. |
| Документы / DevRel | Запишите несколько минут, ознакомьтесь или предварительно прочитайте дизайн + приложения, а также опубликуйте или опубликуйте резюме доказательств на ближайшем портале. | Архивы `docs/source/sorafs_gateway_dns_design_*.md` активированы и примечания к развертыванию. |

## Вход и предварительные требования

- Определенные хосты (`docs/source/soradns/deterministic_hosts.md`) и
  o подмости для проверки резольверов (`docs/source/soradns/resolver_attestation_directory.md`).
- Артефакты шлюза: руководство оператора, помощники по автоматизации TLS/ECH,
  руководство по прямому режиму и рабочему процессу самопроверки `docs/source/sorafs_gateway_*`.
- Оснастка: `cargo xtask soradns-directory-release`,
  И18НИ00000026Х, И18НИ00000027Х,
  `scripts/sorafs_gateway_self_cert.sh`, и помощники CI
  (`ci/check_sorafs_gateway_conformance.sh`, `ci/check_sorafs_gateway_probe.sh`).
- Разделы: вызов GAR, учетные данные ACME DNS/TLS, ключ маршрутизации для PagerDuty,
  токен аутентификации Torii для получения резольверов.

## Контрольный список перед полетом

1. Подтвердите участников и актуальную повестку дня.
   `docs/source/sorafs_gateway_dns_design_attendance.md` и циркулирует в повестке дня
   настоящий (`docs/source/sorafs_gateway_dns_design_agenda.md`).
2. Приготовьте рис де Артефатос в обычном виде.
   `artifacts/sorafs_gateway_dns/<YYYYMMDD>/` е
   `artifacts/soradns_directory/<YYYYMMDD>/`.
3. Настройка оборудования (декларации GAR, подтверждения RAD, пакеты соответствия шлюза) e
   Гарантия, что состояние `git submodule` осталось в конце концов.
4. Проверка отдельных документов (код выпуска Ed25519, архив сообщений ACME, токен PagerDuty)
   E se batem com контрольные суммы хранят.
5. Проверка дыма фасада под номерами целей телеметрии (конечная точка Pushgateway, плата GAR Grafana)
   анте делают упражнение.## Этапы автоматизации

### Определенная карта хостов и выпуск для директории RAD

1. Использование помощника по получению детерминированных хостов против набора манифестов
   Предлагаю и подтвержу, что это не так, и они расслабятся.
   `docs/source/soradns/deterministic_hosts.md`.
2. Вот комплект директорий резольверов:

```bash
cargo xtask soradns-directory-release \
  --rad-dir artifacts/soradns/rad_candidates \
  --output-root artifacts/soradns_directory \
  --release-key-path secrets/soradns/release.key \
  --car-cid bafybeigdyrdnsmanifest... \
  --note "dns-kickoff-20250303"
```

3. Зарегистрируйте идентификатор директора, SHA-256 и список отпечатков,
   `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` и через несколько минут начнется начало матча.

### Захват телеметрии DNS

- На передней панели хранятся журналы прозрачности резольверов в течение ≥10 минут использования.
  `scripts/telemetry/run_soradns_transparency_tail.sh --mode staging`.
- Экспортируйте метрики из Pushgateway и архивируйте снимки NDJSON или другие файлы.
  директория запускает ID.

### Упражнения по автоматизации шлюзов

1. Выполните проверку TLS/ECH:

```bash
cargo xtask sorafs-gateway-probe \
  --config configs/sorafs_gateway/probe.staging.toml \
  --output artifacts/sorafs_gateway_probe/<run-id>.json
```

2. Езда на ремне безопасности (`ci/check_sorafs_gateway_conformance.sh`) e
   o Помощник по самодиагностике (`scripts/sorafs_gateway_self_cert.sh`) для настройки
   o комплект тестов Norito.
3. Запишите события PagerDuty/Webhook, чтобы проверить, что происходит автоматизация.
   функция Понта в Понта.

### Эмпакотаменто де евиденсиас

- Атуализировать временные метки `ops/drill-log.md` com, участников и хэши зондов.
- Armazene artefatos nos diretorios de run ID e publique um resumo executivo
  в течение нескольких минут делайте документы/DevRel.
- Ссылка на пакет доказательств не содержит билета управления до начала пересмотра.

## Упрощение заседаний и передача доказательств

- **Подсказка модератора:**
  - T-24 h — Сообщение об управлении программой + снимок повестки дня/представления в `#nexus-steering`.
  - T-2 h — Атуализация сетевого TL или моментального снимка телеметрии GAR и регистрация отличий в `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`.
  - T-15 м — автоматизация операций проверяет наличие датчиков и определяет или запускает идентификатор в активном режиме `artifacts/sorafs_gateway_dns/current`.
  - Durante a chamada — модератор Compartilha este runbook e designa um scriba ao vivo; Документы/DevRel захватывают встроенные файлы acao.
- **Шаблон минут:** Копия эскиза
  `docs/source/sorafs_gateway_dns_design_minutes.md` (только без комплекта)
  сделать портал) и приступить к немедленному просмотру для сеанса. Включенный список
  участники, решения, события, хэши доказательств и риски.
- **Загрузить доказательства:** ZIP-архив в каталоге `runbook_bundle/`,
  anexe или PDF-файл рендерится в течение нескольких минут, регистрируйте хэши SHA-256 в течение нескольких минут +
  повестка дня, сообщите нам об псевдониме рецензентов правительства, когда вы загружаете
  чегарем их `s3://sora-governance/sorafs/gateway_dns/<date>/`.

## Снимок доказательств (начало Марко 2025 г.)

Os ultimos artefatos de ensaio/живые референсиадо без дорожной карты и через несколько минут
фикам без ведра `s3://sora-governance/sorafs/gateway_dns/`. Ос хэширует abaixo
espelham или манифест canonico (`artifacts/sorafs_gateway_dns/<run-id>/runbook_bundle/evidence_manifest_*.json`).

- ** Пробный прогон — 2 марта 2025 г. (`artifacts/sorafs_gateway_dns/20250302/`)**
  - Пакет Tarball: `b13571d2822c51f771d0e471f4f66d088a78ed6c1a5adb0d4b020b04dd9a5ae0`
  - PDF за минуту: `cac89ee3e6e4fa0adb9694941c7c42ffddb513f949cf1b0c9f375e14507f4f18`
- **Мастерская ao vivo — 03.03.2025 (`artifacts/sorafs_gateway_dns/20250303/runbook_bundle/`)**
  - `bc83e6a014c2d223433f04ddc3c588bfeff33ee5cdcb15aad6527efeba582a1c  minutes_20250303.md`
  - `030a98fb3e3a52dbb0fcf25a6ea4365b11d9487707bb6700cb632710f7c082e4  gar_snapshot_20250303.json`
  - `5ac17e684976d6862628672627f229f7719da74235aa0a5f0ce994dad34cb3c4  sorafs_gateway_dns_design_metrics_20250303.prom`
  - `5c6163d0ae9032c2d52ca2ecca4037dfaddcc503eb56239b53c5e9c4000997cf  probe_20250303.json`
  - `87f6341896bfb830966a4a5d0fc9158fabcc135ba16ef0d53882e558de77ba49  probe_20250303_webhook.jsonl`
  - `9b968b0bf4ca654d466ec2be5291936f1441908354e9d2da4d0a52f1568bbe03  probe.staging.toml`
  - _(Загрузить файл: `gateway_dns_minutes_20250303.pdf` — приложение Docs/DevRel или SHA-256 при рендеринге PDF или в пакете.)_

## Связи с материалами- [Книга операций для шлюза] (./operations-playbook.md)
- [План наблюдения от SoraFS](./observability-plan.md)
- [Децентрализованный DNS-трекер и шлюз] (https://github.com/hyperledger-iroha/iroha/blob/master/roadmap.md#core-workstreams)