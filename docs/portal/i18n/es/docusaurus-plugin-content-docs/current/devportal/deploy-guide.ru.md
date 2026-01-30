---
lang: es
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

## Обзор

Этот плейбук переводит пункты дорожной карты **DOCS-7** (публикация SoraFS) и **DOCS-8** (автоматизация pin в CI/CD) в практическую процедуру для портала разработчиков. Он охватывает фазу build/lint, упаковку SoraFS, подписание манифестов через Sigstore, продвижение алиасов, проверку и тренировки отката, чтобы каждый preview и release были воспроизводимыми и аудируемыми.

Поток предполагает, что у вас есть бинарь `sorafs_cli` (собранный с `--features cli`), доступ к Torii endpoint с правами pin-registry и OIDC учетные данные для Sigstore. Долгоживущие секреты (`IROHA_PRIVATE_KEY`, `SIGSTORE_ID_TOKEN`, токены Torii) храните в CI vault; локальные запуски могут подхватывать их из export в shell.

## Предварительные условия

- Node 18.18+ с `npm` или `pnpm`.
- `sorafs_cli` из `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- URL Torii, который открывает `/v1/sorafs/*`, плюс учетная запись/приватный ключ, способные отправлять манифесты и алиасы.
- OIDC issuer (GitHub Actions, GitLab, workload identity и т. п.) для выпуска `SIGSTORE_ID_TOKEN`.
- Опционально: `examples/sorafs_cli_quickstart.sh` для dry run и `docs/source/sorafs_ci_templates.md` для шаблонов GitHub/GitLab workflows.
- Настройте OAuth переменные Try it (`DOCS_OAUTH_*`) и выполните
  [security-hardening checklist](./security-hardening.md) перед продвижением билда за пределы lab. Теперь сборка портала падает, если переменные отсутствуют или TTL/пуллинг параметры выходят за допустимые окна; `DOCS_OAUTH_ALLOW_INSECURE=1` используйте только для одноразовых локальных preview. Прикрепляйте доказательства pen-test к релиз-таску.

## Шаг 0 - Зафиксировать bundle прокси Try it

Перед продвижением preview в Netlify или gateway зафиксируйте источники Try it proxy и digest подписанного OpenAPI манифеста в детерминированный bundle:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` копирует proxy/probe/rollback хелперы, проверяет подпись OpenAPI и пишет `release.json` плюс `checksums.sha256`. Прикрепите этот bundle к Netlify/SoraFS gateway ticket, чтобы ревьюеры могли воспроизвести точные исходники прокси и подсказки Torii target без пересборки. Bundle также фиксирует, включены ли bearer токены клиента (`allow_client_auth`), чтобы план rollout и правила CSP оставались синхронизированными.

## Шаг 1 - Build и lint портала

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

`npm run build` автоматически запускает `scripts/write-checksums.mjs`, создавая:

- `build/checksums.sha256` - SHA256 манифест для `sha256sum -c`.
- `build/release.json` - метаданные (`tag`, `generated_at`, `source`) для каждого CAR/манифеста.

Архивируйте оба файла вместе с CAR summary, чтобы ревьюеры могли сравнивать preview артефакты без пересборки.

## Шаг 2 - Упаковать статические ассеты

Запустите CAR packer на выходном каталоге Docusaurus. Пример ниже пишет все артефакты в `artifacts/devportal/`.

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

Summary JSON фиксирует число чанков, дайджесты и подсказки для планирования proof, которые затем используют `manifest build` и CI dashboards.

## Шаг 2b - Упаковать OpenAPI и SBOM companions

DOCS-7 требует публиковать сайт портала, snapshot OpenAPI и SBOM payloads как отдельные манифесты, чтобы gateways могли добавлять `Sora-Proof`/`Sora-Content-CID` headers для каждого артефакта. Release helper (`scripts/sorafs-pin-release.sh`) уже пакует OpenAPI каталог (`static/openapi/`) и SBOM из `syft` в отдельные CARы `openapi.*`/`*-sbom.*` и записывает метаданные в `artifacts/sorafs/portal.additional_assets.json`. В ручном потоке повторите шаги 2-4 для каждого payload со своими префиксами и метаданными (например `--car-out "$OUT"/openapi.car` плюс `--metadata alias_label=docs.sora.link/openapi`). Зарегистрируйте каждую пару manifest/alias в Torii (сайт, OpenAPI, portal SBOM, OpenAPI SBOM) до переключения DNS, чтобы gateway мог отдавать stapled proofs для всех опубликованных артефактов.

## Шаг 3 - Собрать манифест

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

Настройте pin-policy флаги под окно релиза (например, `--pin-storage-class hot` для канареек). JSON вариант опционален, но удобен для review.

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

Bundle фиксирует дайджест манифеста, дайджесты чанков и BLAKE3 хэш OIDC токена, не сохраняя JWT. Храните bundle и detached signature; продакшен продвижения могут переиспользовать те же артефакты вместо повторной подписи. Локальные запуски могут заменить provider флаги на `--identity-token-env` (или задать `SIGSTORE_ID_TOKEN` в окружении), когда внешний OIDC helper выпускает токен.

## Шаг 5 - Отправить в pin registry

Отправьте подписанный манифест (и chunk plan) в Torii. Всегда запрашивайте summary, чтобы запись registry/alias была аудируемой.

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority ih58... \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

При rollout preview или canary alias (`docs-preview.sora`) повторите отправку с уникальным alias, чтобы QA мог проверить контент перед production promotion.

Alias binding требует трех полей: `--alias-namespace`, `--alias-name` и `--alias-proof`. Governance выдает proof bundle (base64 или Norito bytes) после утверждения запроса alias; храните его в CI secrets и подайте как файл перед `manifest submit`. Оставьте alias флаги пустыми, если хотите только закрепить манифест без изменения DNS.

## Шаг 5b - Сгенерировать governance proposal

Каждый манифест должен сопровождаться парламентским proposal, чтобы любой гражданин Sora мог внести изменение без доступа к привилегированным ключам. После submit/sign выполните:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` фиксирует каноническую инструкцию `RegisterPinManifest`, digest чанков, policy и alias hint. Прикрепите его к governance ticket или Parliament portal, чтобы делегаты могли сравнивать payload без пересборки. Команда не использует ключи Torii, поэтому любой гражданин может подготовить proposal локально.

## Шаг 6 - Проверить proofs и телеметрию

После pin выполните детерминированные проверки:

```bash
sorafs_cli proof verify \
  --manifest "$OUT"/portal.manifest.to \
  --car "$OUT"/portal.car \
  --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
  --manifest "$OUT"/portal.manifest.to \
  --bundle "$OUT"/portal.manifest.bundle.json \
  --chunk-plan "$OUT"/portal.plan.json
```

- Следите за `torii_sorafs_gateway_refusals_total` и `torii_sorafs_replication_sla_total{outcome="missed"}`.
- Запустите `npm run probe:portal`, чтобы проверить Try-It proxy и записанные ссылки на свежем контенте.
- Соберите evidence мониторинга из
  [Publishing & Monitoring](./publishing-monitoring.md), чтобы соблюсти DOCS-3c observability gate. Helper теперь принимает несколько `bindings` (сайт, OpenAPI, portal SBOM, OpenAPI SBOM) и требует `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` на целевом хосте через `hostname` guard. Вызов ниже пишет единый JSON summary и evidence bundle (`portal.json`, `tryit.json`, `binding.json`, `checksums.sha256`) в каталог релиза:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## Шаг 6a - Планирование сертификатов gateway

Сформируйте план TLS SAN/challenge до создания GAR packets, чтобы команда gateway и DNS approvers работали с одной и той же evidence. Новый helper зеркалит DG-3 inputs, перечисляя canonical wildcard hosts, pretty-host SANs, DNS-01 labels и рекомендуемые ACME challenges:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

Коммитьте JSON вместе с release bundle (или прикрепляйте к change ticket), чтобы операторы могли вставить SAN значения в конфигурацию `torii.sorafs_gateway.acme`, а GAR reviewers могли сверить canonical/pretty mappings без повторного запуска. Добавляйте дополнительные `--name` для каждого суффикса в одном релизе.

## Шаг 6b - Получить canonical host mappings

Перед тем как шаблонизировать GAR payloads, зафиксируйте детерминированный host mapping для каждого alias. `cargo xtask soradns-hosts` хеширует каждый `--name` в canonical label (`<base32>.gw.sora.id`), эмитит нужный wildcard (`*.gw.sora.id`) и выводит pretty host (`<alias>.gw.sora.name`). Сохраняйте вывод в release artifacts, чтобы DG-3 reviewers могли сверять mapping рядом с GAR submission:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

Используйте `--verify-host-patterns <file>`, чтобы быстро падать, когда GAR или gateway binding JSON не содержит обязательных хостов. Helper принимает несколько файлов, поэтому легко проверять и GAR template, и `portal.gateway.binding.json` в одной команде:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

Прикрепите summary JSON и verification log к DNS/gateway change ticket, чтобы аудиторы могли подтвердить canonical/wildcard/pretty hosts без повторного запуска. Перезапускайте команду при добавлении новых alias, чтобы GAR updates наследовали те же evidence.

## Шаг 7 - Сгенерировать DNS cutover descriptor

Production cutovers требуют аудируемого change packet. После успешной submission (alias binding) helper генерирует `artifacts/sorafs/portal.dns-cutover.json`, фиксируя:

- metadata alias binding (namespace/name/proof, manifest digest, Torii URL, submitted epoch, authority);
- release context (tag, alias label, manifest/CAR пути, chunk plan, Sigstore bundle);
- verification pointers (probe command, alias + Torii endpoint); и
- опциональные change-control поля (ticket id, cutover window, ops contact, production hostname/zone);
- metadata route promotion из `Sora-Route-Binding` header (canonical host/CID, пути header + binding, команды проверки), чтобы GAR promotion и fallback drills ссылались на одну evidence;
- сгенерированные route-plan артефакты (`gateway.route_plan.json`, header templates и optional rollback headers), чтобы change tickets и CI lint hooks могли проверить, что каждый DG-3 пакет ссылается на canonical promotion/rollback планы;
- опциональные metadata для cache invalidation (purge endpoint, auth variable, JSON payload и пример `curl`);
- rollback hints на предыдущий descriptor (release tag и manifest digest), чтобы change tickets фиксировали детерминированный fallback.

Если release требует cache purge, сгенерируйте канонический plan вместе с descriptor:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

Прикрепите `portal.cache_plan.json` к DG-3 packet, чтобы операторы имели детерминированные hosts/paths (и соответствующие auth hints) при `PURGE` запросах. Опциональная cache секция descriptor может ссылаться на этот файл напрямую, чтобы change-control reviewers видели, какие endpoints очищаются при cutover.

Каждый DG-3 packet также требует promotion + rollback checklist. Сгенерируйте его через `cargo xtask soradns-route-plan`, чтобы change-control reviewers могли отследить точные preflight/cutover/rollback шаги для каждого alias:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

`gateway.route_plan.json` фиксирует canonical/pretty hosts, staged health-check reminders, GAR binding updates, cache purges и rollback actions. Прикладывайте его вместе с GAR/binding/cutover артефактами до подачи change ticket, чтобы Ops могли репетировать те же шаги.

`scripts/generate-dns-cutover-plan.mjs` формирует descriptor и запускается из `sorafs-pin-release.sh`. Для ручной генерации:

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

Заполните опциональные metadata через переменные окружения перед запуском pin helper:

| Переменная | Назначение |
| --- | --- |
| `DNS_CHANGE_TICKET` | Ticket ID, который сохраняется в descriptor. |
| `DNS_CUTOVER_WINDOW` | ISO8601 окно cutover (например, `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | Production hostname + authoritative zone. |
| `DNS_OPS_CONTACT` | On-call alias или escalation контакт. |
| `DNS_CACHE_PURGE_ENDPOINT` | Purge endpoint, записываемый в descriptor. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Env var с purge token (default: `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | Путь к предыдущему cutover descriptor для rollback metadata. |

Прикрепите JSON к DNS change review, чтобы approvers могли проверить manifest digest, alias bindings и probe команды без просмотра CI логов. CLI флаги `--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`, `--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`, `--cache-purge-auth-env` и `--previous-dns-plan` дают те же override при запуске helper вне CI.

## Шаг 8 - Сформировать resolver zonefile skeleton (опционально)

Когда известен production cutover window, release script может автоматически сгенерировать SNS zonefile skeleton и resolver snippet. Передайте нужные DNS записи и metadata через env или CLI; helper вызовет `scripts/sns_zonefile_skeleton.py` сразу после генерации cutover descriptor. Укажите минимум одно значение A/AAAA/CNAME и GAR digest (BLAKE3-256 подписанного GAR payload). Если зона/hostname известны и `--dns-zonefile-out` пропущен, helper пишет в `artifacts/sns/zonefiles/<zone>/<hostname>.json` и создает `ops/soradns/static_zones.<hostname>.json` как resolver snippet.

| Переменная / флаг | Назначение |
| --- | --- |
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | Путь для сгенерированного zonefile skeleton. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Путь resolver snippet (по умолчанию `ops/soradns/static_zones.<hostname>.json`). |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | TTL для записей (default: 600 секунд). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | IPv4 адреса (comma-separated env или повторяемый флаг). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | IPv6 адреса. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | Опциональный CNAME target. |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | SHA-256 SPKI pins (base64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Дополнительные TXT записи (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | Override для computed zonefile version label. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | Указать `effective_at` timestamp (RFC3339) вместо начала cutover window. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | Override для proof literal в metadata. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | Override для CID в metadata. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Guardian freeze state (soft, hard, thawing, monitoring, emergency). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | Guardian/council ticket reference для freeze. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | RFC3339 timestamp для thawing. |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | Дополнительные freeze notes (comma-separated env или повторяемый флаг). |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | BLAKE3-256 digest (hex) подписанного GAR payload. Требуется при gateway bindings. |

Workflow GitHub Actions читает эти значения из secrets репозитория, чтобы каждый production pin автоматически эмитил zonefile artifacts. Настройте следующие secrets (строки могут содержать comma-separated lists для multi-value полей):

| Secret | Назначение |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | Production hostname/zone для helper. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | On-call alias в descriptor. |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | IPv4/IPv6 записи для публикации. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | Опциональный CNAME target. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | SPKI pins base64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | Дополнительные TXT записи. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | Freeze metadata в skeleton. |
| `DOCS_SORAFS_GAR_DIGEST` | Hex BLAKE3 digest подписанного GAR payload. |

При запуске `.github/workflows/docs-portal-sorafs-pin.yml` укажите inputs `dns_change_ticket` и `dns_cutover_window`, чтобы descriptor/zonefile унаследовали корректные метаданные окна. Оставляйте их пустыми только для dry run.

Типичная команда (по runbook SN-7 owner):

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
```

Helper автоматически переносит change ticket в TXT запись и наследует начало cutover window как `effective_at`, если не указано иное. Полный operational workflow см. в `docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Примечание о публичной DNS-делегации

Скелет zonefile определяет только авторитативные записи зоны. Делегацию NS/DS
родительской зоны нужно настроить у регистратора или DNS-провайдера, чтобы
обычный интернет мог найти ваши nameserver'ы.

- Для cutover на apex/TLD используйте ALIAS/ANAME (зависит от провайдера) или
  публикуйте записи A/AAAA, указывающие на anycast-IP gateway.
- Для поддоменов публикуйте CNAME на derived pretty host
  (`<fqdn>.gw.sora.name`).
- Канонический хост (`<hash>.gw.sora.id`) остается в домене gateway и не
  публикуется в вашей публичной зоне.

### Шаблон заголовков gateway

Deploy helper также генерирует `portal.gateway.headers.txt` и `portal.gateway.binding.json` - два артефакта, удовлетворяющие DG-3 требованиям gateway-content-binding:

- `portal.gateway.headers.txt` содержит полный блок HTTP headers (включая `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS и `Sora-Route-Binding`), который edge gateway должен приклеивать к каждому ответу.
- `portal.gateway.binding.json` фиксирует ту же информацию в машиночитаемом виде, чтобы change tickets и автоматизация могли сравнивать host/cid bindings без парсинга shell вывода.

Они генерируются через `cargo xtask soradns-binding-template` и фиксируют alias, manifest digest и gateway hostname, которые были переданы в `sorafs-pin-release.sh`. Для регенерации или кастомизации блока headers выполните:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

Передайте `--csp-template`, `--permissions-template` или `--hsts-template`, чтобы переопределить шаблоны заголовков; используйте их вместе с `--no-*` флагами для полного удаления header.

Прикладывайте snippet заголовков к CDN change request и передавайте JSON в gateway automation pipeline, чтобы реальное продвижение хоста соответствовало evidence.

Release script автоматически запускает verification helper, чтобы DG-3 tickets всегда содержали свежую evidence. Перезапускайте вручную, если правите binding JSON вручную:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

Команда декодирует stapled `Sora-Proof`, проверяет совпадение `Sora-Route-Binding` с manifest CID + hostname и падает при дрейфе headers. Архивируйте вывод вместе с другими release artifacts при запуске вне CI, чтобы DG-3 reviewers имели доказательство проверки.

> **Интеграция DNS descriptor:** `portal.dns-cutover.json` теперь включает секцию `gateway_binding`, указывающую на эти артефакты (пути, content CID, статус proof и literal header template) **и** секцию `route_plan`, ссылающуюся на `gateway.route_plan.json` и основную/rollback template. Включайте эти блоки в каждый DG-3 change ticket, чтобы reviewers могли сравнить точные значения `Sora-Name/Sora-Proof/CSP` и подтвердить соответствие планов promotion/rollback evidence bundle без открытия build архива.

## Шаг 9 - Запустить publishing monitors

Roadmap item **DOCS-3c** требует постоянных доказательств, что портал, Try it proxy и gateway bindings остаются здоровыми после релиза. Запускайте consolidated monitor сразу после шагов 7-8 и подключайте его к вашим scheduled probes:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` загружает config (см. `docs/portal/docs/devportal/publishing-monitoring.md` для схемы) и выполняет три проверки: probes путей портала + CSP/Permissions-Policy validation, Try it proxy probes (опционально `/metrics`), и gateway binding verifier (`cargo xtask soradns-verify-binding`), который теперь требует наличие и ожидаемое значение Sora-Content-CID вместе с alias/manifest проверками.
- Команда завершается с non-zero, когда любой probe падает, чтобы CI/cron/runbook операторы могли остановить релиз до продвижения alias.
- Флаг `--json-out` пишет единый summary JSON по целям; `--evidence-dir` пишет `summary.json`, `portal.json`, `tryit.json`, `binding.json` и `checksums.sha256`, чтобы governance reviewers могли сравнить результаты без повторного запуска мониторинга. Архивируйте этот каталог под `artifacts/sorafs/<tag>/monitoring/` вместе с Sigstore bundle и DNS cutover descriptor.
- Включайте вывод мониторинга, Grafana export (`dashboards/grafana/docs_portal.json`) и Alertmanager drill ID в release ticket, чтобы DOCS-3c SLO можно было аудировать позже. Playbook мониторинга публикаций находится в `docs/portal/docs/devportal/publishing-monitoring.md`.

Portal probes требуют HTTPS и отклоняют `http://` базовые URL, если не задан `allowInsecureHttp` в конфиге monitor; держите production/staging на TLS и включайте override только для локальных preview.

Автоматизируйте монитор через `npm run monitor:publishing` в Buildkite/cron после запуска портала. Та же команда, направленная на production URLs, подпитывает постоянные health checks между релизами.

## Автоматизация через `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` инкапсулирует шаги 2-6. Он:

1. архивирует `build/` в детерминированный tarball,
2. запускает `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature` и `proof verify`,
3. опционально выполняет `manifest submit` (включая alias binding) при наличии Torii credentials, и
4. пишет `artifacts/sorafs/portal.pin.report.json`, опциональный `portal.pin.proposal.json`, DNS cutover descriptor (после submission) и gateway binding bundle (`portal.gateway.binding.json` плюс header block), чтобы governance/networking/ops команды могли сверять evidence без изучения CI логов.

Перед запуском установите `PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME` и (опционально) `PIN_ALIAS_PROOF_PATH`. Используйте `--skip-submit` для dry run; GitHub workflow ниже переключает это через input `perform_submit`.

## Шаг 8 - Публикация OpenAPI specs и SBOM bundles

DOCS-7 требует, чтобы портал, OpenAPI spec и SBOM артефакты проходили один и тот же детерминированный pipeline. Имеющиеся helpers покрывают все три:

1. **Перегенерировать и подписать spec.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   Указывайте `--version=<label>` для сохранения исторического snapshot (например `2025-q3`). Helper пишет snapshot в `static/openapi/versions/<label>/torii.json`, зеркалит в `versions/current` и записывает metadata (SHA-256, manifest status, updated timestamp) в `static/openapi/versions.json`. Портал использует этот индекс для выбора версии и отображения digest/signature. Если `--version` не задан, сохраняются существующие labels и обновляются только `current` + `latest`.

   Manifest фиксирует SHA-256/BLAKE3 digests, чтобы gateway мог приклеивать `Sora-Proof` для `/reference/torii-swagger`.

2. **Сформировать CycloneDX SBOMs.** Release pipeline уже ожидает syft-based SBOMs согласно `docs/source/sorafs_release_pipeline_plan.md`. Держите вывод рядом с build artifacts:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **Упаковать каждый payload в CAR.**

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

   Следуйте тем же шагам `manifest build`/`manifest sign`, что и для основного сайта, задавая отдельные alias на артефакт (например, `docs-openapi.sora` для spec и `docs-sbom.sora` для подписанного SBOM). Отдельные alias оставляют SoraDNS, GAR и rollback tickets привязанными к конкретному payload.

4. **Submit и binding.** Переиспользуйте текущую authority и Sigstore bundle, но фиксируйте alias tuple в release checklist, чтобы аудиторы знали, какой Sora name соответствует какому digest.

Архивация spec/SBOM манифестов вместе с build портала гарантирует, что каждый release ticket содержит полный набор артефактов без повторного запуска packer.

### Automation helper (CI/package script)

`./ci/package_docs_portal_sorafs.sh` кодирует шаги 1-8, чтобы roadmap item **DOCS-7** можно было выполнить одной командой. Helper:

- выполняет подготовку портала (`npm ci`, OpenAPI/Norito sync, widget tests);
- формирует CARs и manifest pairs для портала, OpenAPI и SBOM через `sorafs_cli`;
- опционально выполняет `sorafs_cli proof verify` (`--proof`) и Sigstore signing (`--sign`, `--sigstore-provider`, `--sigstore-audience`);
- кладет все артефакты в `artifacts/devportal/sorafs/<timestamp>/` и пишет `package_summary.json` для CI/release tooling; и
- обновляет `artifacts/devportal/sorafs/latest` на последний запуск.

Пример (полный pipeline с Sigstore + PoR):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

Полезные флаги:

- `--out <dir>` - override корня артефактов (по умолчанию каталоги с timestamp).
- `--skip-build` - использовать существующий `docs/portal/build` (полезно при offline mirrors).
- `--skip-sync-openapi` - пропустить `npm run sync-openapi`, когда `cargo xtask openapi` не может достучаться до crates.io.
- `--skip-sbom` - не вызывать `syft`, если бинарь не установлен (скрипт печатает предупреждение).
- `--proof` - выполнить `sorafs_cli proof verify` для каждой пары CAR/manifest. Мульти-файловые payloads пока требуют chunk-plan support в CLI, поэтому оставляйте этот флаг выключенным при ошибках `plan chunk count` и проверяйте вручную после появления gate.
- `--sign` - вызвать `sorafs_cli manifest sign`. Передайте токен через `SIGSTORE_ID_TOKEN` (или `--sigstore-token-env`) либо позвольте CLI получить его через `--sigstore-provider/--sigstore-audience`.

Для production артефактов используйте `docs/portal/scripts/sorafs-pin-release.sh`. Он пакует портал, OpenAPI и SBOM, подписывает каждый manifest и записывает дополнительные metadata в `portal.additional_assets.json`. Helper понимает те же knobs, что и CI packager, плюс новые `--openapi-*`, `--portal-sbom-*` и `--openapi-sbom-*` для настройки alias tuples, override SBOM source через `--openapi-sbom-source`, пропуск отдельных payloads (`--skip-openapi`/`--skip-sbom`) и указания нестандартного `syft` через `--syft-bin`.

Скрипт выводит все команды; скопируйте лог в release ticket вместе с `package_summary.json`, чтобы ревьюеры могли сверять CAR digests, plan metadata и Sigstore bundle hashes без просмотра случайного shell вывода.

## Шаг 9 - Проверка gateway + SoraDNS

Перед объявлением cutover убедитесь, что новый alias резолвится через SoraDNS и gateways приклеивают свежие proofs:

1. **Запустить probe gate.** `ci/check_sorafs_gateway_probe.sh` выполняет `cargo xtask sorafs-gateway-probe` на demo fixtures в `fixtures/sorafs_gateway/probe_demo/`. Для реальных деплоев укажите target hostname:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   Probe декодирует `Sora-Name`, `Sora-Proof` и `Sora-Proof-Status` по `docs/source/sorafs_alias_policy.md` и падает при расхождении digest манифеста, TTL или GAR bindings.

   For lightweight spot checks (for example, when only the binding bundle
   changed), run `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   The helper validates the captured binding bundle and is handy for release
   tickets that only need binding confirmation instead of a full probe drill.

2. **Собрать evidence для drill.** Для operator drills или PagerDuty dry runs используйте `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario devportal-rollout -- ...`. Wrapper сохраняет headers/logs в `artifacts/sorafs_gateway_probe/<stamp>/`, обновляет `ops/drill-log.md` и опционально запускает rollback hooks или PagerDuty payloads. Установите `--host docs.sora`, чтобы проверять SoraDNS путь, а не IP.

3. **Проверить DNS bindings.** Когда governance публикует alias proof, сохраните GAR файл, указанный в probe (`--gar`), и прикрепите его к release evidence. Resolver owners могут прогнать тот же вход через `tools/soradns-resolver`, чтобы убедиться, что кешируемые записи соответствуют новому манифесту. Перед прикреплением JSON выполните `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`, чтобы детерминированный host mapping, metadata манифеста и telemetry labels были проверены офлайн. Helper может сформировать `--json-out` summary рядом с подписанным GAR.
  При создании нового GAR используйте `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...` (возвращайтесь к `--manifest-cid <cid>` только при отсутствии manifest файла). Helper теперь выводит CID **и** BLAKE3 digest прямо из manifest JSON, удаляет пробелы, дедуплицирует повторяющиеся `--telemetry-label`, сортирует labels и пишет default CSP/HSTS/Permissions-Policy templates перед генерацией JSON, чтобы payload оставался детерминированным.

4. **Следить за метриками alias.** Держите на экране `torii_sorafs_alias_cache_refresh_duration_ms` и `torii_sorafs_gateway_refusals_total{profile="docs"}`; обе серии есть в `dashboards/grafana/docs_portal.json`.

## Шаг 10 - Мониторинг и evidence bundling

- **Dashboards.** Экспортируйте `dashboards/grafana/docs_portal.json` (SLO портала), `dashboards/grafana/sorafs_gateway_observability.json` (latency gateway + proof health) и `dashboards/grafana/sorafs_fetch_observability.json` (orchestrator health) для каждого релиза. Прикладывайте JSON экспорт к release ticket.
- **Probe archives.** Храните `artifacts/sorafs_gateway_probe/<stamp>/` в git-annex или evidence bucket. Включайте probe summary, headers и PagerDuty payload.
- **Release bundle.** Сохраняйте CAR summary портала/SBOM/OpenAPI, manifest bundles, Sigstore signatures, `portal.pin.report.json`, Try-It probe logs и link-check reports в одной папке (например, `artifacts/sorafs/devportal/20260212T1103Z/`).
- **Drill log.** Когда probes - часть drill, `scripts/telemetry/run_sorafs_gateway_probe.sh` добавляет запись в `ops/drill-log.md`, чтобы это покрывало SNNet-5 chaos requirement.
- **Ticket links.** Ссылайтесь на Grafana panel IDs или прикрепленные PNG exports в change ticket вместе с путем к probe report, чтобы ревьюеры могли сверить SLOs без shell доступа.

## Шаг 11 - Multi-source fetch drill и scoreboard evidence

Публикация в SoraFS теперь требует multi-source fetch evidence (DOCS-7/SF-6) вместе с DNS/gateway proofs. После pin манифеста:

1. **Запустить `sorafs_fetch` на live манифесте.** Используйте те же plan/manifest артефакты из шагов 2-3 и gateway credentials для каждого провайдера. Сохраняйте все выходные данные, чтобы аудиторы могли повторить решение orchestrator:

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

   - Сначала получите provider adverts, упомянутые в манифесте (например, `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`) и передайте их через `--provider-advert name=path`, чтобы scoreboard оценивал окна возможностей детерминированно. `--allow-implicit-provider-metadata` используйте **только** для CI фикстур; production drills должны ссылаться на подписанные adverts из pin.
   - При наличии дополнительных регионов повторите команду с соответствующими provider tuples, чтобы каждый cache/alias имел связанный fetch артефакт.

2. **Архивировать выходы.** Сохраняйте `scoreboard.json`, `providers.ndjson`, `fetch.json` и `chunk_receipts.ndjson` в evidence каталоге релиза. Эти файлы фиксируют peer weighting, retry budget, EWMA latency и chunk receipts для SF-7.

3. **Обновить telemetry.** Импортируйте fetch outputs в dashboard **SoraFS Fetch Observability** (`dashboards/grafana/sorafs_fetch_observability.json`), следите за `torii_sorafs_fetch_duration_ms`/`_failures_total` и панелями по provider ranges. Прикладывайте Grafana snapshots к release ticket вместе с путем к scoreboard.

4. **Проверить alert rules.** Запустите `scripts/telemetry/test_sorafs_fetch_alerts.sh`, чтобы проверить Prometheus alert bundle до закрытия релиза. Прикрепите promtool output, чтобы DOCS-7 reviewers подтвердили, что stall/slow-provider alerts активны.

5. **Подключить в CI.** Portal pin workflow держит `sorafs_fetch` шаг за input `perform_fetch_probe`; включите его для staging/production, чтобы fetch evidence формировалась вместе с manifest bundle. Локальные drills могут использовать тот же скрипт, экспортируя gateway tokens и задав `PIN_FETCH_PROVIDERS` (comma-separated список провайдеров).

## Promotion, observability и rollback

1. **Promotion:** держите отдельные staging и production alias. Продвигайте, повторно запуская `manifest submit` с тем же manifest/bundle, меняя `--alias-namespace/--alias-name` на production alias. Это исключает пересборку/повторную подпись после approval QA.
2. **Monitoring:** подключайте dashboard pin-registry (`docs/source/grafana_sorafs_pin_registry.json`) и portal probes (`docs/portal/docs/devportal/observability.md`). Следите за checksum drift, failed probes или spikes retry proof.
3. **Rollback:** для отката повторно отправьте предыдущий manifest (или retire текущий alias) с `sorafs_cli manifest submit --alias ... --retire`. Всегда храните последний known-good bundle и CAR summary, чтобы rollback proofs можно было воспроизвести при ротации CI logs.

## Шаблон CI workflow

Минимальный pipeline должен:

1. Build + lint (`npm ci`, `npm run build`, генерация checksums).
2. Упаковка (`car pack`) и вычисление manifest.
3. Подпись через job-scoped OIDC token (`manifest sign`).
4. Загрузка артефактов (CAR, manifest, bundle, plan, summaries) для аудита.
5. Отправка в pin registry:
   - Pull requests -> `docs-preview.sora`.
   - Tags / protected branches -> promotion production alias.
6. Выполнение probes + proof verification gates перед завершением.

`.github/workflows/docs-portal-sorafs-pin.yml` соединяет эти шаги для manual releases. Workflow:

- build/test портала,
- упаковка build через `scripts/sorafs-pin-release.sh`,
- подпись/проверка manifest bundle через GitHub OIDC,
- загрузка CAR/manifest/bundle/plan/proof summaries как artifacts, и
- (опционально) отправка manifest + alias binding при наличии secrets.

Настройте следующие repository secrets/variables перед запуском job:

| Name | Назначение |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | Torii host с `/v1/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | Epoch identifier, записанный при submission. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | Подписывающая authority для submission манифеста. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | Alias tuple, привязываемый при `perform_submit` = `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Base64 alias proof bundle (опционально). |
| `DOCS_ANALYTICS_*` | Существующие analytics/probe endpoints. |

Запускайте workflow через Actions UI:

1. Укажите `alias_label` (например, `docs.sora.link`), опционально `proposal_alias` и опциональный `release_tag`.
2. Оставьте `perform_submit` выключенным для генерации артефактов без Torii (dry run) или включите для публикации на настроенный alias.

`docs/source/sorafs_ci_templates.md` описывает общие CI helpers для внешних проектов, но для повседневных релизов следует использовать portal workflow.

## Чеклист

- [ ] `npm run build`, `npm run test:*` и `npm run check:links` проходят.
- [ ] `build/checksums.sha256` и `build/release.json` сохранены в artifacts.
- [ ] CAR, plan, manifest и summary сгенерированы под `artifacts/`.
- [ ] Sigstore bundle + detached signature сохранены с логами.
- [ ] `portal.manifest.submit.summary.json` и `portal.manifest.submit.response.json` сохранены при submission.
- [ ] `portal.pin.report.json` (и опциональный `portal.pin.proposal.json`) архивированы рядом с CAR/manifest artifacts.
- [ ] Логи `proof verify` и `manifest verify-signature` сохранены.
- [ ] Grafana dashboards обновлены + Try-It probes успешны.
- [ ] Rollback notes (ID предыдущего манифеста + alias digest) приложены к release ticket.
