---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## Обзор

Этот плейбук переводит пункты дорожной карты **DOCS-7** (публикация SoraFS) и **DOCS-8** (контакт автоматизации в CI/CD) в практическую процедуру для разработчиков портала. Он соответствует фазе build/lint, упаковке SoraFS, подписанию манифестов через Sigstore, продвижению псевдонимов, проверке и откате тренировок, чтобы каждый предварительный просмотр и выпуск были воспроизводимыми и аудируемыми.

Поток предполагает, что у вас есть бинарник `sorafs_cli` (собранный с `--features cli`), доступ к конечной точке Torii с правами pin-реестра и учетные данные OIDC для Sigstore. Долгоживущие секреты (`IROHA_PRIVATE_KEY`, `SIGSTORE_ID_TOKEN`, токены Torii) хранятся в хранилище CI; локальные запуски могут захватывать их из экспорта в оболочку.

## Предварительные условия

- Узел 18.18+ с `npm` или `pnpm`.
- `sorafs_cli` из `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- URL Torii, который предоставляет `/v1/sorafs/*`, а также учетную запись/приватный ключ, способные отправлять манифесты и псевдонимы.
- Эмитент OIDC (GitHub Actions, GitLab, идентификатор рабочей нагрузки и т. п.) для выпуска `SIGSTORE_ID_TOKEN`.
- Опционально: `examples/sorafs_cli_quickstart.sh` для пробного запуска и `docs/source/sorafs_ci_templates.md` для шаблонов рабочих процессов GitHub/GitLab.
- Настроить переменные OAuth Попробуйте (`DOCS_OAUTH_*`) и выполните
  [Контрольный список по усилению безопасности](./security-hardening.md) перед продвижением билда по ограничениям lab. Теперь сборка портала падает, если переменные отключения или TTL/пуллинг параметров приводят к допустимым окнам; `DOCS_OAUTH_ALLOW_INSECURE=1` поддерживают только для одноразовых локальных предварительных просмотров. Прикрепляйте доказательства пен-теста к релиз-таску.

## Шаг 0 - Зафиксировать пакетный прокси Попробуйте

Перед продвижением предварительного просмотра в Netlify или шлюзе зафиксируйте источники Попробуйте прокси и дайджест подписанного OpenAPI манифеста в детерминированном бандле:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` копирует помощники прокси/зонда/отката, затем подпись OpenAPI и пишет `release.json` плюс `checksums.sha256`. Прикрепите этот пакет к билету шлюза Netlify/SoraFS, чтобы ревьюеры могли воспроизвести точные исходники прокси и подсказки Torii target без пересборки. Пакет также фиксирует, включает ли токены на предъявителя клиента (`allow_client_auth`), чтобы планировать развертывание и правила CSP независимо синхронизированными.

## Шаг 1 — Сборка и проверка портала

```bash
cd docs/portal
npm ci
npm run sync-openapi
npm run sync-norito-snippets
npm run test:norito-snippets
npm run test:widgets
npm run check:links
npm run build
```

`npm run build` автоматически запускает `scripts/write-checksums.mjs`, создается:

- `build/checksums.sha256` - Манифест SHA256 для `sha256sum -c`.
- `build/release.json` - метаданные (`tag`, `generated_at`, `source`) для каждого CAR/манифеста.

Архивируйте оба файла вместе с резюме CAR, чтобы ревьюеры могли сравнивать предварительный просмотр документов без пересборки.

## Шаг 2 — Упаковать статические ассеты

Запустите упаковщик CAR в выходном каталоге Docusaurus. Пример ниже пишет все документы в `artifacts/devportal/`.

```bash
OUT=artifacts/devportal
mkdir -p "$OUT"

sorafs_cli car pack \
  --input build \
  --car-out "$OUT"/portal.car \
  --plan-out "$OUT"/portal.plan.json \
  --summary-out "$OUT"/portal.car.json \
  --chunker-handle sorafs.sf1@1.0.0
```

Сводка JSON фиксирует числа чанков, дайджестов и подсказок для проверки планирования, которые затем используют `manifest build` и информационные панели CI.## Шаг 2b - Упаковать OpenAPI и компаньоны SBOM

DOCS-7 требует публикации портала сайта, моментального снимка OpenAPI и полезных данных SBOM в качестве манифеста, чтобы шлюзы могли заголовки `Sora-Proof`/`Sora-Content-CID` для каждого документа. Release helper (`scripts/sorafs-pin-release.sh`) уже пакует каталог OpenAPI (`static/openapi/`) и SBOM из `syft` в программе CARы `openapi.*`/`*-sbom.*` и записывает метаданные в `artifacts/sorafs/portal.additional_assets.json`. В ручном потоке повторите шаги 2–4 для каждой полезной нагрузки со своими префиксами и метаданными (например, `--car-out "$OUT"/openapi.car` плюс `--metadata alias_label=docs.sora.link/openapi`). Зарегистрируйте каждую пару манифестов/псевдонимов в Torii (сайт, OpenAPI, портал SBOM, OpenAPI SBOM) до переключения DNS, чтобы шлюз мог передавать скрепленные доказательства для всех опубликованных документов.

## Шаг 3 — Собрать манифест

```bash
sorafs_cli manifest build \
  --summary "$OUT"/portal.car.json \
  --manifest-out "$OUT"/portal.manifest.to \
  --manifest-json-out "$OUT"/portal.manifest.json \
  --pin-min-replicas 5 \
  --pin-storage-class warm \
  --pin-retention-epoch 14 \
  --metadata alias_label=docs.sora.link
```

Установите флажки политики выводов под окном релиза (например, `--pin-storage-class hot` для Канады). Вариант JSON опционален, но удобен для проверки.

## Шаг 4 - Подписать через Sigstore

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

Bundle фиксирует дайджест-манифест, дайджесты чанков и BLAKE3-хэш OIDC токена, не сохраняя JWT. Храните связку и отдельную подпись; Продавец может переиспользовать те же документы вместо повторной загрузки. Локальные запуски могут заменить флаги провайдера на `--identity-token-env` (или задать `SIGSTORE_ID_TOKEN` в вызове), когда внешний помощник OIDC выдает токен.

## Шаг 5 — Отправить в реестр контактов

Отправьте подписанный манифест (и план фрагмента) в формате Torii. Всегда запрашивайте краткое описание, чтобы запись в реестре/псевдоним была аудируемой.

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority <i105-account-id> \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

При предварительном выпуске или псевдониме canary (`docs-preview.sora`) повторите отправку с псевдонимом признания, чтобы QA мог проверить контент перед продвижением продукции.

Для привязки псевдонима требуется три поля: `--alias-namespace`, `--alias-name` и `--alias-proof`. Управление выдает пакет доказательств (байты base64 или Norito) после утверждения псевдонима запроса; сохраните его в секретах CI и подайте как файл перед `manifest submit`. Оставьте псевдоним флаги пустыми, если хотите только закрепить манифест без изменений DNS.

## Шаг 5b — Сгенерировать предложение по управлению

Каждый манифест должен сопровождаться парламентским предложением, чтобы любой гражданин мог внести изменения без доступа к привилегированным ключам. После отправки/подписи выполните:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` фиксирует каноническую процедуру `RegisterPinManifest`, дайджест чанков, политику и подсказку псевдонима. Прикрепите его к правительственному билету или порталу парламента, чтобы делегаты могли сэкономить полезную нагрузку без пересборки. Команда не использует ключи Torii, поэтому любой гражданин может сформировать предложение локально.

## Шаг 6 - Проверка доказательств и телеметрия

После выполнения PIN-кода определенных проверок:

```bash
sorafs_cli proof verify \
  --manifest "$OUT"/portal.manifest.to \
  --car "$OUT"/portal.car \
  --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
  --manifest "$OUT"/portal.manifest.to \
  --bundle "$OUT"/portal.manifest.bundle.json \
  --chunk-plan "$OUT"/portal.plan.json
```- Следите за `torii_sorafs_gateway_refusals_total` и `torii_sorafs_replication_sla_total{outcome="missed"}`.
- Запустите `npm run probe:portal`, чтобы проверить прокси-сервер Try-It и приведенные ссылки на свежий контент.
- Собрать доказательства из Диптихов.
  [Публикация и мониторинг](./publishing-monitoring.md), чтобы соблюсти шлюз наблюдения DOCS-3c. Helper теперь принимает несколько `bindings` (сайт, OpenAPI, портал SBOM, OpenAPI SBOM) и требует `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` на целевом хосте через `hostname` охранник. Вызов ниже пишет единый JSON-сводку и пакет доказательств (`portal.json`, `tryit.json`, `binding.json`, `checksums.sha256`) в каталоге релиза:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## Шаг 6a — Планирование шлюза сертификатов

Сформируйте план TLS SAN/challenge для создания пакетов GAR, чтобы шлюз команды и утверждающие DNS работали с одним и тем же доказательством. Новый помощник зеркалирует входы DG-3, перечисляемые хосты с каноническими подстановочными знаками, сети SAN с красивыми хостами, метки DNS-01 и рекомендуемые проблемы ACME:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

Коммитируйте JSON вместе с релизным пакетом (или прикрепляйте к билету смены), чтобы операторы могли вставить значения SAN в конфигурацию `torii.sorafs_gateway.acme`, а рецензенты GAR могли сверить канонические/красивые сопоставления без повторного запуска. Добавьте дополнительный `--name` для каждого суффикса в одном релизе.

## Шаг 6b — Получить канонические сопоставления хостов

Перед тем как шаблонизировать полезные данные GAR, зафиксируйте определенное сопоставление хостов для каждого псевдонима. `cargo xtask soradns-hosts` хеширует каждый `--name` в каноническую метку (`<base32>.gw.sora.id`), эмитирует нужный подстановочный знак (`*.gw.sora.id`) и выводит симпатичный хост (`<alias>.gw.sora.name`). Сохраняйте выводы в выпуске артефактов, чтобы рецензенты DG-3 могли сверять сопоставления рядом с подачей GAR:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

Используйте `--verify-host-patterns <file>`, чтобы быстро упасть, когда GAR или JSON привязки шлюза не содержит обязательных хостов. Helper использует несколько файлов, поэтому легко проверить и шаблон GAR, и `portal.gateway.binding.json` в одной группе:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

Прикрепите сводную информацию в формате JSON и журнал проверки к билету изменения DNS/шлюза, чтобы аудиторы могли контролировать canonical/wildcard/pretty хосты без повторного запуска. Перезапускайте команду при добавлении нового псевдонима, чтобы обновления GAR наследовали те же доказательства.

## Шаг 7 — Сгенерировать дескриптор переключения DNS

Переключения производства требуют аудируемого пакета изменений. После успешной отправки (привязки псевдонима) помощник по цепи `artifacts/sorafs/portal.dns-cutover.json`, фиксируя:- привязка псевдонима метаданных (пространство имен/имя/доказательство, дайджест манифеста, URL-адрес Torii, отправленная эпоха, авторитет);
- контекст выпуска (тег, метка псевдонима, путь манифеста/CAR, план фрагмента, пакет Sigstore);
- указатели проверки (команда зонда, псевдоним + конечная точка Torii); и
- опциональные поля контроля изменений (идентификатор заявки, окно переключения, контакт с оператором, имя хоста/зоны производства);
- продвижение маршрута метаданных из заголовка `Sora-Route-Binding` (канонический хост/CID, заголовок пути + привязка, проверка данных), чтобы продвижение GAR и резервные тренировки ссылались на одно свидетельство;
- сгенерированные артефакты плана маршрутизации (`gateway.route_plan.json`, шаблоны заголовков и дополнительные заголовки отката), чтобы изменить заявки и перехватчики CI lint, можно проверить, что каждый пакет DG-3 ссылается на канонические планы продвижения/отката;
- дополнительные метаданные для аннулирования кэша (очистка конечной точки, переменная аутентификации, полезная нагрузка JSON и пример `curl`);
- подсказки отката для предыдущего дескриптора (тега выпуска и дайджеста манифеста), чтобы изменить билеты, фиксирующие определенный резервный вариант.

Если релиз требует очистки кэша, сгенерируйте стандартный план вместе с дескриптором:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

Прикрепите `portal.cache_plan.json` к пакету DG-3, чтобы операторы имели определенные хосты/пути (и соответствующие подсказки аутентификации) при запросах `PURGE`. Опциональная дескриптор секции кэша может ссылаться на этот файл напрямую, чтобы рецензенты, контролирующие изменения, видели, какие конечные точки очищаются при переключении.

Каждый пакет DG-3 также требует продвижения + контрольный список отката. Сгенерируйте его через `cargo xtask soradns-route-plan`, чтобы рецензенты, контролирующие изменения, могли отслеживать точные этапы предполетной проверки/переключения/отката для каждого псевдонима:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

`gateway.route_plan.json` фиксирует канонические/pretty хосты, поэтапные напоминания о проверках работоспособности, обновления привязки GAR, очистку кэша и действия по откату. Прикладывайте его вместе с GAR/привязкой/переключением сертификатов подачи заявки на изменение, чтобы операторы могли повторить те же шаги.

`scripts/generate-dns-cutover-plan.mjs` формирует дескриптор и запускается из `sorafs-pin-release.sh`. Для ручной генерации:

```bash
node scripts/generate-dns-cutover-plan.mjs \
  --pin-report artifacts/sorafs/portal.pin.report.json \
  --out artifacts/sorafs/portal.dns-cutover.json \
  --change-ticket OPS-4821 \
  --dns-hostname docs.sora.link \
  --dns-zone sora.link \
  --ops-contact docs-oncall@sora.link \
  --cache-purge-endpoint https://cache.api/purge \
  --cache-purge-auth-env CACHE_PURGE_TOKEN \
  --previous-dns-plan artifacts/sorafs/previous.dns-cutover.json
```

Заполните опциональные метаданные через переменные окружения перед запуском pin helper:

| Переменная | Назначение |
| --- | --- |
| `DNS_CHANGE_TICKET` | Идентификатор заявки, который сохраняется в дескрипторе. |
| `DNS_CUTOVER_WINDOW` | Обрезка окна ISO8601 (например, `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | Рабочее имя хоста + авторитетная зона. |
| `DNS_OPS_CONTACT` | Псевдоним дежурного или контакт для эскалации. |
| `DNS_CACHE_PURGE_ENDPOINT` | Очистить конечную точку, записанную в дескрипторе. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Env var с токеном очистки (по умолчанию: `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | Путь к предыдущему дескриптору переключения для метаданных отката. |

Прикрепите JSON к проверке изменений DNS, чтобы утверждающие лица могли проверить дайджест манифеста, привязки псевдонимов и зондировать команду без просмотра журналов CI. Флаги CLI `--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`, `--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`, `--cache-purge-auth-env` и `--previous-dns-plan` предоставляют же переопределить при запуске помощника вне CI.## Шаг 8 — Сформировать скелет файла зоны преобразователя (опционально)

Когда известно окно переключения производства, сценарий выпуска может автоматически генерировать скелет файла зоны SNS и фрагмент преобразователя. Передайте нужные записи DNS и метаданные через env или CLI; вспомогательный вызов `scripts/sns_zonefile_skeleton.py` сразу после генерации дескриптора переключения. Укажите хотя бы одно значение A/AAAA/CNAME и дайджест GAR (подписанная полезная нагрузка GAR BLAKE3-256). Если зона/имя хоста известно и `--dns-zonefile-out` пропущено, helper пишет в `artifacts/sns/zonefiles/<zone>/<hostname>.json` и создает `ops/soradns/static_zones.<hostname>.json` как фрагмент резольвера.

| Переменная / флаг | Назначение |
| --- | --- |
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | Путь для сгенерированного скелета файла зоны. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Путь фрагмент резольвера (по умолчанию `ops/soradns/static_zones.<hostname>.json`). |
| И18НИ00000190Х, И18НИ00000191Х | TTL для записей (по умолчанию: 600 секунд). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | Адрес IPv4 (env, разделенный запятыми, или повторяющийся флаг). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | IPv6-адрес. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | Дополнительная цель CNAME. |
| И18НИ00000198Х, И18НИ00000199Х | Выводы SHA-256 SPKI (base64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Дополнительные записи TXT (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | Переопределить метку вычисляемой версии файла зоны. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | Указать временную метку `effective_at` (RFC3339) вместо начала окна переключения. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | Переопределить литерал доказательства в метаданных. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | Переопределить CID в метаданных. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Состояние заморозки Guardian (мягкое, жесткое, оттаивание, мониторинг, аварийная ситуация). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | Ссылка на билет опекуна/совета для заморозки. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | Временная метка RFC3339 для размораживания. |
| И18НИ00000218Х, И18НИ00000219Х | Дополнительные примечания к закреплению (разделенные запятыми env или повторяющийся флаг). |
| И18НИ00000220Х, И18НИ00000221Х | BLAKE3-256 дайджест (шестнадцатеричный) подписанной полезной нагрузки GAR. Требуется при привязке шлюза. |

Рабочий процесс GitHub Actions учитывает значение секретов репозитория, чтобы каждый рабочий вывод автоматически эмитировал артефакты файла зоны. Настройте следующие секреты (строки могут сохранять списки, разделенные запятыми, для полей с несколькими значениями):

| Секрет | Назначение |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | Рабочее имя хоста/зоны для помощника. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | Псевдоним дежурного вызова в дескрипторе. |
| И18НИ00000225Х, И18НИ00000226Х | Записи IPv4/IPv6 для публикации. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | Дополнительная цель CNAME. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Контакты SPKI base64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | Дополнительные записи TXT. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | Заморозить метаданные в скелете. |
| `DOCS_SORAFS_GAR_DIGEST` | Шестнадцатеричный дайджест подписанного полезного груза GAR BLAKE3. |

При запуске `.github/workflows/docs-portal-sorafs-pin.yml` укажите входные данные `dns_change_ticket` и `dns_cutover_window`, чтобы дескриптор/файл зоны унаследовал правильные метаданные окна. Оставляем их пустыми только для пробного прогона.

Типичная команда (от владельца runbook SN-7):

```bash
./docs/portal/scripts/sorafs-pin-release.sh \
  --dns-zonefile-out artifacts/sns/zonefiles/sora.link/20250303.docs.sora.json \
  --dns-zonefile-resolver-snippet ops/soradns/static_zones.docs.json \
  --dns-zonefile-ipv4 198.51.100.4 \
  --dns-zonefile-ttl 600 \
  --dns-zonefile-freeze-state soft \
  --dns-zonefile-freeze-ticket SNS-DF-XXXX \
  --dns-zonefile-freeze-expires-at 2025-03-10T12:00Z \
  --dns-gar-digest <gar-digest-hex> \
  ...other flags...
```Помощник автоматически переносит заявку на изменение в запись TXT и наследует начало окна переключения как `effective_at`, если не указано иное. Полный оперативный документооборот см. в `docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Примечание о публичной DNS-делегации

Файл зоны скелета определяет только авторитарные записи зон. Делегирование НС/ДС
королевской зоны необходимо настроить регистратора или DNS-провайдера, чтобы
обычный интернет мог найти ваш сервер имен.

- Для переключения на apex/TLD воспользуйтесь ALIAS/ANAME (зависит от провайдера) или
  Публикуйте записи A/AAAA, указывающие на шлюз Anycast-IP.
- Для поддоменов опубликуйте CNAME на производном красивом хосте.
  (`<fqdn>.gw.sora.name`).
- Канонический хост (`<hash>.gw.sora.id`) остается в шлюзе домена и не
  публикуется в вашей публичной открытости.

### Шаблон заголовков шлюза

Разверните помощник также цепи `portal.gateway.headers.txt` и `portal.gateway.binding.json` — два экземпляра, обеспечивающие DG-3, требует привязки содержимого шлюза:

- `portal.gateway.headers.txt` содержит полные блочные HTTP-заголовки (включая `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS и `Sora-Route-Binding`), которые пограничный шлюз должен приложить к каждому ответу.
- `portal.gateway.binding.json` фиксирует ту же самую информацию в машиночитаемом виде, чтобы изменить билеты и автоматизация позволяет сравнивать привязки хоста/cid без анализа вывода оболочки.

Они генерируются через `cargo xtask soradns-binding-template` и фиксируют псевдоним, дайджест манифеста и имя хоста шлюза, которые были переданы в `sorafs-pin-release.sh`. Для регенерации или кастомизации заголовков блока выполните:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

Передайте `--csp-template`, `--permissions-template` или `--hsts-template`, чтобы переопределить шаблоны заголовков; Их воспользуйтесь вместе с флагами `--no-*` для насадки для полного удаления.

Прикладывайте фрагмент заголовков к запросу на изменение CDN и передавайте JSON в конвейер автоматизации шлюза, чтобы реально продвинуть адрес хоста в качестве доказательства.

Сценарий выпуска автоматически запускает помощник проверки, чтобы билеты DG-3 всегда предоставляли свежие доказательства. Перезапускайте вручную, если правите привязку JSON вручную:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

Команда декодирует вшитый `Sora-Proof`, в результате чего совпадение `Sora-Route-Binding` с заголовками манифеста CID + имя хоста и падает при дрейфе. Архивируйте вывод вместе с другими артефактами выпуска при запуске вне CI, чтобы рецензенты DG-3 провели проверку доказательств.

> **Интеграционный DNS-дескриптор:** `portal.dns-cutover.json` теперь включает раздел `gateway_binding`, указывающий на эти сертификаты (пути, CID контента, статус-доказательство и буквальный шаблон заголовка) **и** раздел `route_plan`, ссылающийся на `gateway.route_plan.json` и шаблон создания/отката. Включите эти блоки в каждый запрос на изменение DG-3, чтобы рецензенты могли сравнить точные значения `Sora-Name/Sora-Proof/CSP` и обеспечить соответствие плану пакета доказательств продвижения/отката без открытия архива сборки.

## Шаг 9 - Запустить мониторы публикацииПункт дорожной карты **DOCS-3c** постоянное доказательство того, что портал требует проверки привязки прокси-сервера и шлюза после выпуска. Запустите консолидированный монитор сразу после шагов 7–8 и подключите его к запланированным зондам:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` загружает конфиг (см. `docs/portal/docs/devportal/publishing-monitoring.md` для схем) и проводит три проверки: зондирует пути портала + CSP/Permissions-Policy validation, пробует прокси-зонды (опционально `/metrics`) и верификатор привязки шлюза (`cargo xtask soradns-verify-binding`), для чего теперь требуется наличие и ожидаемое значение Sora-Content-CID вместе с проверкой псевдонима/манифеста.
- Команда завершается с ненулевым значением, когда любой зонд падает, чтобы операторы CI/cron/runbook могли остановить релиз до псевдонима продвижения.
- Флаг `--json-out` пишет единый сводный файл JSON по лицам; `--evidence-dir` записывает `summary.json`, `portal.json`, `tryit.json`, `binding.json` и `checksums.sha256`, чтобы рецензенты могли сравнивать результаты без повторного запуска. Архивируйте этот каталог под `artifacts/sorafs/<tag>/monitoring/` вместе с пакетом Sigstore и дескриптором переключения DNS.
- Включите вывод «Детихи», экспорт Grafana (`dashboards/grafana/docs_portal.json`) и идентификатор тренировки Alertmanager в билет релиза, чтобы DOCS-3c SLO можно было аудировать позже. Публикация «Плейбука Диптихи» находится в `docs/portal/docs/devportal/publishing-monitoring.md`.

Зонды портала требуют HTTPS и отклоняют базовые URL-адреса `http://`, если не задано `allowInsecureHttp` в конфигурации монитора; держите производство/постановку на TLS и включите переопределение только для локального предварительного просмотра.

Автоматизируйте монитор через `npm run monitor:publishing` в Buildkite/cron после запуска портала. Та же команда, направленная на рабочие URL-адреса, проводит постоянные проверки работоспособности между релизами.

## Автоматизация через `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` инкапсулирует шаги 2–6. Он:

1. архивирует `build/` в определенный архив,
2. запускает `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature` и `proof verify`,
3. опционально выполнять `manifest submit` (включая привязку псевдонима) при наличии учетных данных Torii, и
4. пишет `artifacts/sorafs/portal.pin.report.json`, опциональный `portal.pin.proposal.json`, дескриптор переключения DNS (после отправки) и пакет привязки шлюза (`portal.gateway.binding.json` плюс блок заголовка), чтобы команды управления/сетей/операций могли сверять доказательства без изучения журналов CI.

Перед запуском установите `PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME` и (опционально) `PIN_ALIAS_PROOF_PATH`. Используйте `--skip-submit` для пробного прогона; Рабочий процесс GitHub ниже переключает его через ввод `perform_submit`.

## Шаг 8 — Публикация спецификаций OpenAPI и пакетов SBOM

Для портала DOCS-7 требуется спецификация OpenAPI и документы SBOM, проходящие через один и тот же определенный конвейер. Имеющиеся помощники раскрывают все три:

1. **Перегенерировать и подписать спец.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```Указывайте `--version=<label>` для сохранения предыдущего снимка (например, `2025-q3`). Helper записывает снимок в `static/openapi/versions/<label>/torii.json`, зеркалирует в `versions/current` и записывает метаданные (SHA-256, статус манифеста, обновленную метку времени) в `static/openapi/versions.json`. Портал использует этот индекс для выбора версии и отображения дайджеста/подписи. Если `--version` не задано, то переменные метки и обновляются только `current` + `latest`.

   Манифест фиксирует дайджесты SHA-256/BLAKE3, чтобы шлюз мог приклеить `Sora-Proof` для `/reference/torii-swagger`.

2. **Сформировать SBOM CycloneDX.** Конвейер выпуска уже ожидает SBOM на основе syft согласно `docs/source/sorafs_release_pipeline_plan.md`. Держите вывод рядом с артефактами сборки:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **Упаковать каждую полезную нагрузку в CAR.**

   ```bash
   sorafs_cli car pack \
     --input docs/portal/static/openapi \
     --car-out "$OUT"/openapi.car \
     --plan-out "$OUT"/openapi.plan.json \
     --summary-out "$OUT"/openapi.car.json

   sorafs_cli car pack \
     --input "$OUT"/portal.sbom.json \
     --car-out "$OUT"/portal.sbom.car \
     --plan-out "$OUT"/portal.sbom.plan.json \
     --summary-out "$OUT"/portal.sbom.car.json
   ```

   Следуйте тем же шагам `manifest build`/`manifest sign`, что и для основного сайта, задавая добавление псевдонима в документ (например, `docs-openapi.sora` для спецификации и `docs-sbom.sora` для подписанного SBOM). Отдельные псевдонимы о объявлениях SoraDNS, GAR и билеты отката предписанными к конкретной полезной нагрузке.

4. **Отправка и привязка.** Используйте текущую авторизацию и пакет Sigstore, но зафиксируйте кортеж псевдонимов в контрольном списке выпуска, чтобы аудиторы знали, какое имя Sora соответствует какому дайджесту.

Архивация спецификаций/манифестов SBOM вместе с порталом сборки гарантирует, что каждый билет выпуска содержит полный набор документов без повторного запуска упаковщика.

### Помощник по автоматизации (скрипт CI/пакета)

`./ci/package_docs_portal_sorafs.sh` кодирует шаги 1–8, чтобы элемент дорожной карты **DOCS-7** можно было выбрать одной командой. Помощник:

- проведение подготовки портала (синхронизация `npm ci`, OpenAPI/Norito, тесты виджетов);
- формирует CAR и пары манифеста для портала, OpenAPI и SBOM через `sorafs_cli`;
- опционально исполнение `sorafs_cli proof verify` (`--proof`) и Sigstore подписание (`--sign`, `--sigstore-provider`, `--sigstore-audience`);
- кладет все необходимые документы в `artifacts/devportal/sorafs/<timestamp>/` и записывает `package_summary.json` для инструментов CI/выпуска; и
- обновляется `artifacts/devportal/sorafs/latest` при последнем запуске.

Пример (полный конвейер с Sigstore + PoR):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

Полезные флаги:

- `--out <dir>` - переопределить исходные документы (по умолчанию каталоги с меткой времени).
- `--skip-build` - использовать существующий `docs/portal/build` (полезно при оффлайн зеркалах).
- `--skip-sync-openapi` - пропустить `npm run sync-openapi`, когда `cargo xtask openapi` не сможет достучаться до crates.io.
- `--skip-sbom` - не сохранять `syft`, если бинарный файл не установлен (скрипт печатает предупреждение).
- `--proof` - Выбор `sorafs_cli proof verify` для каждой пары CAR/манифеста. Мультифайловые полезные нагрузки требуют поддержки фрагментного плана в CLI, поэтому предоставьте этот флаг выключенным при ошибках `plan chunk count` и проверяйте вручную после показа шлюза.
- `--sign` - вызвать `sorafs_cli manifest sign`. Передайте токен через `SIGSTORE_ID_TOKEN` (или `--sigstore-token-env`) либо разрешите CLI получить его через `--sigstore-provider/--sigstore-audience`.Для производства артефактов воспользуйтесь `docs/portal/scripts/sorafs-pin-release.sh`. Он пакует портал OpenAPI и SBOM, подписывает каждый манифест и записывает дополнительные метаданные в `portal.additional_assets.json`. Helper понимает те же ручки, что и CI packager, а также новые `--openapi-*`, `--portal-sbom-*` и `--openapi-sbom-*` для кортежей псевдонимов, переопределение источника SBOM через `--openapi-sbom-source`, пропуск отдельных полезных данных (`--skip-openapi`/`--skip-sbom`) и характеристики нестандартного `syft` через `--syft-bin`.

Скрипт выводит все команды; Скопируйте регистрацию в билете выпуска вместе с `package_summary.json`, чтобы ревьюеры могли сверять дайджесты CAR, метаданные плана и хэши пакета Sigstore без просмотра случайного результата оболочки.

## Шаг 9 — Проверка шлюза + SoraDNS

Перед объявлением переключения убедитесь, что новый псевдоним резолвится через SoraDNS, и шлюзы приклеивают свежие доказательства:

1. **Запуск щупа.** `ci/check_sorafs_gateway_probe.sh` выполните `cargo xtask sorafs-gateway-probe` на демонстрационных приборах в `fixtures/sorafs_gateway/probe_demo/`. Для отдельных деплоев укажите имя целевого хоста:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   Зонд декодирует `Sora-Name`, `Sora-Proof` и `Sora-Proof-Status` по `docs/source/sorafs_alias_policy.md` и падает при расстановке дайджеста манифеста, привязок TTL или GAR.

   Для облегченных выборочных проверок (например, когда только пакет переплета
   изменилось), запустите `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   Помощник проверяет захваченный пакет привязок и удобен для выпуска.
   билеты, для которых требуется только обязательное подтверждение вместо полной проверки.

2. **Собрать доказательства для тренировки.** Для операторских дрелей или пробных прогонов PagerDuty используйте `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario devportal-rollout -- ...`. Wrapper сохраняет заголовки/логи в `artifacts/sorafs_gateway_probe/<stamp>/`, обновляет `ops/drill-log.md` и опционально запускает перехватчики отката или полезные нагрузки PagerDuty. Установите `--host docs.sora`, чтобы проверить путь SoraDNS, а не IP.

3. **Проверить привязки DNS.** Когда управление публикует подтверждение псевдонима, сохраните GAR-файл, указанный в зонде (`--gar`), и прикрепите его к освобождению доказательства. Владельцы резольвера могут прогнать тот же вход через `tools/soradns-resolver`, чтобы убедиться, что кешируемые записи соответствуют новому манифесту. Перед добавлением JSON выполняется `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`, чтобы определенное сопоставление хостов, манифест метаданных и метки телеметрии были проверены офлайн. Helper может составить резюме `--json-out` рядом с подписанным GAR.
  При создании нового GAR воспользуйтесь `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...` (возвращайтесь к `--manifest-cid <cid>` только при отсутствии файла манифеста). Helper теперь выводит CID **и** BLAKE3 дайджест прямо из манифеста JSON, сохраняет пробелы, дедулицирует повторяющиеся `--telemetry-label`, сортирует метки и записывает шаблоны CSP/HSTS/Permissions-Policy по умолчанию перед генерацией JSON, чтобы полезная нагрузка была определена.

4. **Следить за метриками псевдонима.** Держите на экране `torii_sorafs_alias_cache_refresh_duration_ms` и `torii_sorafs_gateway_refusals_total{profile="docs"}`; обе серии есть в `dashboards/grafana/docs_portal.json`.

## Шаг 10 – Мониторинг и объединение доказательств- **Панели мониторинга.** Экспортируйте `dashboards/grafana/docs_portal.json` (SLO-портал), `dashboards/grafana/sorafs_gateway_observability.json` (шлюз задержки + проверка работоспособности) и `dashboards/grafana/sorafs_fetch_observability.json` (работоспособность оркестратора) для каждого релиза. Используйте JSON-экспорт для выпуска билета.
- **Архивы зондирования.** Храните `artifacts/sorafs_gateway_probe/<stamp>/` в git-annex или контейнере доказательств. Включите сводку зонда, заголовки и полезную нагрузку PagerDuty.
- **Release package.** Сохраняйте сводку CAR портала/SBOM/OpenAPI, пакеты манифеста, подписи Sigstore, `portal.pin.report.json`, журналы проб Try-It и отчеты о проверке ссылок на одном компьютере (например, `artifacts/sorafs/devportal/20260212T1103Z/`).
- **Журнал тренировки.** Когда зонды - часть тренировки, в `scripts/telemetry/run_sorafs_gateway_probe.sh` вносится запись в `ops/drill-log.md`, чтобы это покрывало требования SNNet-5 к хаосу.
- **Ссылки на заявки.** Ссылайтесь на идентификаторы панелей Grafana или прикрепленные файлы PNG для экспорта в заявку на изменение вместе с отчетом о проверке, чтобы ревьюеры могли проверить SLO без доступа к оболочке.

## Шаг 11. Анализ выборки из нескольких источников и доказательства на табло

Публикация в SoraFS теперь требует подтверждения выборки из нескольких источников (DOCS-7/SF-6) вместе с доказательствами DNS/шлюза. После закрепления манифеста:

1. **Запустите `sorafs_fetch` в реальном манифесте.** Используйте те же документы плана/манифеста из стадий 2–3 и учетные данные шлюза для каждого провайдера. Сохраняйте данные всех выходных, чтобы аудиторы могли использовать оркестратор решений:

   ```bash
   OUT=artifacts/sorafs/devportal
   FETCH_OUT="$OUT/fetch/$(date -u +%Y%m%dT%H%M%SZ)"
   mkdir -p "$FETCH_OUT"

   cargo run -p sorafs_car --bin sorafs_fetch -- \
     --plan "$OUT/portal.plan.json" \
     --manifest-json "$OUT/portal.manifest.json" \
     --gateway-provider name=docs-us,provider-id="$DOCS_US_PROVIDER_ID",base-url="$DOCS_US_GATEWAY",stream-token="$DOCS_US_STREAM_TOKEN" \
     --gateway-provider name=docs-eu,provider-id="$DOCS_EU_PROVIDER_ID",base-url="$DOCS_EU_GATEWAY",stream-token="$DOCS_EU_STREAM_TOKEN" \
     --scoreboard-out "$FETCH_OUT/scoreboard.json" \
     --provider-metrics-out "$FETCH_OUT/providers.ndjson" \
     --json-out "$FETCH_OUT/fetch.json" \
     --chunk-receipts-out "$FETCH_OUT/chunk_receipts.ndjson" \
     --telemetry-json artifacts/sorafs/provider_telemetry.json \
     --max-peers=3 \
     --retry-budget=4
   ```

   - Сначала получите рекламные объявления поставщика, упомянутые в манифесте (например, `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`), и передайте их через `--provider-advert name=path`, чтобы табло измеряло возможности окна определенным образом. `--allow-implicit-provider-metadata` викор **только** для CI фикстур; Производственные сверла должны ссылаться на подписанные объявления из булавки.
   - При наличии дополнительных регионов повторите команду с поддержкой кортежей поставщика, чтобы каждый кэш/псевдоним имел связанный извлекаемый документ.

2. **Архив выходов.** Сохраняйте `scoreboard.json`, `providers.ndjson`, `fetch.json` и `chunk_receipts.ndjson` в каталоге свидетельств релиза. Эти файлы фиксируют вес одноранговых узлов, бюджет повторных попыток, задержку EWMA и поступления фрагментов для SF-7.

3. **Обновить телеметрию.** Импортируйте выходные данные выборки в панель управления **SoraFS Fetch Observability** (`dashboards/grafana/sorafs_fetch_observability.json`), следуя за `torii_sorafs_fetch_duration_ms`/`_failures_total` и панелями по диапазонам провайдера. Прикладывайте снимки Grafana, чтобы выпустить билет, вместе с табло.

4. **Проверить правила оповещений.** Запустите `scripts/telemetry/test_sorafs_fetch_alerts.sh`, чтобы проверить пакет оповещений Prometheus до закрытия релиза. Прикрепите выходные данные promtool, чтобы рецензенты DOCS-7 подтвердили, что оповещения о остановке/медленной работе провайдера активны.

5. **Подключиться к CI.** Рабочий процесс портального вывода содержит шаг `sorafs_fetch` для ввода `perform_fetch_probe`; Его для постановки/производства, чтобы получить доказательства, формировалась вместе с пакетом манифеста. Локальные тренировки могут использовать тот же скрипт, экспортируя токены шлюза и задав `PIN_FETCH_PROVIDERS` (список провайдеров, разделенный запятыми).

## Продвижение, наблюдаемость и откат1. **Акция**: устанавливаете псевдоним постановки и производства. Продвигайте, повторно запустите `manifest submit` с тем же манифестом/пакетом, меняя `--alias-namespace/--alias-name` на рабочем псевдониме. Это ответственность за пересборку/повторную подпись после утверждения QA.
2. **Мониторинг:** подключите приборную панель с пин-реестром (`docs/source/grafana_sorafs_pin_registry.json`) и портальными датчиками (`docs/portal/docs/devportal/observability.md`). Следите за дрейфом контрольной суммы, неудачными проверками или резкими повторными попытками проверки.
3. **Откат:** для отката повторно отправьте предыдущий манифест (или удалите текущий псевдоним) с `sorafs_cli manifest submit --alias ... --retire`. Всегда храните последний заведомо исправный пакет и сводку CAR, чтобы доказательства отката можно было воспроизвести при ротации журналов CI.

## Шаблон рабочего процесса CI

Минимальный конвейер должен:

1. Сборка + lint (`npm ci`, `npm run build`, генерация контрольных сумм).
2. Упаковка (`car pack`) и вычисление манифеста.
3. Подпись через токен OIDC (`manifest sign`).
4. Загрузка документов (CAR, манифест, комплект, план, резюме) для аудита.
5. Отправка в реестр контактов:
   - Запросы на извлечение -> `docs-preview.sora`.
   - Теги/защищенные ветки -> псевдоним продвижения продукции.
6. Выполнение проб + ворота проверки доказательств перед завершением.

`.github/workflows/docs-portal-sorafs-pin.yml` выполняет эти шаги для выпуска вручную. Рабочий процесс:

- построить/тестировать портал,
- сборка упаковки через `scripts/sorafs-pin-release.sh`,
- Подпись/проверка пакета манифеста через GitHub OIDC,
- загрузка резюме CAR/манифеста/пакета/плана/доказательства как артефакты, и
- (опционально) отправка манифеста + привязки псевдонима при наличии секретов.

Перед запуском задания установите следующие секреты/переменные репозитория:

| Имя | Назначение |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | Хост Torii с `/v1/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | Идентификатор эпохи, указанный при подаче. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | Подписывающая власть для подачи манифеста. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | Кортеж-псевдоним, привязываемый при `perform_submit` = `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Пакет проверки псевдонимов Base64 (опционально). |
| `DOCS_ANALYTICS_*` | Существующие аналитические/конечные точки зондирования. |

Запускайте рабочий процесс через пользовательский интерфейс действий:

1. Укажите `alias_label` (например, `docs.sora.link`), опционально `proposal_alias` и опционально `release_tag`.
2. Оставьте встроенный `perform_submit` выключенным для генерации артефактов без Torii (пробный прогон) или для публикации под псевдонимом настроения.

`docs/source/sorafs_ci_templates.md` описывает общие помощники CI для внешних проектов, но для ежедневных выпусков следует использовать рабочий процесс портала.

## Чеклист- [ ] `npm run build`, `npm run test:*` и `npm run check:links` передачи.
- [ ] `build/checksums.sha256` и `build/release.json` сохранены в артефактах.
- [ ] CAR, план, манифест и сводка сгенерированы под `artifacts/`.
- [ ] Sigstore комплект + отдельная подпись сохранены с логами.
- [ ] `portal.manifest.submit.summary.json` и `portal.manifest.submit.response.json` сохраняются при отправке.
- [ ] `portal.pin.report.json` (и опциональный `portal.pin.proposal.json`) архивируются рядом с CAR/артефактами манифеста.
- [ ] Логи `proof verify` и `manifest verify-signature` сохранены.
- [ ] Grafana обновлены панели мониторинга + успешны пробные версии.
- [ ] Примечания к откату (ID формы манифеста + дайджест псевдонима) приложить к выпуску билета.