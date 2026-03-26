---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## Панорама общая

Эта книга действий преобразует элементы дорожной карты **DOCS-7** (публикация SoraFS) и **DOCS-8**
(автоматизация вывода CI/CD) в процедуре, доступной для доступа к порталу удаления.
Cubre la fase de build/lint, el empaquetado SoraFS, la Firma de Manificos con Sigstore,
продвижение псевдонима, проверка и инструкции по откату для предварительного просмотра и выпуска данных
Морская воспроизводимость и проверяемость.

Эль-флухо предполагает, что связан бинарный файл `sorafs_cli` (создан с `--features cli`), доступ к
Конечная точка Torii с разрешениями на регистрацию контактов и учетными данными OIDC для Sigstore. Секреты гвардии
большая жизнь (`IROHA_PRIVATE_KEY`, `SIGSTORE_ID_TOKEN`, токены Torii) в хранилище CI; лас
Местные выбросы могут быть загружены после экспорта оболочки.

## Предварительные условия

- Узел 18.18+ с `npm` или `pnpm`.
- `sorafs_cli` от `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- URL-адрес Torii, который экспонирует `/v1/sorafs/*`, является приватной учетной записью/клавишой авторизации, которую можно передать манифесту и псевдониму.
— Emisor OIDC (GitHub Actions, GitLab, удостоверение рабочей нагрузки и т. д.) для эмитирования `SIGSTORE_ID_TOKEN`.
- Необязательно: `examples/sorafs_cli_quickstart.sh` для пробных прогонов и `docs/source/sorafs_ci_templates.md` для формирования рабочих процессов GitHub/GitLab.
- Настройте переменные OAuth в Try it (`DOCS_OAUTH_*`) и извлеките их.
  [Контрольный список по усилению безопасности](./security-hardening.md) перед продвижением сборки
  Фуэра дель лаборатории. Сборка портала сейчас падает, когда эти переменные фальтан
  o когда ручки TTL/опроса будут задействованы в приложениях; экспорт
  `DOCS_OAUTH_ALLOW_INSECURE=1` только для предварительного просмотра удаляемых языков. Адъюнта ла
  доказательство пен-теста на выпускном билете.

## Шаг 0 - Захват пакета прокси Попробуйте

Прежде чем продвигать предварительный просмотр Netlify или шлюза, продайте свои прокси-серверы. Попробуйте и
дайджест манифеста OpenAPI, зафиксированный в определенном пакете:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` скопируйте помощники прокси/зонда/отката, проверьте фирму
OpenAPI и пропишите `release.json` для `checksums.sha256`. Дополнительный пакет к билету
Продвижение шлюза Netlify/SoraFS для того, чтобы исправления можно было воспроизвести точно
прокси-сервер и точки назначения Torii без реконструкции. Пакет также должен быть зарегистрирован
носители провистов для клиентов, обладающих навыками (`allow_client_auth`) для обслуживания плана
развертывание и регулярная синхронизация CSP.

## Шаг 1 — Постройте и проверьте портал

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

`npm run build` автоматически извлекается `scripts/write-checksums.mjs`, производится:

- `build/checksums.sha256` — отображает SHA256, соответствующий `sha256sum -c`.
- `build/release.json` - метаданные (`tag`, `generated_at`, `source`) в каждом CAR/манифесте.

Архив из архивов вместе с резюме CAR, чтобы можно было сравнить артефакты
предварительный просмотр греха реконструкции.

## Шаг 2 – Эмпакета лос активы статикосВытащите контейнер CAR из папки Docusaurus. Эль-Эджемпло-де-Абахо
Опишите все артефакты bajo `artifacts/devportal/`.

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

Возобновлен JSON-захват блоков, дайджестов и планов доказательства, которые
`manifest build` и панели мониторинга CI повторно используются после этого.

## Пасо 2b - Компаньоны Empaqueta OpenAPI y SBOM

DOCS-7 требует публикации местоположения портала, моментального снимка OpenAPI и полезных данных SBOM.
как отдельные манифесты для шлюзов, можно вставить заголовки
`Sora-Proof`/`Sora-Content-CID` для каждого артефакта. Эль помощник по освобождению
(`scripts/sorafs-pin-release.sh`) вы запечатываете директорию OpenAPI
(`static/openapi/`) и SBOM, излучаемые через `syft` и отдельные CAR
`openapi.*`/`*-sbom.*` и зарегистрируйте метаданные на
`artifacts/sorafs/portal.additional_assets.json`. Руководство по извлечению жидкости из воды,
повторите Лос-Пасос 2-4 для каждой полезной нагрузки с использованием префиксов и этикеток метаданных.
(например, `--car-out "$OUT"/openapi.car` для
`--metadata alias_label=docs.sora.link/openapi`). Registra Cada по манифесту/псевдониму
en Torii (sitio, OpenAPI, SBOM del portal, SBOM de OpenAPI) перед сменой DNS для этого
Шлюз может быть использован для доступа ко всем публичным артефактам.

## Шаг 3 – Создание манифеста

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

Отрегулируйте политические флаги, закрепив их в следующем выпуске (например, `--pin-storage-class
горячие пара канареек). Вариант JSON является необязательным, но удобным для проверки кода.

## Пасо 4 - Фирма с Sigstore

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

Регистрация пакета данных дайджеста манифеста, дайджестов фрагментов и хэша BLAKE3 токена
OIDC не сохраняет JWT. Guarda Tanto el Bundle как отдельная фирма; лас-промоционес де
производство может повторно использовать артефакты и повторно использовать их. Местные выбросы
можно повторно установить флаги поставщика с `--identity-token-env` (или установщиком
`SIGSTORE_ID_TOKEN` в электронном виде), когда помощник OIDC внешне излучает токен.

## Шаг 5 – реестр контактов Envia

Отправка фирменного заявления (и плана фрагментов) Torii. Пожалуйста, продолжайте возобновлять работу, чтобы
la entrada/alias resultante sea, поддающийся проверке.

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority soraカタカナ... \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

Когда вы укажете псевдоним предварительного просмотра canary (`docs-preview.sora`), повторите его.
Отправка с уникальным псевдонимом для обеспечения качества, позволяющей проверить контент перед продвижением
производство.

Привязка псевдонима требует трех полей: `--alias-namespace`, `--alias-name` и `--alias-proof`.
Управление создает пакет доказательств (base64 или байт Norito), когда будет получен запрос
дель псевдоним; охранять секреты CI и экспонировать как архив до вызова `manifest submit`.
Установите флаги псевдонимов, когда они будут отображаться только в DNS.

## Пасо 5b – Общие условия управления

Када манифестирует, что нужно пройти через список депутатов для парламента, чтобы он мог действовать
Сора может ввести ваш камбио без привилегий. Despues de los pasos
отправить/подписать, выдать:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
````portal.pin.proposal.json` захват инструкции Canonica `RegisterPinManifest`,
эль дайджест де кусков, ла политика и ла писта де псевдоним. Дополнительный билет на управление или все
Портал Парламента позволяет делегатам сравнить полезную нагрузку и реконструировать артефакты.
Как командующий нунса, пока я клаву авторизации Torii, cualquier ciudadano может быть отредактирован
местное пропуэста.

## Пасо 6 – проверка правильности и телеметрии

В случае обнаружения, выберите определенные способы проверки:

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

- Верифика `torii_sorafs_gateway_refusals_total` y
  `torii_sorafs_replication_sla_total{outcome="missed"}` для аномалий.
- Измените `npm run probe:portal` для запуска прокси-сервера Try-It и зарегистрированных ссылок.
  против полученного контента.
- Captura la videncia de Monitoreo Descrita en
  [Публикация и мониторинг](./publishing-monitoring.md) для шлюза наблюдения DOCS-3c
  se satisfaga junto a los pasos de publicacion. El helper ahora acepta Multiples entradas
  `bindings` (sitio, OpenAPI, SBOM del portal, SBOM de OpenAPI) и апликация `Sora-Name`/`Sora-Proof`/`Sora-Content-CID`
  на хосте через дополнительный охранник `hostname`. La Invocacion de Abajo написать Танто ООН
  Возобновить единый JSON в качестве пакета доказательств (`portal.json`, `tryit.json`, `binding.json` y
  `checksums.sha256`) в директории выпуска:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## Paso 6a — Сертификаты шлюза

Получите план SAN/вызов TLS перед созданием пакетов GAR для оборудования шлюза и Лос-Анджелеса.
Специалисты DNS пересмотрели неверные доказательства. Новый помощник отображает инструменты автоматизации
В списке DG-3 указаны канонические хосты с подстановочными знаками, сети SAN с красивыми хостами, этикетки DNS-01 и соответствующие рекомендации ACME:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

Скомитет JSON-объединения с пакетом выпуска (или запрос с камбузным билетом) для операторов
Вы можете использовать значения SAN в конфигурации `torii.sorafs_gateway.acme` из Torii и вносить изменения.
GAR может подтвердить канонические/довольно греховные изображения, повторно выведенные из хоста. Агрега
Дополнительные аргументы `--name` для каждого успешного продвижения в выпуске «Mismo».

## Paso 6b – Получение карт хоста canonicos

Перед использованием полезных нагрузок тамплиеров GAR зарегистрируйте карту хоста, определённую для каждого псевдонима.
`cargo xtask soradns-hosts` hashea cada `--name` в каноническом этикете
(`<base32>.gw.sora.id`), введите запрос подстановочного знака (`*.gw.sora.id`) и получите красивый хост
(И18НИ00000150Х). Сохраняйте высвобождение артефактов для внесения изменений.
DG-3 можно сравнить карту мира с отправкой GAR:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

США `--verify-host-patterns <file>` для быстрого падения, когда JSON-де-GAR или привязка шлюза
опустите один требуемый хост. Помощник принимает несколько архивов проверки, haciendo
можно легко перейти к заводу GAR как `portal.gateway.binding.json`, включив его в мисма-вызов:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```Дополнение к возобновлению JSON и журналу проверки билета DNS/шлюза для этого
аудиторы могут подтвердить канонические хосты, подстановочные знаки и довольно грех повторно запустить сценарии.
Повторно выберите команду, когда вы соедините новый псевдоним для пакета для актуализации GAR
hereden la misma Evidencia.

## Шаг 7 — Общий дескриптор переключения DNS

Производственные обрезки требуют проверки пакета камбио. Despues de un submit exitoso
(привязка псевдонима), el helper emite
`artifacts/sorafs/portal.dns-cutover.json`, захвачено:

- метаданные привязки псевдонима (пространство имен/имя/доказательство, дайджест манифеста, URL Torii,
  эпоха enviado, autoridad);
- контекст выпуска (тег, псевдоним метки, маршруты манифеста/CAR, план фрагментов, пакет Sigstore);
- Punteros de verificacion (командный зонд, псевдоним + конечная точка Torii); й
- дополнительные поля контроля сдачи (идентификатор билета, выход на переключение, контактные операции,
  имя хоста/зона производства);
- метаданные продвижения маршрутов, полученные из заголовка `Sora-Route-Binding`
  (хост canonico/CID, правила заголовка + привязка, команды проверки), гарантия, что продвижение
  GAR и лос-учения по резервным ссылкам на ошибочные доказательства;
- Сгенерированные артефакты плана маршрута (`gateway.route_plan.json`,
  шаблоны заголовков и дополнительные заголовки отката), для того, чтобы билеты камбии и перехваты
  Воспользуйтесь CI, чтобы проверить, что каждый пакет DG-3 ссылается на планы продвижения/отката.
  каноники до апробации;
- Дополнительные метаданные недействительности кэша (конечная точка очистки, переменная аутентификации, полезная нагрузка JSON,
  и пример команды `curl`); й
- подсказки об откате предыдущего дескриптора (тег выпуска и дайджест манифеста)
  для того, чтобы билеты захватили детерминированный резервный путь.

Когда требуется очистка кэша, создается канонический план вместе с дескриптором отключения:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

Дополнение к `portal.cache_plan.json`, полученное в пакете DG-3 для работы
все хосты/пути детерминированы (и подсказки аутентификации совпадают) для запросов эмитента `PURGE`.
Дополнительный раздел кэша дескриптора может быть направлен непосредственно в этот архив,
поддерживайте внесенные изменения в контроль изменений с точностью до того, что конечные точки будут свободными
во время переключения.

Для каждого пакета DG-3 также необходим контрольный список продвижения + откат. Генерала через
`cargo xtask soradns-route-plan`, чтобы изменения в управлении изменениями могли быть продолжены
Точные предполетные действия, переключение и откат по псевдониму:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

El `gateway.route_plan.json` Emitido Captura хосты canonicos/pretty, записывающие устройства проверки работоспособности
на этапах актуализации привязки GAR, очистки кэша и действий по откату. Включено с
артефакты GAR/привязка/переключение перед отправкой камбийского билета, чтобы можно было выполнить операции и
aprobar los mismos pasos con guion.

`scripts/generate-dns-cutover-plan.mjs` импульс этот дескриптор и автоматически выбрасывается
`sorafs-pin-release.sh`. Для регенерации или персонализации вручную:```bash
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

Извлеките дополнительные метаданные с помощью переменных-энторно, прежде чем выбросить помощник-пин-код:

| Переменная | Предложение |
| --- | --- |
| `DNS_CHANGE_TICKET` | ID билета находится в дескрипторе. |
| `DNS_CUTOVER_WINDOW` | Вентиляция переключения ISO8601 (например, `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | Имя хоста производства + авторитарная зона. |
| `DNS_OPS_CONTACT` | Псевдоним дежурного или контактного лица по телефону. |
| `DNS_CACHE_PURGE_ENDPOINT` | Конечная точка очистки зарегистрированного кэша в дескрипторе. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Конверт содержит токен очистки (по умолчанию: `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | Откройте предыдущий дескриптор переключения для метаданных отката. |

Дополнение к JSON к обзору DNS, чтобы апробаторы могли проверить дайджесты манифестов,
привязки псевдонимов и команд-зондов, которые проверяют журналы CI. Флаги CLI
И18НИ00000173Х, И18НИ00000174Х, И18НИ00000175Х,
И18НИ00000176Х, И18НИ00000177Х, И18НИ00000178Х,
`--cache-purge-auth-env`, y `--previous-dns-plan` доказали, что переопределения ошибочны.
когда вы выбрасываете помощника из CI.

## Шаг 8 – Вызовите экран файла зоны сопоставителя (необязательно)

Когда запуск производства будет завершен, сценарий выпуска может быть отправлен
автоматически создается файл зоны SNS и фрагмент резольвера. Отмена регистрации DNS
метаданные через переменные или параметры CLI; Эль помощник Ламара а
`scripts/sns_zonefile_skeleton.py` немедленно после создания дескриптора переключения.
Подтвердите все изменения A/AAAA/CNAME и переварите GAR (BLAKE3-256 твердого груза GAR). Си ла
Зона/имя хоста son conocidos y `--dns-zonefile-out`, если опустить его, записать помощника
`artifacts/sns/zonefiles/<zone>/<hostname>.json` y llena
`ops/soradns/static_zones.<hostname>.json` как фрагмент преобразователя.| Переменная/флаг | Предложение |
| --- | --- |
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | Откройте экран созданного файла зоны. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Вызов фрагмента преобразователя (по умолчанию: `ops/soradns/static_zones.<hostname>.json`, если его опустить). |
| И18НИ00000190Х, И18НИ00000191Х | TTL применяется к созданным реестрам (по умолчанию: 600 секунд). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | Направления IPv4 (окружающая среда отделена от других или повторяется флагом CLI). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | Направления IPv6. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | Целевой CNAME необязательно. |
| И18НИ00000198Х, И18НИ00000199Х | Сосны СПКИ ША-256 (base64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Дополнительные входы TXT (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | Переопределить метку версии вычисляемого файла зоны. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | Введите временную метку `effective_at` (RFC3339) в начало открытия клапана переключения. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | Отменить регистрацию буквального доказательства в метаданных. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | Отменить регистрацию CID в метаданных. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Estado de Freeze de Guardian (мягкое, жесткое, оттаивание, мониторинг, чрезвычайная ситуация). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | Справка о билете опекуна/совета по замораживанию. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | Временная метка RFC3339 для размораживания. |
| И18НИ00000218Х, И18НИ00000219Х | Дополнительные заморозки (отдельные запятые или повторяющийся флаг). |
| И18НИ00000220Х, И18НИ00000221Х | Дайджест BLAKE3-256 (шестнадцатеричный) полезной нагрузки фирмы GAR. Requerido cuando hay привязки шлюза. |

Рабочий процесс GitHub Actions имеет большое значение для секретов репозитория, которые нужно закрепить
автоматическое создание артефактов файла зоны. Настройте важные секреты
(строки могут содержать отдельные списки для многозначных кампаний):

| Секрет | Предложение |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | Имя хоста/зона производства pasados ​​al helper. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | Псевдоним «дежурный по вызову» добавлен в дескриптор. |
| И18НИ00000225Х, И18НИ00000226Х | Зарегистрируйте общедоступный IPv4/IPv6. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | Целевой CNAME необязательно. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Сосны СПКИ base64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | Дополнительные входы TXT. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | Замороженные метаданные, зарегистрированные в электронном виде. |
| `DOCS_SORAFS_GAR_DIGEST` | Перебрать BLAKE3 в шестнадцатеричном формате полезной нагрузки GAR. |

Все различия `.github/workflows/docs-portal-sorafs-pin.yml`, пропорциональны входам
`dns_change_ticket` и `dns_cutover_window` для того, чтобы дескриптор/файл зоны был открыт
корректа. Дехарлос на белом фоне соло, когда он выполняет холостые пробежки.

Тип вызова (совпадает с записной книжкой владельца SN-7):

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
  ...otros flags...
```

Помощник автоматически захватывает билет на вход в формате TXT и передает начало
Включение переключения под меткой времени `effective_at` означает, что оно будет переопределено. Пара эль-Флюхо
Полная работа, версия `docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Обратите внимание на публичное делегирование DNSСкелет файла зоны определяет только авторитарные регистры зоны. Аун
необходимо настроить делегирование NS/DS зоны от вашего регистратора или
Проверьте DNS, чтобы публичный доступ в Интернет мог встретиться с серверами имен.

- Для переключения на вершину/TLD, Псевдоним/ИМЯ США (зависит от поставщика) o
  общедоступные регистры A/AAAA, которые подключаются к любому IP-адресу шлюза.
- Для подчиненных пользователей опубликуйте CNAME, чтобы получить красивое производное хоста.
  (`<fqdn>.gw.sora.name`).
- Канонический хост (`<hash>.gw.sora.id`) постоянно остается в домене шлюза.
  и я не публичен в той зоне, где вы находитесь.

### Установка заголовков шлюза

Помощник по развертыванию также выдает `portal.gateway.headers.txt` y
`portal.gateway.binding.json`, эти артефакты удовлетворяют требованиям DG-3.
привязка содержимого шлюза:

- `portal.gateway.headers.txt` содержит полный блок заголовков HTTP (включая
  `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS, и дескриптор
  `Sora-Route-Binding`) те шлюзы на границе должны быть введены в каждый ответ.
- `portal.gateway.binding.json` Регистрация неправильной информации в виде разборчивой информации для машин
  для того, чтобы билеты камбии и автоматизации можно было сравнить привязки хоста/цида
  распар ла салида де ракушка.

Генерируется автоматически через
`cargo xtask soradns-binding-template`
и захватите псевдоним, дайджест манифеста и имя хоста шлюза, которое есть
пасарон `sorafs-pin-release.sh`. Чтобы обновить или персонализировать блок заголовков,
выброс:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

Pasa `--csp-template`, `--permissions-template`, o `--hsts-template` для переопределения
шаблоны заголовков по умолчанию, если требуются дополнительные указания;
Комбинация с переключателями `--no-*` существует для полного удаления заголовка.

Дополнение фрагмента заголовков к запросу CDN и подачи документа JSON
Все конвейеры автоматизации шлюза для продвижения реального хоста совпадают с
доказательства освобождения.

Сценарий выпуска автоматически выбрасывается помощником по проверке для того, чтобы его
билеты DG-3 siempre incluyan evidencia reciente. Выполните выброс вручную
Когда вы редактируете привязку JSON вручную:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

Команда декодирования полезной нагрузки `Sora-Proof` зашифрована, убедитесь, что метаданные `Sora-Route-Binding`
совпадение с CID манифеста + имя хоста, а также быстрый заголовок, который соответствует указанному.
Архив консолидированной консоли и других артефактов, которые выбрасываются в тот момент, когда они выбрасываются.
Командир CI для проверки изменений DG-3 должен проверить, что привязка действительно действительна
перед переключением.> **Интеграция дескриптора DNS:** `portal.dns-cutover.json` сейчас внесен в раздел
> `gateway_binding` отвечает на эти артефакты (руты, CID контента, состояние доказательства и
> шаблонный литерал заголовков) **y** одна строка `route_plan` ссылка
> `gateway.route_plan.json` основные шаблоны заголовков и откат. Включить Эсос
> Блокируйте каждый билет камбио DG-3 для того, чтобы ревизоры можно было сравнить с ценностями
> точные данные `Sora-Name/Sora-Proof/CSP` и подтверждение планов продвижения/отката
> Совпало с пакетом доказательств без открытия архива сборки.

## Пасо 9 - Ejecuta контролирует публикацию

Элемент дорожной карты **DOCS-3c** требует продолжения подтверждения того, что портал, прокси-сервер Попробуйте и лос
привязки шлюза сохраняются после освобождения. Выброс консолидированного монитора
немедленно после 7–8 лет после Лос-Пасоса и подключения к нашим программным зондам:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` загрузить архив конфигурации (версия
  `docs/portal/docs/devportal/publishing-monitoring.md` для этой задачи) y
  Выполнение трех проверок: проверка путей портала + проверка CSP/Permissions-Policy,
  зондирует прокси-сервер. Попробуйте (дополнительно выберите конечную точку `/metrics`), и проверьте
  привязка шлюза (`cargo xtask soradns-verify-binding`), которая сейчас нужна и доступна
  Доблесть успешно выполнила объединение Sora-Content-CID с проверками псевдонима/манифеста.
- Конечная точка команды с ненулевым значением, когда проверка выполняется для CI, заданий cron или операторов.
  Runbook может задержать выпуск перед рекламным псевдонимом.
- Pasar `--json-out` напишите возобновленный JSON Unico с заданным целевым значением; `--evidence-dir`
  излучают `summary.json`, `portal.json`, `tryit.json`, `binding.json` и `checksums.sha256` для того, что
  Изменения в управлении можно сравнить с полученными результатами и повторно извлечь мониторы. Архив
  этот каталог bajo `artifacts/sorafs/<tag>/monitoring/` в комплекте Sigstore и el
  дескриптор переключения DNS.
- Включите удаление монитора, экспорт Grafana (`dashboards/grafana/docs_portal.json`),
  и идентификатор тренировки Alertmanager и билет выпуска для SLO DOCS-3c можно использовать
  аудитадо луэго. El playbook, посвященный мониторингу публикации, вживую
  `docs/portal/docs/devportal/publishing-monitoring.md`.

Для проверки портала требуется HTTPS и повторная база URL-адресов `http://`, а затем `allowInsecureHttp`.
эта конфигурация в конфигурации монитора; Мантен нацелен на продюсирование/постановку в TLS и соло
навык переопределения локалей предварительного просмотра.

Автоматизация монитора через `npm run monitor:publishing` в Buildkite/cron на любом портале
este en vivo. Командир, нажав на URL-адреса продукции, продайте чеки на продажу.
SRE/Docs продолжают использовать все выпуски.

## Автоматизация с `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` содержит Пасос 2-6. Эсте:1. архив `build/` в детерминированном архиве,
2. эжекта `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`,
   у `proof verify`,
3. Дополнительный выброс `manifest submit` (включая привязку псевдонима) cuando hay credenciales
   Torii, y
4. пропишите `artifacts/sorafs/portal.pin.report.json`, дополнительно
  `portal.pin.proposal.json`, дескриптор переключения DNS (после отправки),
  и пакет привязки шлюза (`portal.gateway.binding.json`, кроме блока заголовков)
  для того, чтобы оборудование управления, сетевое взаимодействие и возможности сравнения пакета доказательств
  грех пересматривать журналы CI.

Конфигурация `PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME`, y (опционально)
`PIN_ALIAS_PROOF_PATH` перед вызовом сценария. США `--skip-submit` для сухих условий
бежит; Рабочий процесс GitHub описывает его альтернативно через ввод `perform_submit`.

## Шаг 8 – специальные публикации OpenAPI и пакеты SBOM

DOCS-7 требует сборки портала, спецификации OpenAPI и артефактов SBOM, доступных
трубопровод por el mismo определен. Лос-хелперс экзистентес кубрен лос трес:

1. **Регенерация и конкретизация.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   Выполните этику выпуска через `--version=<label>`, когда нужно сохранить снимок
   исторический (например, `2025-q3`). El helper описать снимок ru
   `static/openapi/versions/<label>/torii.json`, вот здесь
   `versions/current`, и регистрация метаданных (SHA-256, состояние манифеста, y
   временная метка актуализирована) в `static/openapi/versions.json`. Портал десарролладорес
   Этот указатель для панелей Swagger/RapiDoc позволяет выбрать версию
   и мой дайджест/фирма asociado en linea. Omitir `--version` храните этикетки
   Release Previo нетронутым и соло Refresca los Punteros `current` + `latest`.

   Манифест захвата обрабатывает SHA-256/BLAKE3, чтобы можно было зафиксировать шлюз
   заголовки `Sora-Proof` для `/reference/torii-swagger`.

2. **Создавайте SBOM CycloneDX.** Конвейер выпуска позволяет SBOM базироваться в исходном коде.
   Сегун `docs/source/sorafs_release_pipeline_plan.md`. Мантен-ла-Салида
   вместе с артефактами сборки:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **Empaqueta cada payload en un CAR.**

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

   Sigue los mismos pasos de `manifest build` / `manifest sign`, что является основным местом,
   задать псевдоним для артефакта (например, `docs-openapi.sora` для спецификации и
   `docs-sbom.sora` для фирменного комплекта SBOM). Псевдоним Mantener, отличающийся от Mantiene SoraDNS,
   GAR и билеты отката точно соответствуют полезной нагрузке.

4. **Отправить и привязать.** Повторно использовать авторизованный существующий пакет Sigstore, предварительно зарегистрировав его.
   Кортеж псевдонимов и контрольный список выпуска для того, чтобы аудиторы были растрированы под именем Сора
   отобразите то, что дайджест манифеста.

Архивируйте спецификации/SBOM вместе со сборкой портала, гарантируя, что каждый билет будет
выпустите полный набор артефактов и повторно извлеките упаковщик.

### Помощник по автоматизации (скрипт CI/пакета)`./ci/package_docs_portal_sorafs.sh` Код Пасоса 1–8 для пункта дорожной карты
**DOCS-7** можно использовать в одиночку. Эль помощник:

- требуется подготовка портала (`npm ci`, синхронизация OpenAPI/norito, тесты виджетов);
- эмитируйте CAR и части манифестов портала, OpenAPI и SBOM через `sorafs_cli`;
- дополнительный выброс `sorafs_cli proof verify` (`--proof`) и фирма Sigstore
  (И18НИ00000315Х, И18НИ00000316Х, И18НИ00000317Х);
- deja todos los artefactos bajo `artifacts/devportal/sorafs/<timestamp>/` y
  напишите `package_summary.json` для того, чтобы CI/инструменты выпуска можно было включить в пакет; й
- Обновление `artifacts/devportal/sorafs/latest` для быстрого извлечения полученного сообщения.

Пример (полный конвейер с Sigstore + PoR):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

Флаги conocer:

- `--out <dir>` - переопределить корень артефактов (по умолчанию указывается временная метка).
- `--skip-build` - повторное использование существующего `docs/portal/build` (при использовании CI без возможности
  реконструировать зеркала в автономном режиме).
- `--skip-sync-openapi` - опустить `npm run sync-openapi` cuando `cargo xtask openapi`
  нет возможности открыть crates.io.
- `--skip-sbom` - evita llamar a `syft`, когда бинарный файл не установлен (el script imprime
  una advertencia en su lugar).
- `--proof` - выброс `sorafs_cli proof verify` для каждого CAR/манифеста. Лос-полезная нагрузка де
  требуется несколько архивов, поддерживающих план фрагментов в CLI, если у вас есть этот флаг
  грех установить, если обнаружены ошибки `plan chunk count` и проверено руководство, когда
  llegue el ворота вверх по течению.
- `--sign` - вызов `sorafs_cli manifest sign`. Докажите, что это обман
  `SIGSTORE_ID_TOKEN` (или `--sigstore-token-env`) или узнать, что CLI используется при использовании
  `--sigstore-provider/--sigstore-audience`.

Куандо завидует артефактам производства США `docs/portal/scripts/sorafs-pin-release.sh`.
Заполните портал, OpenAPI и полезные данные SBOM, фирменное сообщение и регистрацию метаданных.
дополнительные активы в `portal.additional_assets.json`. Помощник по установке дополнительных ручек
используются для упаковщика CI для новых переключателей `--openapi-*`, `--portal-sbom-*`, y
`--openapi-sbom-*` для назначения кортежей псевдонимов для артефактов, переопределения действующего SBOM через
`--openapi-sbom-source`, опустить полезные нагрузки (`--skip-openapi`/`--skip-sbom`),
и установите бинарный файл `syft` без значения по умолчанию с `--syft-bin`.

Сценарий показывает каждую команду, которую нужно выбросить; скопировать журнал и билет выпуска
объединение с `package_summary.json`, чтобы можно было сравнить версии CAR, метаданные
план, и хэши пакета Sigstore без повторной проверки оболочки ad hoc.

## Шаг 9 – проверка шлюза + SoraDNS

Прежде чем объявить о переключении, проверьте новый псевдоним через SoraDNS и шлюзы
энграпан прюбас фреска:

1. **Извлеките ворота зонда.** `ci/check_sorafs_gateway_probe.sh` ejercita
   `cargo xtask sorafs-gateway-probe` против демонстрационных приборов ru
   `fixtures/sorafs_gateway/probe_demo/`. Для того, чтобы проверить имя хоста, выполните следующие действия:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```Декодированный зонд `Sora-Name`, `Sora-Proof`, и второй `Sora-Proof-Status`
   `docs/source/sorafs_alias_policy.md` и упал, когда дайджест манифеста,
   TTL или привязки GAR были разработаны.

   Для облегченных выборочных проверок (например, когда только пакет переплета
   изменилось), запустите `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   Помощник проверяет захваченный пакет привязок и удобен для выпуска.
   билеты, для которых требуется только обязательное подтверждение вместо полной проверки.

2. **Доказательство тренировок.** Для тренировок оператора или имитации PagerDuty, оберните
   эль-зонд с `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario
   devportal-rollout -- ...`. Обертка для заголовков/журналов bajo
   `artifacts/sorafs_gateway_probe/<stamp>/`, актуализируется `ops/drill-log.md`, y
   (необязательно) различные способы отката или полезные данные PagerDuty. Эстаблес
   `--host docs.sora` для проверки рута SoraDNS и жесткого кодирования IP-адреса.

3. **Привязка DNS к проверке.** При публичном управлении доказательством псевдонима и регистрации архива GAR
   ссылка на зонд (`--gar`) и дополнение к доказательствам освобождения. Владельцы решателя
   можно отразить ввод неправильного значения через `tools/soradns-resolver`, чтобы гарантировать, что входы в кэш
   respeten el manifico nuevo. Прежде чем добавить JSON, выведите
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   для определения карты хоста, метаданных манифеста и этикеток телеметрии
   действительны в автономном режиме. Помощник может возобновить работу `--json-out` вместе с фирмой GAR для того, чтобы
   пересматривает теганские доказательства, поддающиеся проверке, без нарушения бинарио.
  Куандо редактирует новый GAR, prefiere
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (vuelve a `--manifest-cid <cid>` соло, когда архив манифестов не доступен). Эль помощник
  сейчас выводится CID **y** дайджест BLAKE3 непосредственно из манифеста JSON, записывается на белом фоне,
  флаги дедупликации `--telemetry-label`, повторяющиеся действия, соблюдение правил этикета и создание шаблонов по умолчанию.
  CSP/HSTS/Permissions-Policy перед написанием JSON для определения полезной нагрузки
  в том числе, когда операторы захватывают этикеты для отдельных раковин.

4. **Наблюдайте за метриками псевдонимов.** Мантен `torii_sorafs_alias_cache_refresh_duration_ms`
   y `torii_sorafs_gateway_refusals_total{profile="docs"}` на подставке под зондом
   корр; Серия ambas представлена ​​графическими изображениями на `dashboards/grafana/docs_portal.json`.

## Пасо 10 — Monitoreo и комплект доказательств- **Панели мониторинга.** Экспорт `dashboards/grafana/docs_portal.json` (SLO портала),
  `dashboards/grafana/sorafs_gateway_observability.json` (задержка шлюза +
  Привет де доказательство), y `dashboards/grafana/sorafs_fetch_observability.json`
  (приветствие оркестратора) для каждого выпуска. Дополнительная информация по экспорту JSON в билет
  Release для того, чтобы эти изменения могли воспроизвести запросы Prometheus.
- **Архивы проб.** Сохранение `artifacts/sorafs_gateway_probe/<stamp>/` в git-приложении.
  о твое ведро доказательств. Включите резюме зонда, заголовки и полезную нагрузку PagerDuty.
  захватить сценарий телеметрии.
- **Пакет выпуска.** Защитите резюме CAR портала/SBOM/OpenAPI, пакеты манифестов,
  фирмы Sigstore, `portal.pin.report.json`, журналы проверки Try-It, отчеты о проверке связи
  только одна ковровая дорожка с отметкой времени (например, `artifacts/sorafs/devportal/20260212T1103Z/`).
- **Журнал бурения.** Когда зонды были частью сверла, я почувствовал, что
  `scripts/telemetry/run_sorafs_gateway_probe.sh` объединяется с `ops/drill-log.md`
  для того, чтобы удовлетворить требования, предъявляемые к SNNet-5.
- **Ссылки на билет.** Ссылка на идентификаторы панели Grafana или экспорт дополнительных файлов PNG в билет.
  Камбио, работа с рутиной отчета о расследовании, для того, чтобы ревизоры могли пройти проверку лос SLO
  грех доступа к оболочке.

## Пасо 11 — Учения по извлечению данных из нескольких источников и доказательств на табло

Опубликовано в SoraFS, сейчас требуются доказательства выборки из нескольких источников (DOCS-7/SF-6)
junto a las Pruebas de DNS/gateway anteriores. После появления манифеста:

1. **Ejecuta `sorafs_fetch` против живого проявления.** Используйте артефакты плана/манифеста.
   производятся в Лос-Пасосе 2–3 раза, если у вас есть удостоверения шлюза, выданные для каждого подтверждения. упорствовать
   чтобы выполнить задание, чтобы аудиторы могли воспроизвести растроку решения оркестратора:

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

   - Покажите первые рекламные объявления, ссылающиеся на манифесты (например,
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     и прошел через `--provider-advert name=path`, чтобы табло можно было оценить
     детерминированная способность детерминированной формы. США
     `--allow-implicit-provider-metadata` **solo** cuando воспроизводит приборы в режиме CI; лос дриллы
     производство должно цитировать рекламные фирмы, которые рассказывают о булавке.
   - Когда манифест ссылается на дополнительные регионы, повторите команду с кортежами
     проверяйте корреспонденцию для того, чтобы каждый кэш/псевдоним был артефактом ассоциированной выборки.

2. **Архив las salidas.** Guarda `scoreboard.json`,
   `providers.ndjson`, `fetch.json`, y `chunk_receipts.ndjson` на ковре доказательств
   выпуск. Этот архив захватывает взвешивание одноранговых узлов, повторную попытку бюджета, задержку EWMA и т. д.
   квитанции о том, что пакет управления должен быть сохранен для SF-7.

3. **Актуализация телеметрии.** Импорт данных выборки на приборную панель **SoraFS Fetch
   Наблюдаемость** (`dashboards/grafana/sorafs_fetch_observability.json`),
   наблюдайте `torii_sorafs_fetch_duration_ms`/`_failures_total` и панели ранго-пор-проверяющего
   парааномалии. Откройте снимки панели Grafana и билет на выпуск вместе с рутиной
   табло.4. **Курить в соответствии с правилами оповещения.** Выброс `scripts/telemetry/test_sorafs_fetch_alerts.sh`
   для проверки пакета оповещений Prometheus перед выпуском. Adjunta la salida de
   promtool al Ticket для проверки DOCS-7, подтверждающей оповещения о остановке
   медленный провайдер сигуен армады.

5. **Кабель в CI.** Рабочий процесс вывода портала с помощью `sorafs_fetch` отделяет ввод
   И18НИ00000390Х; умеющий создавать инсценировки/продукцию для доказывания
   получить пакет документов, созданный вместе с руководством по вмешательству. Лос-буры локали пуеден
   повторно использовать сценарий Missmo, экспортировать токены шлюза и установить `PIN_FETCH_PROVIDERS`
   а-ля список проверяющих отдельных лиц.

## Продвижение, наблюдение и откат

1. **Промоакция**: отдельные псевдонимы для постановки и производства. Повторная рекламная акция
   `manifest submit` с манифестом/пакетом, камбиандо
   `--alias-namespace/--alias-name`, чтобы добавить псевдоним производства. Эсто Эвита
   реконструировать или повторно утвердить то, что обеспечивает контроль качества на этапе подготовки.
2. **Монитор:** импортируйте панель регистрации контактов.
   (`docs/source/grafana_sorafs_pin_registry.json`) для отдельных зондов портала
   (версия `docs/portal/docs/devportal/observability.md`). Оповещение о дрейфе контрольных сумм,
   исследует провалы или пики повторной попытки доказательства.
3. **Откат:** чтобы вернуться, повторно откройте предыдущий манифест (или удалите фактический псевдоним) с помощью
   `sorafs_cli manifest submit --alias ... --retire`. Пакет Manten siempre el ultimo
   conocido как буэно и возобновленное CAR, чтобы откат можно было повторить, если
   Лос-логи CI вращаются.

## Планирование рабочего процесса CI

Как минимум, ваш трубопровод должен быть:

1. Сборка + проверка (`npm ci`, `npm run build`, генерация контрольных сумм).
2. Empaquetar (`car pack`) и компьютерные манифесты.
3. Фирменно использовать токен OIDC для задания (`manifest sign`).
4. Subir artefactos (CAR, манифест, связка, план, резюме) для аудитории.
5. Отправьте реестр контактов:
   - Запросы на извлечение -> `docs-preview.sora`.
   - Теги / ветки защиты -> продвижение псевдонима производства.
6. Зонды выброса + ворота проверки доказательства перед завершением.

`.github/workflows/docs-portal-sorafs-pin.yml` подключите все эти документы для выпуска руководств.
Эль рабочий процесс:

- создать/проверить портал,
- пакет сборки через `scripts/sorafs-pin-release.sh`,
- фирма/проверка пакета манифестов с использованием GitHub OIDC,
- подключайте CAR/manificesto/bundle/plan/resumenes как артефакты, y
- (необязательно) envia el manifico + псевдоним, связывающий cuando hay secrets.

Настройте секреты/переменные важного репозитория перед изменением задания:| Имя | Предложение |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | Хост Torii, который экспонирует `/v1/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | Идентификатор эпохи регистрации поданных заявок. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | Разрешение на подачу заявления. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | Кортеж псевдонимов, который манифестируется, когда `perform_submit` es `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Пакет проверки псевдонима, кодированного в base64 (необязательно; опустите для привязки альтернативного псевдонима). |
| `DOCS_ANALYTICS_*` | Конечные точки аналитики/исследования существуют, повторно используются в других рабочих процессах. |

Отображение рабочего процесса через пользовательский интерфейс действий:

1. Provee `alias_label` (например, `docs.sora.link`), `proposal_alias` дополнительно,
   и вы можете переопределить необязательный параметр `release_tag`.
2. Дежа `perform_submit` без маркировки для создания артефактов без tocar Torii
   (используется для пробных прогонов) или может быть использован для прямой публикации или настройки псевдонима.

`docs/source/sorafs_ci_templates.md` в качестве документа по общим помощникам CI для
Проекты из этого хранилища, но рабочий процесс портала должен быть предпочтительным вариантом
для освобождения дель-диа-диа.

## Контрольный список

- [ ] `npm run build`, `npm run test:*`, y `npm run check:links` estan en verde.
- [ ] `build/checksums.sha256` и `build/release.json` захвачены и артефакты.
- [ ] АВТОМОБИЛЬ, план, декларация и возобновление создания бахо `artifacts/`.
- [ ] Bundle Sigstore + отдельные записи с журналами.
- [ ] `portal.manifest.submit.summary.json` y `portal.manifest.submit.response.json`
      capturados cuando hay представления.
- [ ] `portal.pin.report.json` (и дополнительно `portal.pin.proposal.json`)
      Объединенный архив с артефактами CAR/manifico.
- [ ] Журналы архивов `proof verify` и `manifest verify-signature`.
- [ ] Актуализированные информационные панели Grafana + тестовые тесты Try-It.
- [ ] Примечания об откате (ID предыдущего манифеста + дайджест псевдонима) дополнительные функции
      билет на выпуск.