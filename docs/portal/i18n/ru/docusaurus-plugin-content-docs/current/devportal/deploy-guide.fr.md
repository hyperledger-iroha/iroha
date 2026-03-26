---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## ансамбль

Этот сборник сценариев преобразует элементы дорожной карты **DOCS-7** (публикация SoraFS) и **DOCS-8**
(автоматизация CI/CD) в рамках процедуры, доступной для разработки портала.
Il couvre laphase build/lint, l'empaquetage SoraFS, la подпись манифестов с Sigstore,
продвижение псевдонимов, проверка и инструкции по откату для предварительного просмотра и выпуска каждой части
так что это воспроизводимо и проверяемо.

Le flux предположим, что вы получили двоичный файл `sorafs_cli` (компилируем с `--features cli`), доступ к
конечная точка Torii с разрешениями на регистрацию контактов и учетными данными OIDC для Sigstore.
Сохраняйте длинные секреты (`IROHA_PRIVATE_KEY`, `SIGSTORE_ID_TOKEN`, токены Torii) в
ваш ящик CI; Локали выполнения могут быть использованы для зарядного устройства через экспорт оболочки.

## Предварительное условие

- Узел 18.18+ с `npm` или `pnpm`.
- `sorafs_cli` через `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- URL Torii, который предоставляет доступ к `/v1/sorafs/*` плюс доступ к конфиденциальному доступу к авторизации, который может быть полезен.
  де манифесты и де псевдонимы.
- Emmeteur OIDC (GitHub Actions, GitLab, удостоверение рабочей нагрузки и т. д.) для заполнения `SIGSTORE_ID_TOKEN`.
- Опция: `examples/sorafs_cli_quickstart.sh` для работы всухую и т. д.
  `docs/source/sorafs_ci_templates.md` для создания рабочих процессов GitHub/GitLab.
- Настройте переменные OAuth. Попробуйте (`DOCS_OAUTH_*`) и выполните.
  [контрольный список по усилению безопасности](./security-hardening.md) перед продвижением сборки
  закуска из лаборатории. Le build du portail echoue maintenant lorsque ces переменные manquent
  или другие ручки TTL/опрос сортировки окон; экспортировать
  `DOCS_OAUTH_ALLOW_INSECURE=1` — уникальная возможность для предварительного просмотра языковых модулей. Жуаньез
  les preuves de pen-test au Ticket de Release.

## Etape 0 - Capturer un Bundle du Proxy Попробуйте

Прежде чем продвигать предварительный просмотр версий Netlify или шлюза, выберите источники прокси. Попробуйте
и дайджест манифеста OpenAPI подписан в определенном пакете:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` скопируйте вспомогательные прокси/зонд/откат, проверьте подпись
OpenAPI и код `release.json` плюс `checksums.sha256`. Joignez ce Bundle с билетом
Продвижение шлюза Netlify/SoraFS для того, чтобы рецензенты могли повторно получать точные источники
и подсказки о цели Torii без реконструкции. Регистрация пакета на всех носителях
Fournis Par Le Client Etaient Actives (`allow_client_auth`) для составления плана развертывания
и правила CSP в фазе.

## Этап 1 — Сборка и установка порта

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

`npm run build` выполняет автоматизацию `scripts/write-checksums.mjs`, производя:

- `build/checksums.sha256` - манифест SHA256 для `sha256sum -c`.
- `build/release.json` — метаданные (`tag`, `generated_at`, `source`) исправляют ошибки в CAR/манифесте.

Архив этих фотографий с резюме CAR для тех, кто рецензенты могут лучше сравнивать артефакты
предварительный просмотр без реконструкции.

## Этап 2 — Упаковка статических активовВыполните упаковщик CAR против набора вылетов Docusaurus. L'exmple ci-dessous ecrit
артефакты sous `artifacts/devportal/`.

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

Возобновление JSON-сбора данных по частям, дайджестов и подсказок по планированию доказательства
que `manifest build` и панели мониторинга CI повторно используются плюс поздно.

## Этап 2b — Упаковать компаньонов OpenAPI и SBOM

DOCS-7 требует публикации сайта порта, моментального снимка OpenAPI и полезных данных SBOM.
различия между манифестами для того, чтобы шлюзы могли эффективно использовать заголовки
`Sora-Proof`/`Sora-Content-CID` для артефакта чека. Помощник по освобождению
(`scripts/sorafs-pin-release.sh`) поместить репертуар OpenAPI
(`static/openapi/`) и SBOM, отправляемые через `syft` в отдельных CAR
`openapi.*`/`*-sbom.*` и зарегистрируйте метадоннистов в
`artifacts/sorafs/portal.additional_assets.json`. En флюс вручную,
повторите этапы 2–4 для заполнения полезной нагрузки с собственными префиксами и метаданными меток.
(пример примера `--car-out "$OUT"/openapi.car` плюс
`--metadata alias_label=docs.sora.link/openapi`). Зарегистрируйте пару манифест/псевдоним
на Torii (сайт, OpenAPI, SBOM portail, SBOM OpenAPI) перед базированием DNS для этого
ворота служат превентивным аграфам для всех публикаций артефактов.

## Этап 3 — Создание манифеста

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

Настройте флаги политики выводов для окон выпуска (например, `--pin-storage-class
горячее для канареек). Вариант JSON — это более практичный вариант для проверки кода.

## Этап 4 — Подписчик с Sigstore

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

Пакет регистрирует дайджест манифеста, дайджесты фрагментов и хэш BLAKE3 токена
OIDC без сохранения JWT. Gardez le Bundle et la Signature Detachee; промо-акции
производство может повторно использовать мемы-артефакты вместо переподписчика. Места казней
возможно заменить поставщика флагов по `--identity-token-env` (или определить `SIGSTORE_ID_TOKEN`
) Quand un helper OIDC externe emet le token.

## Этап 5 — реестр Soumettre au Pin

Соберите манифест (и план фрагментов) в Torii. Требуйте возобновления работы
que l'entree/alias результат, который можно проверить.

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

Для развертывания предварительного просмотра или канарейки (`docs-preview.sora`), повторите вызов
с уникальным псевдонимом для того, чтобы можно было проверить контент перед продвижением продукции.

Привязка псевдонимов требует трех чемпионов: `--alias-namespace`, `--alias-name` и `--alias-proof`.
Управление производит пакет доказательств (base64 или байты Norito), когда требуется псевдоним
одобряемый; Stockez-le dans les secrets CI et Exposez-le comme fichier avant d'invoquer
`manifest submit`. Laissez les flags d'alias vides quand vous voulez seulement pinner
манифест без касания или DNS.

## Этап 5b — Генерирование предложения по управлению

Chaque манифест doit voyager с предложением для парламента, который будет представлен всем гражданам
Вы можете ввести изменение привилегий без необходимости ввода учетных данных. После обеда
этапы отправить/подписать, lancez:```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` запишите каноническую инструкцию `RegisterPinManifest`,
дайджест фрагментов, политика и индекс псевдонимов. Жоаньез-ле-о-билет управления или о
Портал Парламента для того, чтобы делегаты могли сравнить полезную нагрузку без реконструкции.
Comme la commande ne touche jamais la cle d'autorite Torii, tout citoyen peut rediger la
локализация предложения.

## Этап 6 — Проверка доказательств и телеметрии

После нажатия кнопки выполните этапы определения определенной проверки:

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

- Surveillez `torii_sorafs_gateway_refusals_total` и др.
  `torii_sorafs_replication_sla_total{outcome="missed"}` для устранения аномалий.
- Выполните `npm run probe:portal` для выполнения прокси-сервера Try-It и регистрации залогов.
  contre le contenu fraichement pinne.
- Capturez l'evidence de Monitoring Decrite dans
  [Публикация и мониторинг](./publishing-monitoring.md) для ворот наблюдения
  DOCS-3c соответствует этапам публикации. Помощник принимает обслуживание
  дополнительные входы `bindings` (сайт, OpenAPI, порт SBOM, SBOM OpenAPI) и обеспечение соблюдения
  `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` по кабелю отеля через опцию le Guard
  `hostname`. Вызов Ci-dessous ecrit un резюме JSON уникальный и комплект доказательств
  (`portal.json`, `tryit.json`, `binding.json`, `checksums.sha256`) sous le repertoire
  де релиз:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## Etape 6a — Планирование сертификатов шлюза

Создайте план SAN/вызов TLS перед созданием пакетов GAR для шлюза оборудования и др.
эксперты DNS, эксперты по мемам. Новый помощник по отражению входных данных DG-3
в перечислении канонических узлов с подстановочными знаками, красивых хостов SAN, меток DNS-01 и т. д.
решения проблем ACME рекомендует:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

Подтвердите JSON с выпуском пакета (или загрузите с билетом изменения) для
какие операторы могут использовать значения SAN в конфигурации
`torii.sorafs_gateway.acme` от Torii и те рецензенты GAR, которые могут подтвердить файлы
сопоставления canonique/pretty sans re-executer les derivations d'hotes. Дополнительные аргументы
`--name` дополняется суффиксом для выпуска мема.

## Этап 6b - Получение канонических сопоставлений

Перед шаблонированием полезных данных GAR зарегистрируйте определенное сопоставление для каждого псевдонима.
`cargo xtask soradns-hosts` хэш-шак `--name` в соответствии с канонической этикеткой
(`<base32>.gw.sora.id`), получите требуемый подстановочный знак (`*.gw.sora.id`) и получите красивый хост
(И18НИ00000150Х). Продолжайте вылазку в артефактах освобождения, чтобы они
рецензенты DG-3 могут сравнить карты с предложением GAR:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

Используйте `--verify-host-patterns <file>` для отображения данных GAR или JSON de
Привязка шлюза к требуемым хостам. Помощник принимает дополнительные документы для проверки,
это облегчает просмотр шаблона GAR и `portal.gateway.binding.json` в графе
вызов мема:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```Откройте JSON-резюме и журнал проверки билета на изменение DNS/шлюза для этого
les Auditeurs может подтвердить канонические хосты, подстановочные знаки и красивые файлы без повторного исполнителя
сценарии. Повторно выполните команду, когда новый псевдоним будет добавлен в пакет для тех, что ле
обновляет доказательства GAR Heritent de la Meme.

## Этап 7 — Генерация описания переключения DNS

Для производства требуется сменный пакет, подлежащий проверке. После ужина
reussie (привязка псевдонима), помощник по работе
`artifacts/sorafs/portal.dns-cutover.json`, захватчик:

- метаданные привязки псевдонима (пространство имен/имя/доказательство, манифест дайджеста, URL Torii,
  эпоха сумис, авторит);
- контекст выпуска (тег, псевдоним метки, манифест chemins/CAR, план блоков, пакет Sigstore);
- указатели проверки (командный зонд, псевдоним + конечная точка Torii); и др.
- возможности контроля сдачи (идентификатор билета, переключение окон, контактные операции,
  имя хоста/зона);
- метаданные продвижения маршрута, полученные из заголовка `Sora-Route-Binding`
  (host canonique/CID, заголовок + привязка, команды проверки), гарантия того, что
  продвижение GAR и др. отработок отступления, ссылающихся на доказательства мемов;
- родовые артефакты плана маршрута (`gateway.route_plan.json`,
  шаблоны заголовков и параметры отката заголовков) для билетов изменения и т. д.
  перехватывает ворс CI, который может проверить пакет пакетов DG-3, справочные файлы планов
  продвижение/откат канонических правил до одобрения;
- Опция метаданных для аннулирования кэша (конечная точка очистки, переменная аутентификация, полезная нагрузка JSON,
  и др. пример команды `curl`); и др.
- подсказки относительно точки отката и прецедента дескриптора (тег выпуска и дайджест манифеста)
  для того, чтобы билеты захватили определенный резервный режим.

Когда потребуется освободить кэш, создайте канонический план с описанием:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

Используйте `portal.cache_plan.json` в пакете DG-3 для операторов, управляющих хостами/путями.
 детерминисты (и подсказки для корреспондентов) lorsqu'ils emettent des requetes `PURGE`.
Опция кэша раздела в описании может ссылаться на указанное направление, охранять
Рецензенты, работающие над сбором данных о конечных точках, удаляются во время переключения.

Chaque paquet DG-3 — это дополнительное продвижение по контрольному списку + откат. Женерез-ле через
`cargo xtask soradns-route-plan` для того, чтобы рецензенты могли отслеживать точные этапы
предполетная подготовка, переключение и откат по псевдонимам:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

Le `gateway.route_plan.json` emis capture хосты canoniques/pretty, спуски для проверки работоспособности
одновременно с этим, обновления привязки GAR, очистка кэша и действия по откату. Жуаньез-ле-Окс
артефакты GAR/привязка/переключение перед обменным билетом, который можно использовать
повторите и проверьте сценарии мемов и этапов.

`scripts/generate-dns-cutover-plan.mjs` питание для описания и автоматического выполнения
`sorafs-pin-release.sh`. Для регенерации или персонализации:

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
```Измените параметры метаданных через переменные среды перед помощником исполнителя:

| Переменная | Но |
| --- | --- |
| `DNS_CHANGE_TICKET` | Идентификатор запаса билетов в описании. |
| `DNS_CUTOVER_WINDOW` | Отверстие переключения ISO8601 (например: `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | Производство имени хоста + авторитарная зона. |
| `DNS_OPS_CONTACT` | Псевдоним дежурного или контактного лица d'escalade. |
| `DNS_CACHE_PURGE_ENDPOINT` | Очистка кэша конечной точки и регистрация в описании. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Env var contenant le token purge (по умолчанию: `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | Chemin vers le descripteur прецедент для отката метаданных. |

Используйте JSON для проверки изменения DNS для того, чтобы проверяющие могли лучше проверить дайджесты
манифесты, привязки псевдонимов и команд зондируют без ошибок журналы CI. Флаги CLI
И18НИ00000173Х, И18НИ00000174Х, И18НИ00000175Х,
И18НИ00000176Х, И18НИ00000177Х, И18НИ00000178Х,
`--cache-purge-auth-env` и `--previous-dns-plan` четыре переопределения мемов
quand le helper Tourne вне CI.

## Этап 8 – Удаление фрагмента файла зоны разрешения (опция)

Когда окончание прерывания производства продолжается, сценарий выпуска может быть удален.
SNS и автоматический преобразователь фрагментов файла зоны. Passez-les
записывает запросы DNS и метаданные через переменные среды или параметры CLI; помощник
Appelle `scripts/sns_zonefile_skeleton.py` немедленно после создания описания.
Четырехкратное значение A/AAAA/CNAME и дайджест GAR (BLAKE3-256 du payload GAR Signe).
Если зона/имя хоста не подключены и `--dns-zonefile-out` отсутствует, помощник запишет его
`artifacts/sns/zonefiles/<zone>/<hostname>.json` и т.д.
`ops/soradns/static_zones.<hostname>.json` как преобразователь фрагментов.| Переменная/флаг | Но |
| --- | --- |
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | Общий файл зоны Chemin du squelette. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Chemin du snippet resolver (по умолчанию: `ops/soradns/static_zones.<hostname>.json` без изменений). |
| И18НИ00000190Х, И18НИ00000191Х | TTL-приложение для дополнительных записей (по умолчанию: 600 секунд). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | Адреса IPv4 (отдельные адреса или флаги повторяемости CLI). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | Адреса IPv6. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | Целевой параметр CNAME. |
| И18НИ00000198Х, И18НИ00000199Х | Пины СПКИ SHA-256 (base64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Дополнительные записи TXT (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | Переопределить метку версии файла зоны при расчете. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | Примените временную метку `effective_at` (RFC3339) вместо дебютного переключения окна. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | Отменить буквальную регистрацию в метаданных. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | Отмена регистрации CID в метаданных. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Etat de Freeze Guardian (мягкая, жесткая, оттаивающая, мониторинг, экстренная). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | Справочный орган опекуна/совета по заморозке билетов. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | Временная метка RFC3339 для оттаивания. |
| И18НИ00000218Х, И18НИ00000219Х | Дополнительные примечания замораживаются (env separe par Virgules или повторяемый флаг). |
| И18НИ00000220Х, И18НИ00000221Х | Дайджест BLAKE3-256 (шестнадцатеричный) для подписи полезной нагрузки GAR. Requis quand des привязки шлюза представлены. |

Рабочий процесс GitHub Actions включает в себя все секреты репозитория для создания цепочек-булавок
Автоматически произвести автоматическую обработку файла зоны артефактов. Настройте следующие секреты (les valeurs peuvent
contenir des listes separees par virgules pour les champs multivalues):

| Секрет | Но |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | Создание имени хоста/зоны передается в помощь. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | Псевдоним дежурного склада в описании. |
| И18НИ00000225Х, И18НИ00000226Х | Записывает IPv4/IPv6 издателя. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | Целевой параметр CNAME. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Пины SPKI base64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | Дополнительные записи TXT. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | Метаданные заморозить зарегистрированного пользователя в squelette. |
| `DOCS_SORAFS_GAR_DIGEST` | Дайджест BLAKE3 в шестнадцатеричном формате полезной нагрузки GAR. |

В сложенном виде `.github/workflows/docs-portal-sorafs-pin.yml`, четыре входа
`dns_change_ticket` и `dns_cutover_window` для текущего описания/файла зоны
Бонне Фенетре. Laissez-les vides уникальность для сухих трасс.

Тип вызова (в линии с владельцем Runbook SN-7):

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
  ...autres flags...
```

Помощник автоматически восстанавливает билет обмена, входя в TXT и получая его при дебюте.
переключение оконного проема с временной меткой `effective_at` sauf override. Чтобы завершить рабочий процесс, представьте себе
`docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Примечание к публичному делегированию DNSСкелет файла зоны не определен, как авторизованная регистрация
зона. Вам необходимо настроить делегирование NS/DS родительской зоны
Ваш регистратор или специалист DNS для общедоступных серверов Интернета
де ном.

- Для переключения на вершину/TLD используйте ALIAS/ANAME (selon le fournisseur) или
  Публикация регистраций A/AAAA указывает на IP-шлюз Anycast.
- Для своих доменов опубликуйте CNAME версии красивого хоста.
  (`<fqdn>.gw.sora.name`).
- Канонический отель (`<hash>.gw.sora.id`) Reste Sous le Domaine du Gateway et
  n'est pas publie в вашей публичной зоне.

### Шаблон шлюза заголовков

Помощник по развертыванию emet aussi `portal.gateway.headers.txt` и др.
`portal.gateway.binding.json`, два артефакта, удовлетворяющие требованиям DG-3
залейте привязку содержимого шлюза:

- `portal.gateway.headers.txt` содержимое полного блока заголовков HTTP (включая
  `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS и т. д.
  `Sora-Route-Binding`), которые пограничные шлюзы дают ответный сигнал.
- `portal.gateway.binding.json` зарегистрировать информацию о меме на доступной машине
  для того, чтобы билеты обмена и автоматизация могли сравнивать ле
  привязки хоста/цида без скребка la sortie Shell.

Автоматизация Ils sont Generes через
`cargo xtask soradns-binding-template`
и захватывает псевдоним, дайджест манифеста и шлюз имени хоста.
`sorafs-pin-release.sh`. Для регенерации или настройки блока заголовков нажмите:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

Passez `--csp-template`, `--permissions-template` или `--hsts-template` для переопределения
Шаблоны заголовков по умолчанию при развертывании перед директивами
добавки; Комбинируйте с переключателями `--no-*`, существующими для поддержки
завершение заголовка.

Используйте фрагмент заголовков по требованию изменения CDN и добавьте документ JSON.
шлюз автоматизации конвейера для того, чтобы продвижение по службе соответствовало
доказательство освобождения.

Сценарий выпуска автоматически выполняет помощник проверки для этих файлов
Билеты DG-3 включают в себя недавние доказательства. Повторное выполнение манипуляций
вы модифицируете привязку JSON по принципу main:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

Команда декодирования полезной нагрузки `Sora-Proof` активируется, проверяется метаданные
`Sora-Route-Binding` соответствует CID манифеста + имя хоста и т. д.
если вы получите заголовок. Archivez la sortie console с другими артефактами
развертывание quand vous lancez la Commande hors CI для рецензентов DG-3
aient la preuve que le связывание a ete valide avant le Cutover.> **Интеграция описания DNS:** `portal.dns-cutover.json` обслуживание установки
> раздел `gateway_binding` указывает на артефакты (chemins, CID контента,
> Закон о доказательстве и буквальный шаблон заголовков) **et** одна строфа `route_plan`
> Эта ссылка `gateway.route_plan.json` содержит шаблоны заголовков
> Принципал и откат. Включите эти блоки в другой билет DG-3 для тех, кто хочет
> рецензенты могут сравнить точные значения `Sora-Name/Sora-Proof/CSP` и др.
> подтверждение того, что планы продвижения/отката соответствуют пакету доказательств
> без создания архива.

## Этап 9 — Выполнение мониторинга публикаций

Дорожная карта объекта **DOCS-3c** требует подтверждения продолжения работы порта и прокси. Попробуйте и др.
Шлюз привязок остается неактивным после выпуска. Выполнить консолидацию монитора
только после 7-8 этапов и переходов к вашим программам:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` зарядите конфигурацию файла (voir
  `docs/portal/docs/devportal/publishing-monitoring.md` для схемы) и др.
  выполнить тройную проверку: проверка путей порта + проверка CSP/Permissions-Policy,
  зондирует прокси-сервер. Попробуйте (опция через конечную точку сына `/metrics`), и затем проверьте
  шлюз привязки (`cargo xtask soradns-verify-binding`), который требуется для обслуживания
  присутствие и значение присутствия Sora-Content-CID с псевдонимом/манифестом проверки.
- Команда возвращает ненулевое значение и отображает эхо-сигнал для CI, заданий cron или
  Операторы Runbook могут остановить выпуск перед продвижением псевдонима.
- Passer `--json-out`, зарегистрированный в резюме в формате JSON, уникальный с соответствующим законом; `--evidence-dir`
  emet `summary.json`, `portal.json`, `tryit.json`, `binding.json` и др. `checksums.sha256`
  для того, чтобы рецензенты могли эффективно сравнивать результаты без проверки мониторов.
  Archivez ce repertoire sous `artifacts/sorafs/<tag>/monitoring/` с пакетом Sigstore
  и дескриптор DNS.
- Включите вылазку монитора, экспорт Grafana (`dashboards/grafana/docs_portal.json`),
  Идентификатор Drill Alertmanager в выпуске билетов для SLO DOCS-3c.
  проверяемый плюс дебил. Le playbook de мониторинг публикации dedie vit a
  `docs/portal/docs/devportal/publishing-monitoring.md`.

Для порта зондов требуется HTTPS и отбрасываются базовые URL-адреса `http://` sauf si
`allowInsecureHttp` определен в конфигурации монитора; производство/постановка «Gardez les Cibles»
по TLS и неактивному переопределению языков предварительного просмотра.

Автоматизация мониторинга через `npm run monitor:publishing` в Buildkite/cron une fois
le portail en ligne. Команда мема, создание URL-адресов, проверка чеков
Sante Continus Entre релизы.

## Автоматизация с `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` инкапсулирует этапы 2–6. Иль:1. архив `build/` в определенном архиве,
2. выполнить `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`,
   эт `proof verify`,
3. выполните опцию `manifest submit` (включая привязку псевдонима) quand
   учетные данные Torii представлены и т. д.
4. введите `artifacts/sorafs/portal.pin.report.json`, выберите опцию
  `portal.pin.proposal.json`, дескриптор DNS (после отправки),
  и шлюз привязки (`portal.gateway.binding.json` плюс блок заголовков)
  для того, чтобы обеспечить управление, сетевое взаимодействие и возможности мощного сравнения доказательств
  без ошибок в журналах CI.

Определения `PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME` и т. д. (опционально)
`PIN_ALIAS_PROOF_PATH` перед вызовом сценария. Используйте `--skip-submit` для сухой очистки.
бежит; Рабочий процесс GitHub можно выполнить с помощью ввода `perform_submit`.

## Etape 8 — Публикация спецификаций OpenAPI и комплектов SBOM

DOCS-7 требует сборки порта, спецификации OpenAPI и артефактов, проходящих через SBOM.
конвейер мемов детерминирован. Существующие помощники содействуют трем:

1. **Regenerer et Signer La Spec.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   Выпуск Fournissez un label через `--version=<label>`, когда вы хотите сохранить снимок
   исторический (пример `2025-q3`). Помощник записал снимок в моментальном снимке
   `static/openapi/versions/<label>/torii.json`, мир в мире
   `versions/current` и зарегистрируйте метаданные (SHA-256, закон манифеста, временную метку
   это мой день) в `static/openapi/versions.json`. Le portail developmentpeur lit cet index
   для панелей Swagger/RapiDoc имеется средство выбора версии и сопутствующих файлов.
   дайджест/ла подпись ассоциированного встроенного. Omettre `--version` сохраняет этикетки выпуска
   прецедент и не требуется использовать указатели `current` + `latest`.

   Манифест захвата дайджестов SHA-256/BLAKE3 для того, чтобы шлюз мог использовать
   заголовки `Sora-Proof` для `/reference/torii-swagger`.

2. **Измените SBOMs CycloneDX.** Выпуск конвейера посещает SBOMs syft
   сел `docs/source/sorafs_release_pipeline_plan.md`. Гардез-ла-Сорти
   pres des artefacts de build:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **Упаковать полезную нагрузку в CAR.**

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

   Suivez les memes etapes `manifest build` / `manifest sign` que le сайт-руководитель,
   в настройке псевдонимов для артефактов (например, `docs-openapi.sora` для спецификации
   `docs-sbom.sora` для подписи пакета SBOM). Гардер де псевдонимов различает обслуживание
   SoraDNS, GAR и т. д. ограничения отката билетов или точная полезная нагрузка.

4. **Отправить и связать.** Повторно использовать существующий авторский файл и пакет Sigstore, затем зарегистрировать его.
   Кортеж псевдонимов в выпуске контрольного списка для того, чтобы самые сильные аудиторы могли его получить
   имя Сора отображается в виде манифеста дайджеста.

Архиватор файлов манифестов спецификации/SBOM с портом сборки, обеспечивающим выпуск билета чака.
полный набор компонентов без повторного выполнения упаковщика.

### Помощник по автоматизации (скрипт CI/пакета)

`./ci/package_docs_portal_sorafs.sh` кодирует этапы 1–8 для дорожной карты объекта
**DOCS-7** — это исполняемый файл с отдельной командой. Помощник:- выполнить подготовку к порту (`npm ci`, синхронизировать OpenAPI/norito, тестировать виджеты);
- получать CAR и пары манифестов порта, OpenAPI и SBOM через `sorafs_cli`;
- выполнить опцию `sorafs_cli proof verify` (`--proof`) и подпись Sigstore
  (И18НИ00000315Х, И18НИ00000316Х, И18НИ00000317Х);
- удалить все артефакты sous `artifacts/devportal/sorafs/<timestamp>/` и др.
  ecrit `package_summary.json` для того, чтобы выпуск CI/outillage мог войти в пакет; и др.
- рафраичить `artifacts/devportal/sorafs/latest` для указателя на последнее исполнение.

Пример (комплект конвейера с Sigstore + PoR):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

Использование флагов:

- `--out <dir>` - переопределить корень артефактов (по умолчанию сохраняется временная метка файлов).
- `--skip-build` - повторно использовать существующий `docs/portal/build` (используется, когда CI не может быть использован
  воссоздать причину зеркал в автономном режиме).
- `--skip-sync-openapi` - игнорировать `npm run sync-openapi` и `cargo xtask openapi`
  не может быть, чтобы вы присоединились к crates.io.
- `--skip-sbom` - отправьте запрос `syft`, когда бинарный файл не установлен (журнал сценария
  un avertissement a la Place).
- `--proof` — выполнить `sorafs_cli proof verify` для каждой пары CAR/манифест. Лес полезные нагрузки
  multi-fichiers exigent encore le поддержка chunk-plan в CLI, не допускайте этого флага
  деактивируйте, если вы исправите ошибки `plan chunk count` и проверите управление
  une fois le ворота вверх по течению.
- `--sign` - вызвать `sorafs_cli manifest sign`. Токен Fournissez через
  `SIGSTORE_ID_TOKEN` (или `--sigstore-token-env`) или отпустите CLI-восстановитель через
  `--sigstore-provider/--sigstore-audience`.

Для создания артефактов используйте `docs/portal/scripts/sorafs-pin-release.sh`.
Il empaquete maintenant le portail, OpenAPI и SBOM, подпишите манифест и зарегистрируйтесь
дополнительные ресурсы метаданных в `portal.additional_assets.json`. Ле помощник
Comprend les Memes, ручки, опции, которые упаковщик CI, плюс новые переключатели
`--openapi-*`, `--portal-sbom-*` и `--openapi-sbom-*` для назначения кортежей псевдонимов
по артефакту, переопределить исходный SBOM через `--openapi-sbom-source`, убедиться в безопасности
полезные нагрузки (`--skip-openapi`/`--skip-sbom`), и указатель на бинарную версию `syft` не по умолчанию
авек `--syft-bin`.

Сценарий афиширует команду выполнения; Копирование журнала в выпуске билетов
avec `package_summary.json` для того, чтобы рецензенты могли лучше сравнивать дайджесты CAR,
Метаданные плана и хеши пакета Sigstore без паркинга специальной оболочки.

## Etape 9 — Шлюз проверки + SoraDNS

Прежде чем объявить о переключении, подтвердите, что новый псевдоним будет разрешен через SoraDNS и какие шлюзы
agraffent desproofs Frais:

1. **Исполнитель датчика ворот.** `ci/check_sorafs_gateway_probe.sh` упражнение
   `cargo xtask sorafs-gateway-probe` контроль демонстрационных приборов в
   `fixtures/sorafs_gateway/probe_demo/`. Для барабанов развертывания наведите курсор на зонд
   версия имени хоста:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   Le зонд декодирует `Sora-Name`, `Sora-Proof` и `Sora-Proof-Status` selon
   `docs/source/sorafs_alias_policy.md` и эхо, когда отображается манифест дайджеста,
   TTL или привязки, производные от GAR.Для облегченных выборочных проверок (например, когда только пакет переплета
   изменилось), запустите `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   Помощник проверяет захваченный пакет привязок и удобен для выпуска.
   билеты, для которых требуется только обязательное подтверждение вместо полной проверки.

2. **Собиратель улик.** Для операций по бурению или пробных прогонов PagerDuty,
   конвертируйте зонд с `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario
   devportal-rollout -- ...`. Le обертка для заготовок/бревен су
   `artifacts/sorafs_gateway_probe/<stamp>/`, встретился с `ops/drill-log.md` и др.
   (опция) отключите перехватчики отката или полезные данные PagerDuty. Определения
   `--host docs.sora` для проверки ввода SoraDNS вместо жестко заданного IP-адреса.

3. **Проверка привязок DNS.** При управлении публикацией псевдонима доказательства зарегистрируйтесь.
   Справочный документ GAR по зонду (`--gar`) и выпуск доказательств.
   Владельцы решающего устройства могут повторно использовать ввод мемов через `tools/soradns-resolver` для заливки
   s'assurer que les entrees en Cache Honorent le Nouveau Manifete. Перед объединением JSON,
   казнить
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   для проверки автономного сопоставления хоста, манифеста метаданных и меток
   телеметрия. Помощник может получить резюме `--json-out` с подписью GAR для тех, кто вам нужен.
   рецензенты ищут доказательства, поддающиеся проверке, без бинарного анализа.
  Когда вы перейдете в новую ГАР, предпочитаете
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (revenez a `--manifest-cid <cid>` уникальность, когда манифест не
  доступно). Помощник по получению обслуживания CID **et** дайджеста BLAKE3
  удалить манифест JSON, объединить пробелы, дедупликацию флагов `--telemetry-label`,
  три метки и шаблоны CSP/HSTS/Permissions-Policy по умолчанию
  d'ecrire le JSON для того, чтобы полезная нагрузка оставалась детерминированной, мем, если операторы
  захват различных этикеток и ракушек.

4. **Псевдоним наблюдателя за метриками.** Gardez `torii_sorafs_alias_cache_refresh_duration_ms`
   et `torii_sorafs_gateway_refusals_total{profile="docs"}` - экран подвесного зонда;
   Серия ces sontgraphees dans `dashboards/grafana/docs_portal.json`.

## Этап 10 – Мониторинг и сбор доказательств- **Панели мониторинга.** Экспорт `dashboards/grafana/docs_portal.json` (портал SLO),
  `dashboards/grafana/sorafs_gateway_observability.json` (шлюз задержки +
  Sante Proof), и др. `dashboards/grafana/sorafs_fetch_observability.json`
  (Санте Оркестратор) для выпуска чака. Joignez les экспортирует JSON или выпуск билетов
  чтобы рецензенты могли ответить на запросы Prometheus.
- **Проверка архивов.** Сохраните `artifacts/sorafs_gateway_probe/<stamp>/` в git-annex.
  или вы ведро доказательств. Включите проверку резюме, заголовки и полезную нагрузку PagerDuty.
  захват телеметрии по сценарию.
- **Пакет выпуска.** Имеются резюме CAR du portail/SBOM/OpenAPI, манифесты пакетов,
  подписи Sigstore, `portal.pin.report.json`, проверка журналов Try-It и отчеты о проверке ссылок
  временная метка sous un dossier (например, `artifacts/sorafs/devportal/20260212T1103Z/`).
- **Журнал тренировок.** Quand les зонды шрифт party d'un Drill, laissez
  `scripts/telemetry/run_sorafs_gateway_probe.sh` добавляет вход в `ops/drill-log.md`
  для того, чтобы мемные доказательства удовлетворяли требования хаоса SNNet-5.
- **Залог билета.** Ссылка на идентификаторы панели Grafana или файлы экспорта PNG, прикрепляемые к ним.
  билет на изменение, с возможностью проверки взаимопонимания, для влиятельных рецензентов
  восстановление SLO без доступа к оболочке.

## Этап 11 – табло с данными из нескольких источников и доказательствами

Издатель SoraFS требует обслуживания получения доказательств из нескольких источников (DOCS-7/SF-6)
avec les preuves DNS/шлюз ci-dessus. После окончания манифеста:

1. **Lancer `sorafs_fetch` против живого манифеста.** Использование плана/манифеста артефактов
   продукты на этапах 2–3 с шлюзом учетных данных для каждого поставщика.
   Продолжайте продвигать вылазки, чтобы зрители могли снова отследить решение
   оркестратор:

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

   - Восстановить ссылки поставщиков рекламы по манифесту (например,
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     et passez-les через `--provider-advert name=path`, чтобы табло оценило файлы
     окончание емкости детерминированного фасада. Утилисез
     `--allow-implicit-provider-metadata` **уникальный**, когда вы радуете светильниками в CI;
     Производство сверл les doivent citer les adverts Signes qui ont сопровождает le Pin.
   - Когда манифест ссылается на дополнительные регионы, повторите команду с файлами
     Корреспонденты поставщиков кортежей для кэша/псевдонима кэша и ассоциации извлечения артефактов.

2. **Архиватор вылетов.** Conservez `scoreboard.json`,
   `providers.ndjson`, `fetch.json` и `chunk_receipts.ndjson` являются доказательствами досье
   du релиз. Эти данные фиксируют одноранговые узлы, бюджет повторных попыток и задержку EWMA.
   и квитанции по частям, которые являются пакетом управления, остаются в силе для SF-7.

3. **Показать телеметрию.** Импортировать вылеты на приборную панель.
   **SoraFS Получить наблюдаемость** (`dashboards/grafana/sorafs_fetch_observability.json`),
   Surveillez `torii_sorafs_fetch_duration_ms`/`_failures_total` и поставщик панелей диапазона
   для аномалий. Используйте снимки Grafana или выпуск билетов с табло chemin.4. **Проверка правил оповещения.** Выполнить `scripts/telemetry/test_sorafs_fetch_alerts.sh`
   для проверки пакета оповещений Prometheus перед выпуском. Жоаньез ла вылазка
   promtool или билет для рецензентов DOCS-7, подтверждающий предупреждения о остановке и т. д.
   медленный поставщик остаточных армий.

5. **Brancher en CI.** Рабочий процесс переноса данных на этап `sorafs_fetch` derriere
   вход `perform_fetch_probe`; activez-le для запуска промежуточной подготовки/производства в ближайшее время
   Доказательства должны быть получены с пакетом манифестов без вмешательства вручную.
   Сверла могут быть повторно использованы для сценария мема и экспорта шлюза токенов и т. д.
   в определенном `PIN_FETCH_PROVIDERS` в списке поставщиков, разделенных по виртуальным параметрам.

## Продвижение, наблюдение и откат

1. **Продвижение**: охрана различных псевдонимов для постановки и производства. Promouvez ru
   повторный исполнитель `manifest submit` с манифестом/пакетом мема, смена
   `--alias-namespace/--alias-name` Указатель заливки для альтернативного производства. Села эвите
   реконструировать или переподписать, чтобы отдел контроля качества одобрил постановку штифта.
2. **Мониторинг:** импортируйте реестр контактов панели управления.
   (`docs/source/grafana_sorafs_pin_registry.json`) плюс характеристики порта зондов
   (голос `docs/portal/docs/devportal/observability.md`). Оповещение о дрейфе контрольных сумм,
   зонды, эхо-сигналы или фотографии, подтверждающие повторную попытку.
3. **Откат:** для восстановления после задержки, возобновления манифестного прецедента (или отмены
   l'псевдоним куранта) с `sorafs_cli manifest submit --alias ... --retire`.
   Gardez toujours le dernier Bundle connu comme bon et le резюме CAR pour que les
   доказательства отката могут быть восстановлены в журналах CI-турнира.

## Модель рабочего процесса CI

Как минимум, ваш конвейер должен сделать следующее:

1. Сборка + lint (`npm ci`, `npm run build`, генерация контрольных сумм).
2. Empaqueter (`car pack`) и калькулятор манифестов.
3. Подписывающее лицо с помощью токена OIDC задания (`manifest sign`).
4. Загрузчик файлов артефактов (CAR, манифест, пакет, план, резюме) для проверки.
5. Реестр Суметр-о-Пин:
   - Запросы на извлечение -> `docs-preview.sora`.
   - Теги/ветки протеже -> продвижение псевдонимов производства.
6. Выполнение зондов + ворота проверки до определения.

`.github/workflows/docs-portal-sorafs-pin.yml` соедините все эти этапы для выпуска вручную.
Рабочий процесс:

- построить/проверить порт,
- запустите сборку через `scripts/sorafs-pin-release.sh`,
- подписать/проверить манифест пакета через GitHub OIDC,
- загрузить CAR/manifeste/bundle/plan/resumes как артефакты и т. д.
- (опция) сочетание манифеста + привязки псевдонимов и присутствующих секретов.

Настройте секреты/переменные, необходимые перед разблокированием задания:| Имя | Но |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | Хост Torii, который выставляет `/v1/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | Идентификатор эпохи зарегистрируйтесь вместе с представленными материалами. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | Авторитет подписи для манифеста душа. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | Псевдоним кортежа находится в манифесте, когда `perform_submit` соответствует `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Псевдоним доказательства пакета кодируется в формате Base64 (опция; Omettre для привязки псевдонима Sauter). |
| `DOCS_ANALYTICS_*` | Существующие средства аналитики/зондов конечных точек повторно используют рабочие процессы в основном. |

Разблокируйте рабочий процесс с помощью действий пользовательского интерфейса:

1. Fournissez `alias_label` (например: `docs.sora.link`), опция `proposal_alias`,
   и переопределить опцию `release_tag`.
2. Установите флажок `perform_submit` для создания артефактов без прикосновения Torii.
   (утилита для пробных прогонов) или активация для направления публикации по настройке псевдонима.

`docs/source/sorafs_ci_templates.md` документируйте общие помощники CI для
les projets вне репозитория, но больше портального рабочего процесса doit etre la voie prepreee
для ежедневных выпусков.

## Контрольный список

- [ ] `npm run build`, `npm run test:*` и др. `npm run check:links` с верт.
- [ ] `build/checksums.sha256` и `build/release.json` захватывают артефакты.
- [ ] CAR, план, манифест и резюме Generes sous `artifacts/`.
- [ ] Bundle Sigstore + фирменные запасы с файлами журналов.
- [ ] `portal.manifest.submit.summary.json` и `portal.manifest.submit.response.json`
      захватывает все, что представлено, вместо этого.
- [ ] `portal.pin.report.json` (и опция `portal.pin.proposal.json`)
      архив с артефактами CAR/manifeste.
- [ ] Архивы журналов `proof verify` и `manifest verify-signature`.
- [ ] Панели Grafana пропущены + пробы Try-It reussis.
- [ ] Откат примечаний (прецедент идентификатора манифеста + псевдоним дайджеста)
      выпуск билетов.