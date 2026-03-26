---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## Визао гераль

Это руководство по преобразованию ваших планов действий **DOCS-7** (опубликовано SoraFS) и **DOCS-8**
(автоматический вывод CI/CD) в процессе действий для портала разработчиков.
Приступите к этапу сборки/подборки, или установите SoraFS, сбор манифестов с Sigstore,
продвижение псевдонима, проверка и инструкции по откату для предварительного просмотра и выпуска каждого файла
сейчас воспроизводство и аудит.

Предположим, что вы говорили о двоичном файле `sorafs_cli` (созданном из `--features cli`), доступ к
конечная точка Torii с разрешениями на регистрацию контактов и учетными данными OIDC для Sigstore. Охрана сегредос
долгое время (`IROHA_PRIVATE_KEY`, `SIGSTORE_ID_TOKEN`, токены Torii) в своем хранилище CI; как
execucoes locais podem carrega-los a partir de Export do Shell.

## Предварительные требования

- Узел 18.18+ com `npm` или `pnpm`.
- `sorafs_cli` и часть `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- URL-адрес Torii, который выставляет `/v1/sorafs/*`, чтобы получить конфиденциальную информацию/задать приватность авторизации, чтобы можно было отправить манифесты и псевдонимы.
— Эмиссор OIDC (GitHub Actions, GitLab, удостоверение рабочей нагрузки и т. д.) для эмиттера `SIGSTORE_ID_TOKEN`.
- Необязательно: `examples/sorafs_cli_quickstart.sh` для запуска и `docs/source/sorafs_ci_templates.md` для формирования рабочих процессов GitHub/GitLab.
- Настройте переменные OAuth de Tre it (`DOCS_OAUTH_*`) и выполните
 [Контрольный список по усилению безопасности](./security-hardening.md) перед продвижением сборки
 Фуэра дель лаборатории. При построении портала сейчас произойдет падение, когда эти переменные будут изменены
 o когда ручки TTL/опроса будут активированы в качестве приложения; экспорт
 `DOCS_OAUTH_ALLOW_INSECURE=1` доступны для предварительного просмотра доступных языков. Приложение а
 доказательство пен-теста на выпускном билете.

## Этап 0 — Captura um paquete del proxe Tre it

Прежде чем продвигать предварительный просмотр Netlife или шлюза, продайте его как Fuentes del Proxe Tre it e o
дайджест манифеста OpenAPI, закрепленный в определенном пакете:

```bash
cd docs/portal
npm run release:tryit-proxe -- \
 --out ../../artifacts/tryit-proxy/$(date -u +%E%m%dT%H%M%SZ) \
 --target https://torii.dev.sora \
 --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` копирование помощников прокси/зонда/отката, проверка и ассинатура
OpenAPI и пропишите `release.json` вместо `checksums.sha256`. Приложение является пакетом билетов
Промокао Netlify/SoraFS шлюз для того, чтобы все исправления могли воспроизводиться как точные
прокси и как точки цели Torii, которые нужно реконструировать. О пакете регистрации, если он есть
носители провистов для клиентов, обладающих навыками (`allow_client_auth`) для плана
развертывание и синхронизация согласно правилам CSP.

## Этап 1 — Создание портала

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

`npm run build` автоматически выполняет `scripts/write-checksums.mjs`, создавая:

- `build/checksums.sha256` — манифест SHA256, подходящий для `sha256sum -c`.
- `build/release.json` - метаданные (`tag`, `generated_at`, `source`) появляются в каждом CAR/манифесте.

Архивируйте архивы вместе с резюме CAR, чтобы можно было проверить артефакты
предварительный просмотр, реконструкция.

## Этап 2 – Эмпакета оставшихся активовВыполните установку CAR против каталога Docusaurus. О, пример абахо
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

Резюме JSON захватывает фрагменты, дайджесты и точки планирования доказательств, которые
`manifest build` и панели мониторинга CI повторно используются.

## Этапа 2b - Компаньоны Empaqueta OpenAPI e SBOM

DOCS-7 требует публикации местоположения портала, моментального снимка OpenAPI и полезных данных ОС SBOM.
какие манифесты различаются для шлюзов, которые можно соединить с заголовками
`Sora-Proof`/`Sora-Content-CID` для каждого артефакта. О помощник освобождения
(`scripts/sorafs-pin-release.sh`) в папке OpenAPI
(`static/openapi/`) и SBOM, излучаемые через `syft`, в отдельных CAR
`openapi.*`/`*-sbom.*` и регистрация метаданных в них
`artifacts/sorafs/portal.additional_assets.json`. Al executar или Fluxo руководство,
Повторите этапы 2–4 для каждой полезной нагрузки с использованием префиксов и этикеток метаданных.
(пример `--car-out "$OUT"/openapi.car`, но
`--metadata alias_label=docs.sora.link/openapi`). Registra Cada по манифесту/псевдониму
em Torii (sitio, OpenAPI, SBOM del portal, SBOM de OpenAPI) перед установкой DNS для этого
o шлюзы могут быть использованы для доступа ко всем публичным артефактам.

## Этап 3 – Составление манифеста

```bash
sorafs_cli manifest build \
 --summare "$OUT"/portal.car.json \
 --manifest-out "$OUT"/portal.manifest.to \
 --manifest-json-out "$OUT"/portal.manifest.json \
 --pin-min-replicas 5 \
 --pin-storage-class warm \
 --pin-retention-epoch 14 \
 --metadata alias_label=docs.sora.link
```

Отрегулируйте политические флаги, которые будут закреплены за следующим выпуском (например, `--pin-storage-class
горячие пара канареек). Вариант JSON является дополнительным, но удобным для проверки кода.

## Этапа 4 - Ассинатура ком Sigstore

```bash
sorafs_cli manifest sign \
 --manifest "$OUT"/portal.manifest.to \
 --chunk-plan "$OUT"/portal.plan.json \
 --bundle-out "$OUT"/portal.manifest.bundle.json \
 --signature-out "$OUT"/portal.manifest.sig \
 --identity-token-provider github-actions \
 --identity-token-audience sorafs-devportal
```

Регистрация пакета или дайджест манифеста, дайджесты фрагментов и хэш BLAKE3 токена
OIDC сохраняется в JWT. Guarda tanto или package como a assinatura separada; как промоционы де
производство может повторно использовать самые важные артефакты для повторного использования. Как execucoes localis
можно повторно установить флаги провайдера com `--identity-token-env` (o establecer
`SIGSTORE_ID_TOKEN` em o entorno) quando um helper OIDC внешний эмитент или токен.

## Этапа 5 - Зависть от реестра контактов

Зависть от фирменного манифеста (или плана блоков) Torii. Пожалуйста, попросите резюме для вас
результат входа/псевдонима будет проверен.

```bash
sorafs_cli manifest submit \
 --manifest "$OUT"/portal.manifest.to \
 --chunk-plan "$OUT"/portal.plan.json \
 --torii-url "$TORII_URL" \
 --authorite <katakana-i105-account-id> \
 --private-kee "$IROHA_PRIVATE_KEY" \
 --submitted-epoch 20260101 \
 --alias-namespace docs \
 --alias-name sora.link \
 --alias-proof "$OUT"/docs.alias.proof \
 --summary-out "$OUT"/portal.submit.json \
 --response-out "$OUT"/portal.submit.response.json
```

Когда вы увидите псевдоним предварительного просмотра или canare (`docs-preview.sora`), повторите
Отправьте нам уникальный псевдоним, чтобы QA мог проверить контент перед рекламой
производство.

Для привязки псевдонима требуются три поля: `--alias-namespace`, `--alias-name` и `--alias-proof`.
Управление производит пакет доказательств (base64 или байты Norito), когда вы запрашиваете запрос
дель псевдоним; охраняйте отдельные части CI и демонстрируйте их как архив до вызова `manifest submit`.
Флаги псевдонимов устанавливаются, когда открываются точки доступа или манифест в DNS.

## Этапа 5b — Общие вопросы управления

Манифест Cada debe viajar comuma propuesta lista para Parliament para que cualquier ciudadano
Мы можем ввести или предоставить вам привилегированные полномочия. Депуа-де-ос-пасос
отправить/подписать, выполнить:

```bash
sorafs_cli manifest proposal \
 --manifest "$OUT"/portal.manifest.to \
 --chunk-plan "$OUT"/portal.plan.json \
 --submitted-epoch 20260101 \
 --alias-hint docs.sora.link \
 --proposal-out "$OUT"/portal.pin.proposal.json
````portal.pin.proposal.json` захватывает инструкцию Canonica `RegisterPinManifest`,
o переваривать куски, политику и псевдонимы. Дополнительный билет на управление или все
Портал Парламента позволяет делегатам сравнивать полезную нагрузку и реконструировать артефакты.
Если у вас есть команда, которая находится в авторизованном ключе Torii, вы можете изменить ее.
местное пропуэста.

## Этап 6 – Проверка подлинности и телеметрии

После проверки выполните определенные этапы проверки:

```bash
sorafs_cli proof verife \
 --manifest "$OUT"/portal.manifest.to \
 --car "$OUT"/portal.car \
 --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
 --manifest "$OUT"/portal.manifest.to \
 --bundle "$OUT"/portal.manifest.bundle.json \
 --chunk-plan "$OUT"/portal.plan.json
```

- Verifique `torii_sorafs_gateway_refusals_total` е
 `torii_sorafs_replication_sla_total{outcome="missed"}` для аномалий.
- Выполните команду `npm run probe:portal` для запуска пробной версии или прокси-сервера и зарегистрированных ссылок ОС.
 contra o contenido recien fijado.
- Получение доказательств описания описанного монитора.
 [Публикация и мониторинг](./publishing-monitoring.md) для доступа к наблюдаемому DOCS-3c
 se satisfaga junto на этапах публичной деятельности. О помощник, ахора acepta Multiples entradas
 `bindings` (место, OpenAPI, SBOM портала, SBOM de OpenAPI) и приложение `Sora-Name`/`Sora-Proof`/`Sora-Content-CID`
 их хост должен быть установлен через дополнительную защиту `hostname`. A invocacao de abajo описывает tanto um
 Единое резюме JSON в виде пакета доказательств (`portal.json`, `tryit.json`, `binding.json` и
 `checksums.sha256`) в каталоге выпуска:

 ```bash
 npm run monitor:publishing -- \
 --config ../../configs/docs_monitor.json \
 --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
 --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
 ```

## Etapa 6a — Сертификаты шлюза

Получение плана SAN/вызова TLS перед созданием пакетов GAR для оборудования шлюзов и ОС
Специалисты DNS пересматривают одно доказательство. Новый помощник при отображении автоматических чернил
В списке DG-3 указаны канонические хосты с подстановочными знаками, сети SAN с красивыми хостами, этикетки DNS-01 и рекомендации ACME:

```bash
cargo xtask soradns-acme-plan \
 --name docs.sora \
 --json-out artifacts/sorafs/portal.acme-plan.json
```

Скомитет или объединение JSON со всеми выпускаемыми пакетами (или субэло с камбийским билетом) для операторов
Вы можете использовать значения SAN в конфигурации `torii.sorafs_gateway.acme` от Torii и вносить изменения в ОС
GAR может подтвердить канонические/представленные карты, чтобы повторно выполнить производные хоста. Агрега
Аргументы `--name` являются дополнительными для каждого успешного продвижения или последующего выпуска.

## Etapa 6b – Получение карт хоста canonicos

Перед использованием полезных данных тамплиеров GAR необходимо зарегистрировать или определить карту хоста для каждого псевдонима.
`cargo xtask soradns-hosts` hashea cada `--name` в соответствии с каноническим этикетом
(`<base32>.gw.sora.id`), выдайте или запросите подстановочный знак (`*.gw.sora.id`) и получите собственный хост
(И18НИ00000150Х). Продолжайте сохранять артефакты выпуска для внесения изменений.
DG-3 может сравнить карту с отправкой GAR:

```bash
cargo xtask soradns-hosts \
 --name docs.sora \
 --json-out artifacts/sorafs/portal.canonical-hosts.json
```

США `--verify-host-patterns <file>` для быстрого падения, когда JSON-де-GAR или привязка шлюза
опустите один требуемый хост. О помощник, принимающий несколько архивов проверки, haciendo
можно легко перейти к установке GAR как `portal.gateway.binding.json` и вставить их в простой вызов:

```bash
cargo xtask soradns-hosts \
 --name docs.sora \
 --json-out artifacts/sorafs/portal.canonical-hosts.json \
 --verify-host-patterns artifacts/sorafs/portal.gar.json \
 --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```Приложение к резюме в формате JSON и журнал проверки билета DNS/шлюза для него
аудиторы могут подтвердить канонические хосты, подстановочные знаки и выполнить повторные запуски сценариев.
Повторно выполните команду, когда вы объедините новый псевдоним в пакете для актуализации GAR.
здесь есть mesma Evidencia.

## Этап 7 — Общий дескриптор переключения DNS

Производственные обрезки требуют проверки пакета камбио. Depois de um submit exitoso
(привязка псевдонима), o helper emite
`artifacts/sorafs/portal.dns-cutover.json`, захвачено:

- метаданные привязки псевдонима (пространство имен/имя/доказательство, дайджест манифеста, URL Torii,
 эпоха enviado, autoridad);
- контекст выпуска (тег, метка псевдонима, маршруты манифеста/CAR, план фрагментов, пакет Sigstore);
- punteros de verificacao (командный зонд, псевдоним + конечная точка Torii); е
- дополнительные поля контроля сдачи (идентификатор билета, выход на переключение, контактные операции,
 имя хоста/зона производства);
- метаданные рекламного ролика, полученные из заголовка `Sora-Route-Binding`
 (хост canonico/CID, правила заголовка + привязка, команды проверки), убедитесь, что реклама
 GAR и отработка резервных ссылок для получения дополнительных доказательств;
- сгенерированные артефакты плана маршрута (`gateway.route_plan.json`,
 шаблоны заголовков и дополнительные заголовки отката) для камбийных билетов и перехватчиков
 Проверьте, может ли CI проверить каждый пакет ссылок DG-3 на планы продвижения/отката.
 канонические до апробации;
- Дополнительные метаданные недействительности кэша (конечная точка очистки, переменная аутентификации, полезная нагрузка JSON,
 Пример команды `curl`); е
- подсказки по откату к предыдущему дескриптору (тег выпуска и дайджест манифеста)
 для того, чтобы захваченные билеты были детерминированными, резервный путь.

Когда требуется освобождение кэша, общий канонический план вместе с дескриптором отключения:

```bash
cargo xtask soradns-cache-plan \
 --name docs.sora \
 --path / \
 --path /gateway/manifest.json \
 --auth-header Authorization \
 --auth-env CACHE_PURGE_TOKEN \
 --json-out artifacts/sorafs/portal.cache_plan.json
```

Приложение или `portal.cache_plan.json`, полученное в пакете DG-3 для ваших операций
детерминированные хосты/пути (подсказки аутентификации, которые совпадают) и запросы эмитента `PURGE`.
Дополнительный дополнительный кэш дескриптора, который может ссылаться на этот архив напрямую,
поддерживайте внесенные изменения в контроль изменений с точностью до тех пор, пока конечные точки будут неактивными.
во время переключения.

Для каждого пакета DG-3 также необходим контрольный список продвижения + откат. Генерала через
`cargo xtask soradns-route-plan` для того, чтобы внесенные изменения в контроль изменений могли быть выполнены в дальнейшем
Точные предполетные операции, переключение и откат под псевдонимом:

```bash
cargo xtask soradns-route-plan \
 --name docs.sora \
 --json-out artifacts/sorafs/gateway.route_plan.json
```

O `gateway.route_plan.json` испускает хосты captura canonicos/pretty, записывающие устройства проверки работоспособности
На этапах актуализируются привязки GAR, очищаются кэши и выполняются действия по откату. Incluyelo com
наши артефакты GAR/привязка/переключение перед отправкой или камбионным билетом, чтобы можно было выполнить операцию и
aprobar os mesmos etapas com guion.

`scripts/generate-dns-cutover-plan.mjs` импульсный дескриптор, который автоматически выполняется
`sorafs-pin-release.sh`. Для регенерации или персонализации вручную:

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
```Измените дополнительные метаданные с помощью переменных-энторно перед исполнителем или помощника-пина:

| Переменная | Предложение |
| --- | --- |
| `DNS_CHANGE_TICKET` | Зашифрованный идентификатор билета или дескриптор. |
| `DNS_CUTOVER_WINDOW` | Вентиляция переключения ISO8601 (например, `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | Имя хоста производства + авторитарная зона. |
| `DNS_OPS_CONTACT` | Псевдоним дежурного или контактного лица по телефону. |
| `DNS_CACHE_PURGE_ENDPOINT` | Конечная точка очистки кэша, зарегистрированная в дескрипторе. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Конверт, содержащий токен очистки (по умолчанию: `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | Откройте предыдущий дескриптор переключения для метаданных отката. |

Приложение или JSON к обзору DNS для проверки дайджестов манифеста,
привязки псевдонимов и команды проверки производят проверку журналов CI. Флаги CLI
И18НИ00000173Х, И18НИ00000174Х, И18НИ00000175Х,
И18НИ00000176Х, И18НИ00000177Х, И18НИ00000178Х,
`--cache-purge-auth-env`, e `--previous-dns-plan` проверены переопределения ОС mesmos
Когда будет выполнен помощник CI.

## Этап 8 – создать или открыть экран файла зоны сопоставителя (необязательно)

Когда происходит переключение производства, сценарий выпуска может быть отправлен
автоматически создается файл зоны SNS и фрагмент резольвера. Порядок регистрации нужных DNS
метаданные через переменные или параметры CLI; о помощница ламара а
`scripts/sns_zonefile_skeleton.py` немедленно генерируется или дескриптор переключения.
Подтвердите все значения A/AAAA/CNAME и переварите GAR (BLAKE3-256 твердого груза GAR). Си а
Зона/имя хоста son conocidos e `--dns-zonefile-out` опустите, или помогите записать их
`artifacts/sns/zonefiles/<zone>/<hostname>.json` и Елена
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
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | Используйте временную метку `effective_at` (RFC3339) при начале начала переключения. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | Отмена буквального подтверждения, зарегистрированного в метаданных. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | Отменить регистрацию CID в метаданных. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Estado de Freeze de Guardian (мягкое, жесткое, оттаивание, мониторинг, чрезвычайная ситуация). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | Справка о билете опекуна/совета по замораживанию. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | Временная метка RFC3339 для размораживания. |
| И18НИ00000218Х, И18НИ00000219Х | Дополнительные заморозки (отдельные запятые или повторяющийся флаг). |
| И18НИ00000220Х, И18НИ00000221Х | Дайджест BLAKE3-256 (шестнадцатеричный) полезной нагрузки фирмы GAR. Требуется, когда есть привязки шлюза. |

В рабочем процессе GitHub Actions очень важна часть секретов репозитория, чтобы каждый раз закрепить их.
автоматическое создание артефактов файла зоны. Настройка следующих секретов
(строки Podem Contener Listas Separadas por Comas Para Campos Multivalor):

| Секрет | Предложение |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | Имя хоста/зона производства pasados ​​al helper. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | Альтернативный псевдоним дежурного вызова или дескриптор. |
| И18НИ00000225Х, И18НИ00000226Х | Зарегистрируйте общедоступный IPv4/IPv6. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | Целевой CNAME необязательно. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Сосны СПКИ base64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | Дополнительные входы TXT. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | Замороженные метаданные, зарегистрированные в электронном виде. |
| `DOCS_SORAFS_GAR_DIGEST` | Переберите BLAKE3 в шестнадцатеричном формате полезной нагрузки GAR. |

Все различия `.github/workflows/docs-portal-sorafs-pin.yml`, пропорциональные входы ОС
`dns_change_ticket` и `dns_cutover_window` для того, чтобы дескриптор/файл зоны был открыт
корректа. Dejarlos em blanco apenas, когда выбрасываются капли.

Типовой призыв (совпадает с книгой владельца SN-7):

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

Помощник автоматически создает билет на получение билета в виде ввода TXT и передает или начинает
Включение переключения под меткой времени `effective_at` означает, что оно будет переопределено. Пара о флюксо
Полная работа, версия `docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Примечание о публичном делегировании DNSСкелет файла зоны определяет авторитарные права доступа к зоне. Аинда э
необходимо настроить делегирование NS/DS в зоне платности без регистратора или поставщика
DNS для Интернет-конференций с серверами имен.

- Для переключения без вершины/TLD используйте псевдоним/ANAME (зависимый от поставщика) или общедоступный.
  Регистры A/AAAA доступны для IP-адресов произвольной рассылки на шлюз.
- Для подчиненных доменов общедоступный CNAME для получения красивого хоста
  (`<fqdn>.gw.sora.name`).
- О хосте canonico (`<hash>.gw.sora.id`) постоянно, как о домене, так и о шлюзе и нао
  e publicado dentro da sua zona publica.

### Установка заголовков шлюза

Помощник по развертыванию тамбема `portal.gateway.headers.txt` e
`portal.gateway.binding.json`, артефакты, удовлетворяющие требованиям DG-3
привязка содержимого шлюза:

- `portal.gateway.headers.txt` содержит полный блок заголовков HTTP (включая
 `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS, дескриптор e o
 `Sora-Route-Binding`) эти шлюзы на границе должны быть введены каждый раз.
- `portal.gateway.binding.json` зарегистрируйте мою информацию в форме, разборчивой для машин.
 для того, чтобы билеты cambio и automacao possam сравнивали привязки хоста/cid sem
 распарьте салиду де ракушку.

Генерируется автоматически через
`cargo xtask soradns-binding-template`
захват псевдонима, дайджеста манифеста и имени хоста шлюза, который есть
пасарон `sorafs-pin-release.sh`. Для регенерации, персонализации или блокировки заголовков,
выполнить:

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
шаблоны заголовков по умолчанию, когда требуются дополнительные указания;
Комбинировалось с коммутаторами ОС `--no-*`, чтобы полностью удалить заголовок.

Приложение или фрагмент заголовков для запроса на создание CDN и содержимого или документа JSON
Все конвейеры автоматических шлюзов для продвижения реального хоста совпадают с
доказательства освобождения.

Сценарий выпуска выполняется автоматически или помощник проверки для вашей ОС
билеты DG-3 siempre incluyan evidencia reciente. Выполните выброс вручную
Когда вы редактируете или связываете JSON вручную:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

Команда декодирования полезной нагрузки `Sora-Proof` зашифрована, убедитесь, что метаданные `Sora-Route-Binding`
совпадает с CID манифеста + имя хоста, а также быстрый заголовок, который соответствует указанному.
Архивируйте консольную консоль вместе с другими артефактами, которые выбрасываются, когда они выбрасываются.
o команда CI для внесения изменений в DG-3, чтобы подтвердить, что обязательность действия подтверждена.
перед переключением.

> **Интеграция дескриптора DNS:** `portal.dns-cutover.json` сейчас включен в раздел
> `gateway_binding` отвечает за эти артефакты (руты, CID контента, состояние доказательства и т. д.)
> шаблонный литерал заголовков) **e** uma estrofa `route_plan` referencendo
> `gateway.route_plan.json` основные шаблоны ОС, заголовки и откат. Включить Эсос
> Блокируйте каждый билет на камбио DG-3, чтобы можно было проверить и сравнить ценности
> Точные данные `Sora-Name/Sora-Proof/CSP` и подтверждение планов продвижения/отката
> Совпало с пакетом доказательств, собранных в начале или в архиве сборки.## Этап 9 — Выполнение публичных проверок

Для пункта дорожной карты **DOCS-3c** требуется постоянное подтверждение наличия портала или его прокси-сервера.
привязки ворот являются полезными для освобождения. Выполнять или контролировать консолидацию
Непосредственно после этапов 7–8 и подключения к программируемым датчикам:

```bash
cd docs/portal
npm run monitor:publishing -- \
 --config ../../configs/docs_monitor.json \
 --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%E%m%dT%H%M%SZ).json \
 --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` Загрузка или архив конфигурации (версия
 `docs/portal/docs/devportal/publishing-monitoring.md` для примера) e
 выполнить три проверки: проверка путей портала + проверка CSP/Permissions-Policy,
 проверяет прокси-сервер (дополнительно проверяется конечная точка `/metrics`), и проверяется
 привязка шлюзов (`cargo xtask soradns-verify-binding`), которая сейчас требует присутствия и
 o смело используйте объединение Sora-Content-CID в качестве проверок псевдонима/манифеста.
- Окончание команды с ненулевым значением, когда будет выполнено задание для CI, заданий cron или операторов.
 Runbook может определить выпуск псевдонима промовера.
- Pasar `--json-out` напишите резюме в формате JSON, единое с целевым местом; `--evidence-dir`
 излучайте `summary.json`, `portal.json`, `tryit.json`, `binding.json` и `checksums.sha256` для того, что
 мы можем внести изменения в управление, чтобы сравнить результаты и повторно выполнить мониторы. Архив
 этот каталог bajo `artifacts/sorafs/<tag>/monitoring/` вместе со всеми комплектами Sigstore e o
 дескриптор переключения DNS.
- Включите удаление монитора или экспорт Grafana (`dashboards/grafana/docs_portal.json`),
 e o ID тренировки Alertmanager в билете выпуска, который может быть использован в SLO DOCS-3c.
 аудитадо луэго. O playbook dedicado de Monitor de Publishing vive em
 `docs/portal/docs/devportal/publishing-monitoring.md`.

Мы проверяем портал, запрашиваем HTTPS и повторяем базовые URL-адреса `http://`, а затем `allowInsecureHttp`
это конфигурация монитора; Mantenha планирует производство/постановку в TLS и приложениях
умение или переопределение локалей предварительного просмотра.

Автоматизация мониторинга через `npm run monitor:publishing` в Buildkite/cron на портале
este em vivo. Моя команда, просмотрите URL-адреса продукции, продукты питания и чеки обслуживания.
SRE/Docs продолжают использовать все выпуски.

## Automacao com `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` инкапсула для этапов 2–6. Эсте:

1. Архивируйте `build/` в определенный архив,
2. выполнить `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`,
 е `proof verify`,
3. опционально выполните `manifest submit` (включите привязку псевдонима), когда он будет зарегистрирован
 Torii, е
4. пропишите `artifacts/sorafs/portal.pin.report.json`, o необязательно
 `portal.pin.proposal.json`, или дескриптор переключения DNS (хранилище отправлений),
 e o пакет привязки шлюзов (`portal.gateway.binding.json` mas o bloque de headers)
 для того, чтобы оборудование для управления, сетевое взаимодействие и возможности сравнения или комплекта доказательств
 журналы sem revisar de CI.

Конфигурация `PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME`, e (опционально)
`PIN_ALIAS_PROOF_PATH` перед вызовом сценария. США `--skip-submit` для сухих условий
бежит; Рабочий процесс GitHub описан в альтернативном виде с помощью ввода `perform_submit`.

## Этап 8 — специальные публикации OpenAPI и пакеты SBOMDOCS-7 требует сборки портала, конкретной спецификации OpenAPI и артефактов SBOM, доступных через
por o mesmo конвейер детерминист. Os helpers Existentes Cubren os Tres:

1. **Регенерация и ассинатура и конкретизация.**

 ```bash
 npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
 cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
 ```

 Этикетка выпуска через `--version=<label>`, когда нужно сохранить снимок
 исторический (например, `2025-q3`). О помощник, напиши или сфотографируй их.
 `static/openapi/versions/<label>/torii.json`, вот спасибо им
 `versions/current`, электронная регистрация метаданных (SHA-256, состояние манифеста,
 актуализирована временная метка) em `static/openapi/versions.json`. О портал desenvolvedores
 Этот указатель для панелей Swagger/RapiDoc позволяет выбрать версию
 e Mostrar o Digest/assinatura asociado em linea. Omitir `--version` mantem как этикеты
 выпуск предыдущего неповрежденного и обновленного обновления `current` + `latest`.

 Манифест захвата обрабатывает SHA-256/BLAKE3 для того, чтобы шлюз мог захватить
 заголовки `Sora-Proof` для `/reference/torii-swagger`.

2. **Выпустите SBOM CycloneDX.** В конвейере выпуска можно увидеть SBOM, основанные на syft.
 Сегун `docs/source/sorafs_release_pipeline_plan.md`. Мантенья-а-Салида
 соединение с артефактами сборки:

 ```bash
 syft dir:build -o json > "$OUT"/portal.sbom.json
 syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
 ```

3. **Упаковать всю полезную нагрузку в АВТОМОБИЛЬ.**

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

 Sigue os mesmos Etapas de `manifest build` / `manifest sign`, что является основным местом,
 задать псевдоним для артефакта (например, `docs-openapi.sora` для конкретной спецификации и
 `docs-sbom.sora` для фирменного комплекта SBOM). Псевдоним отдельного мантема SoraDNS,
 GAR и билеты отката точно по полезной нагрузке.

4. **Отправьте электронную привязку.** Повторно используйте существующий авторизованный пакет Sigstore, предварительно зарегистрировавшись.
 o кортеж псевдонимов em o контрольный список выпуска для того, чтобы аудиторы были растрированы под именем Sora
 отобразите обзор манифеста.

Загрузите манифесты спецификаций/SBOM вместе со сборкой портала, гарантируя, что каждый билет будет
выпустите содержимое или установите полный набор артефактов, чтобы повторно выполнить или упаковать.

### Helper de automacao (скрипт CI/пакета)

`./ci/package_docs_portal_sorafs.sh` кодировка этапов 1–8 для пункта дорожной карты
**DOCS-7** можно использовать, когда вы находитесь в команде. О, помощник:

- выполнить запрос на подготовку портала (`npm ci`, синхронизация OpenAPI/norito, тесты виджетов);
- выдать CAR и части манифеста портала, OpenAPI и SBOM через `sorafs_cli`;
- опционально выполните `sorafs_cli proof verify` (`--proof`) и ассинатуру Sigstore.
 (И18НИ00000315Х, И18НИ00000316Х, И18НИ00000317Х);
- все артефакты bajo `artifacts/devportal/sorafs/<timestamp>/` e
 Опишите `package_summary.json` для того, чтобы CI/инструменты выпуска могли быть включены в пакет; е
- Refresca `artifacts/devportal/sorafs/latest` для возврата к полученному результату.

Пример (полный конвейер с Sigstore + PoR):

```bash
./ci/package_docs_portal_sorafs.sh \
 --proof \
 --sign \
 --sigstore-provider=github-actions \
 --sigstore-audience=sorafs-devportal
```

Флаги conocer:- `--out <dir>` - переопределить корень артефактов (временная метка com по умолчанию).
- `--skip-build` - повторно использовать существующий `docs/portal/build` (использовать, когда CI не используется
 реконструировать зеркала в автономном режиме).
- `--skip-sync-openapi` - опустить `npm run sync-openapi`, когда `cargo xtask openapi`
 нет возможности открыть crates.io.
- `--skip-sbom` - evita llamar a `syft`, когда бинарный файл не установлен (o script imprime
 это реклама в твоих глазах).
- `--proof` — выполнить `sorafs_cli proof verify` для каждого CAR/манифеста. Полезные нагрузки ОС
 требуется несколько архивов, поддерживающих план фрагментов в CLI, если у вас есть этот флаг
 sem establecer, если обнаружены ошибки `plan chunk count` и проверено руководство, когда
 llegue o ворота вверх по течению.
- `--sign` - вызов `sorafs_cli manifest sign`. Provee um token com
 `SIGSTORE_ID_TOKEN` (или `--sigstore-token-env`) или узнать, что из CLI можно получить использование
 `--sigstore-provider/--sigstore-audience`.

Quando завидует артефактам производства США `docs/portal/scripts/sorafs-pin-release.sh`.
Пакет документов на портале, OpenAPI и полезные данные SBOM, сбор каждого манифеста и регистрация метаданных
дополнительные активы в `portal.additional_assets.json`. Дополнительный помощник в работе с дополнительными ручками
используются для упаковки CI для новых коммутаторов `--openapi-*`, `--portal-sbom-*`, e
`--openapi-sbom-*` для назначения кортежей псевдонимов для артефактов, переопределения действующего SBOM через
`--openapi-sbom-source`, опустить полезные нагрузки (`--skip-openapi`/`--skip-sbom`),
Подключитесь к бинарному файлу `syft` без стандартного `--syft-bin`.

Скрипт выставляет каждую команду, которую нужно выполнить; скопировать или зарегистрировать их или билет на выпуск
объединение с `package_summary.json` для того, чтобы мы могли проверить дайджесты CAR, метаданные
план, электронные хэши пакета Sigstore, которые пересматриваются специально для оболочки.

## Этап 9 – проверка шлюза + SoraDNS

Прежде чем объявить о переключении, подтвердите, что новый псевдоним будет повторно разрешен через SoraDNS и какие шлюзы
энграпан провас фреска:

1. **Выполнить вход датчика.** `ci/check_sorafs_gateway_probe.sh` ejercita
 `cargo xtask sorafs-gateway-probe` демонстрация приборов contra os em
 `fixtures/sorafs_gateway/probe_demo/`. Для решения реальных задач можно проверить имя хоста:

 ```bash
 ./ci/check_sorafs_gateway_probe.sh -- \
 --gatewae "https://docs.sora/.well-known/sorafs/manifest" \
 --header "Accept: application/json" \
 --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
 --gar-kee "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
 --host "docs.sora" \
 --report-json artifacts/sorafs_gateway_probe/ci/docs.json
 ```

 Расшифровка зонда `Sora-Name`, `Sora-Proof`, e `Sora-Proof-Status` segun
 `docs/source/sorafs_alias_policy.md` и Falla Quando или дайджест манифеста,
 TTL или привязки GAR разработаны специально.

   Для облегченных выборочных проверок (например, когда только пакет переплета
   изменилось), запустите `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   Помощник проверяет захваченный пакет привязок и удобен для выпуска.
   билеты, для которых требуется только обязательное подтверждение вместо полной проверки.

2. **Доказательство тренировок.** Для тренировок оператора или имитации PagerDuty, оберните
 o зондировать com `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario
 devportal-rollout -- ...`. Обертка защищает заголовки/журналы bajo
 `artifacts/sorafs_gateway_probe/<stamp>/`, актуализируется `ops/drill-log.md`, e
 (необязательно) различные способы отката или полезные данные PagerDuty. Эстаблес
 `--host docs.sora` для проверки рута SoraDNS на жестко запрограммированном IP-адресе.3. **Проверка привязок DNS.** Когда осуществляется публичное управление или подтверждение псевдонима, регистрация или архив GAR
 ссылка на зонд (`--gar`) и дополнительное доказательство выпуска. Владельцы резольвера
 Можно отобразить или ввести ввод через `tools/soradns-resolver`, чтобы гарантировать, что вход в кэш
 ответ о новом манифесте. Перед подключением или JSON выполните
 `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
 для определения карты хоста, метаданных манифеста и этикеток телеметрии
 действительны в автономном режиме. Помощник может отправить резюме `--json-out` вместе с фирмой GAR для вас
 пересматривает теганские доказательства, поддающиеся проверке, после завершения или бинарного процесса.
 Когда редактируется GAR novo, prefiere
 `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
 (vuelve a `--manifest-cid <cid>` apenas quando o archive de manifeto no este disponible). О помощник
 сейчас это производное от CID **e** или дайджест BLAKE3 непосредственно из манифеста JSON, запись на пустом месте,
 флаги дедупликации `--telemetry-label`, повторы, этикетки и эмитирование шаблонов ОС по умолчанию
 CSP/HSTS/Permissions-Police перед написанием или JSON для определения полезной нагрузки
 в том числе, когда операторы захватывают этикеты и части различных ракушек.

4. **Соблюдайте метрики псевдонимов.** Mantenha `torii_sorafs_alias_cache_refresh_duration_ms`
 e `torii_sorafs_gateway_refusals_total{profile="docs"}` на панели зонда или зонда
 корр; Серия Ambas Estan Graficadas Em `dashboards/grafana/docs_portal.json`.

## Этап 10 - Мониторинг и комплект доказательств

- **Панели мониторинга.** Экспорт `dashboards/grafana/docs_portal.json` (SLO портала),
 `dashboards/grafana/sorafs_gateway_observability.json` (задержка шлюза +
 привет де доказательство), e `dashboards/grafana/sorafs_fetch_observability.json`
 (приветствие оркестратора) для каждого выпуска. Anexe os экспортирует JSON все билеты
 Release для того, чтобы эти изменения могли воспроизвести запросы Prometheus.
- **Архив зонда.** Сохранение `artifacts/sorafs_gateway_probe/<stamp>/` в git-приложении.
 о твое ведро доказательств. Включите резюме зонда, заголовки и полезную нагрузку PagerDuty.
 захватывается сценарием телеметрии.
- **Пакет выпуска.** Защита резюме CAR del portal/SBOM/OpenAPI, комплекты манифеста,
 фирмы Sigstore, `portal.pin.report.json`, журналы проб Try-It, электронные отчеты о проверке связи
 временная метка bajo uma ковра com (например, `artifacts/sorafs/devportal/20260212T1103Z/`).
- **Журнал бурения.** Когда вы исследуете часть бура, это то, что вам нужно.
 `scripts/telemetry/run_sorafs_gateway_probe.sh` объединяется с `ops/drill-log.md`
 для того, чтобы получить необходимые доказательства или требования к SNNet-5.
- **Ссылки на билет.** Ссылка на идентификаторы панели Grafana или экспорт PNG-дополнений к билету.
 Камбио, вместе с рутой отчета о расследовании, для того, чтобы мы могли проверить SLO
 sem доступ к оболочке.

## Этап 11 — Упражнение по извлечению данных из нескольких источников и доказательств на табло

Опубликовано в SoraFS, сейчас требуются доказательства выборки из нескольких источников (DOCS-7/SF-6)
junto a as provas de DNS/gatewae anteriores. Депуа-де-фихар или манифест:1. **Выполните `sorafs_fetch` против живого манифеста.** Используйте объекты плана/манифеста.
 производятся на этапах 2–3 раза в качестве удостоверений эмитированных ворот для каждого подтверждения. упорствовать
 Далее нужно сказать, что аудиторы могут воспроизвести или изменить решение оркестратора:

 ```bash
 OUT=artifacts/sorafs/devportal
 FETCH_OUT="$OUT/fetch/$(date -u +%E%m%dT%H%M%SZ)"
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

 - Приведите примеры рекламных объявлений, ссылающихся на манифест (например,
 `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
 Пройдите через `--provider-advert name=path`, чтобы на табло можно было оценить результаты
 детерминированная способность детерминированной формы. США
 `--allow-implicit-provider-metadata` **apenas** quando воспроизводит приборы в CI; ОС дрели
 производство должно быть процитировано в рекламных фирмах, которые будут размещены с помощью булавки.
 - Когда манифест ссылается на дополнительные регионы, повторите команду в виде кортежей
 проверяйте корреспонденцию, чтобы получить кэш/псевдоним артефакта ассоциированной выборки.

2. **Архивировать как сохраненные.** Guarda `scoreboard.json`,
 `providers.ndjson`, `fetch.json`, и `chunk_receipts.ndjson` лежат на ковре доказательств
 выпуск. Эти архивы собраны для взвешивания аналогов, возврата бюджета, задержки EWMA и т. д.
 квитанции на часть пакета управления должны быть сохранены для SF-7.

3. **Актуализация телеметрии.** Импортируется как данные для выборки на панели управления **SoraFS Fetch.
 Наблюдаемость** (`dashboards/grafana/sorafs_fetch_observability.json`),
 Наблюдайте за `torii_sorafs_fetch_duration_ms`/`_failures_total` и панелями рангом для проверки
 парааномалии. Откройте снимки панели Grafana с билетом на выпуск в рутинном режиме.
 табло.

4. **Курить согласно правилам.** Выполнить `scripts/telemetry/test_sorafs_fetch_alerts.sh`.
 для проверки пакета оповещений Prometheus перед выпуском. Анекс в Салида де
 promtool al Ticket для внесения изменений в DOCS-7, подтверждающий, что оповещения о остановке электронной почты
 медленный провайдер сигуен армады.

5. **Кабель в CI.** Рабочий процесс вывода порта на этап `sorafs_fetch` отделяет ввод
 И18НИ00000390Х; умело выполнять постановку/постановку для доказывания
 извлеките комплект из пакета манифеста, содержащего руководство по вмешательству. ОС тренирует локали Podem
 повторно использовать или создать сценарий экспорта токенов шлюза и установить `PIN_FETCH_PROVIDERS`
 список проверяющих отдельных лиц.

## Промокао, наблюдение и откат

1. **Промокация:** отдельные псевдонимы для постановки и производства. Повторное проведение промоциона
 `manifest submit` в составе манифеста/пакета, cambiando
 `--alias-namespace/--alias-name`, чтобы добавить псевдоним производства. Эсто Эвита
 реконструировать или повторно использовать то, что QA проверяется или закрепляется на этапе подготовки.
2. **Мониторинг:** импорт панели управления регистрацией контактов.
 (`docs/source/grafana_sorafs_pin_registry.json`) другие датчики ОС для конкретного портала
 (версия `docs/portal/docs/devportal/observability.md`). Оповещение об отклонении контрольных сумм,
 исследует провалы или пики отступления доказательств.
3. **Откат:** для возврата, повторного просмотра или предыдущего манифеста (или возврата или фактического псевдонима) с использованием
 `sorafs_cli manifest submit --alias ... --retire`. Пакет Mantenha siempre o ultimo
 Conocido как Bueno E o Resumo CAR, чтобы обеспечить повторный откат
 OS журналы CI вращаются.

## Планирование рабочего процесса CIКак минимум, ваш трубопровод должен быть:

1. Сборка + проверка (`npm ci`, `npm run build`, генерация контрольных сумм).
2. Empacotar (`car pack`) и компьютерные манифесты.
3. Используйте токен OIDC для задания (`manifest sign`).
4. Субирские артефакты (КАР, манифест, связка, план, резюме) для аудитории.
5. Отправьте реестр контактов:
 - Запросы на извлечение -> `docs-preview.sora`.
 - Теги/защитные ветки -> продвижение псевдонима производства.
6. Выполнение зондов + ворота проверки до завершения.

`.github/workflows/docs-portal-sorafs-pin.yml` подключите все эти этапы для выпуска руководств.
О, рабочий процесс:

- построить/проверить портал,
- пакет или сборка через `scripts/sorafs-pin-release.sh`,
- Assinatura/verifique или пакет манифеста с использованием GitHub OIDC,
- включать в себя CAR/манифест/связку/план/резюме как артефакты, e
- (необязательно) зависть к манифесту + псевдоним связывания, когда есть секреты.

Настройте следующие секреты/переменные хранилища перед различием в задании:

| Имя | Предложение |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | Хост Torii, который выставляет `/v1/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | Идентификатор зарегистрированной эпохи в представленных материалах. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | Разрешение на убийство для подачи манифеста. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | Кортеж псевдонима, добавленный в манифест `perform_submit`, равен `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Пакет доказательств кодирования псевдонима в base64 (необязательно; опустите для привязки альтернативного псевдонима). |
| `DOCS_ANALYTICS_*` | Конечные точки аналитики/исследования существуют, повторно используются в других рабочих процессах. |

Отображение рабочего процесса через пользовательский интерфейс действий:

1. Provee `alias_label` (например, `docs.sora.link`), `proposal_alias` дополнительно,
 Я могу переопределить необязательное значение `release_tag`.
2. Дежа `perform_submit`, созданная для создания артефактов, sem tocar Torii
 (используется для запуска) или может быть использован для прямой публикации псевдонима, настроенного.

`docs/source/sorafs_ci_templates.md` в виде документа по общим помощникам CI для
Проекты из этого хранилища, но рабочий процесс портала может быть предпочтительным вариантом
для освобождения дель-диа-диа.

## Контрольный список

- [ ] `npm run build`, `npm run test:*`, и `npm run check:links` находятся в зеленом состоянии.
- [ ] `build/checksums.sha256` и `build/release.json` захвачены артефактами.
- [ ] АВТОМОБИЛЬ, план, манифест и составленное резюме `artifacts/`.
- [ ] Bundle Sigstore + журналы assinatura separada almacenados com.
- [ ] `portal.manifest.submit.summary.json` и `portal.manifest.submit.response.json`
 capturados quando hae представления.
- [ ] `portal.pin.report.json` (дополнительно `portal.pin.proposal.json`)
 Объединенный архив и артефакты CAR/манифест.
- [ ] Журналы архивов `proof verify` и `manifest verify-signature`.
- [ ] Актуализированные информационные панели Grafana + тестовые тесты Try-It.
- [ ] Примечания об откате (ID предыдущего манифеста + дайджест псевдонима) дополнительные функции
 билет на выпуск.