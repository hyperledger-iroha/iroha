---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## جائزہ

Загрузите файл **DOCS-7** (SoraFS) или **DOCS-8**
(CI/CD в режиме реального времени) ہے۔
Сборка/проверка SoraFS для Sigstore для проверки подлинности
псевдоним پروموشن، توثیق، اور откат ڈرلز کو کور کرتی ہے تاکہ ہر предварительный просмотр اور
релиз آرٹیفیکٹ قابلِ اعادہ اور قابلِ آڈٹ ہو۔

Если вы хотите использовать `sorafs_cli` (`--features cli` کے ساتھ
build شدہ) ہے، pin-registry اجازتوں والے Torii endpoint تک رسائی ہے، اور
Sigstore کے لئے OIDC اسناد ہیں۔ طویل مدتی راز (`IROHA_PRIVATE_KEY`,
`SIGSTORE_ID_TOKEN`, Torii (например) для подключения CI لوکل رنز انہیں
экспорт оболочки

## پیشگی شرائط

- Узел 18.18+ имеет `npm` или `pnpm`.
- `sorafs_cli` جو `cargo run -p sorafs_car --features cli --bin sorafs_cli` سے حاصل ہو۔
- URL-адрес Torii `/v1/sorafs/*`
  Используйте псевдонимы جمع کر سکے۔
- Эмитент OIDC (GitHub Actions, GitLab, идентификатор рабочей нагрузки).
  `SIGSTORE_ID_TOKEN` منٹ کیا جا سکے۔
- Добавлено: пробные прогоны для `examples/sorafs_cli_quickstart.sh` на GitHub/GitLab
  рабочие процессы есть `docs/source/sorafs_ci_templates.md`.
- Попробуйте использовать OAuth с поддержкой (`DOCS_OAUTH_*`) и выберите нужную сборку.
  پروموٹ کرنے سے پہلے [контрольный список по усилению безопасности](./security-hardening.md)
  چلائیں۔ Воспользуйтесь регуляторами TTL/опроса и нажмите кнопку Опрос.
  حدود سے باہر ہوں تو پورٹل build ناکام ہو جاتا ہے؛ `DOCS_OAUTH_ALLOW_INSECURE=1`
  Просмотрите предварительные просмотры и экспортируйте пентест ثبوت ریلیز ٹکٹ کے ساتھ
  منسلک کریں۔

## مرحلہ 0 — Попробуйте پروکسی بنڈل محفوظ کریں

Предварительный просмотр шлюза Netlify پروموٹ کرنے سے پہلے Попробуйте پروکسی سورسز اور
Воспользуйтесь OpenAPI для дайджеста, который может помочь вам:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` پروکسی/probe/rollback helpers کاپی کرتا ہے،
OpenAPI подпись, которую можно использовать для `release.json`,
`checksums.sha256` لکھتا ہے۔ Доступ к шлюзу Netlify/SoraFS.
Для получения дополнительной информации используйте Torii. ہنٹس بغیر
دوبارہ build کے ری پلے کر سکیں۔ Предоставляется клиентом
носители данных (`allow_client_auth`) для развертывания CSP и CSP.
رہیں۔

## مرحلہ 1 — پورٹل build اور lint

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

`npm run build` Дополнительная информация `scripts/write-checksums.mjs`
یہ تیار کرتا ہے:

- `build/checksums.sha256` — `sha256sum -c` может быть использован для SHA256 или для SHA256.
- `build/release.json` — میٹا ڈیٹا (`tag`, `generated_at`, `source`) или
  CAR/manifest میں پِن کیا جاتا ہے۔

Автомобиль для автомобиля, автомобиль, автомобиль, сборка کیے
предварительный просмотр

## مرحلہ 2 — اسٹیٹک اثاثوں کی پیکجنگ

Автомобильный автомобиль Docusaurus آؤٹ پٹ ڈائریکٹری چلائیں۔ نیچے کی مثال تمام آرٹیفیکٹس
`artifacts/devportal/` کے تحت لکھتی ہے۔

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

Подсчет фрагментов JSON, дайджесты и советы по планированию проверки.
`manifest build` CI ڈیش بورڈز بعد میں دوبارہ استعمال کرتے ہیں۔## مرحلہ 2b — OpenAPI اور SBOM معاون پیکج کریں

DOCS-7 используется для загрузки полезных нагрузок SBOM OpenAPI.
Если вы хотите использовать шлюзы или шлюзы
`Sora-Proof`/`Sora-Content-CID` ہیڈرز اسٹپل کر سکیں۔ ریلیز ہیلپر
(`scripts/sorafs-pin-release.sh`) или OpenAPI ڈائریکٹری (`static/openapi/`)
`syft` и SBOMs, например, `openapi.*`/`*-sbom.*` CARs.
کرتا ہے اور میٹا ڈیٹا `artifacts/sorafs/portal.additional_assets.json` میں
ریکارڈ کرتا ہے۔ В зависимости от полезной нагрузки можно использовать 2-4 варианта полезной нагрузки.
اس کے اپنے префиксы اور метки метаданных
`--car-out "$OUT"/openapi.car` или `--metadata alias_label=docs.sora.link/openapi`).
DNS-сервер может использовать манифест/псевдоним Torii, а также манифест/псевдоним Torii.
OpenAPI, پورٹل SBOM, OpenAPI SBOM) Шлюз, который используется для обеспечения безопасности
скрепленные корректуры فراہم کر سکے۔

## مرحلہ 3 — مینی فیسٹ بنائیں

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

Pin-policy فلیگز کو کریں
канарейки کے لئے `--pin-storage-class hot`)۔ Проверка кода JSON
کے لئے سہولت دیتا ہے۔

## مرحلہ 4 — Sigstore سے سائن کریں

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

Bundle, дайджест, дайджест фрагмента, OIDC, возможность использования хэша BLAKE3.
کرتا ہے بغیر JWT کیے۔ комплект اور отдельная подпись دونوں سنبھال کر
رکھیں؛ продвижение продукции
کر سکتی ہیں۔ Обратитесь к поставщику услуг `--identity-token-env` с запросом на подключение.
ہیں (یا ماحول میں `SIGSTORE_ID_TOKEN` سیٹ کر سکتے ہیں) جب کوئی بیرونی OIDC
ہیلپر ٹوکن جاری کرے۔

## مرحلہ 5 — штифт رجسٹری میں جمع کرائیں

Используйте этот план (план фрагментов) Torii. Краткое содержание طلب کریں
تاکہ رجسٹری запись/доказательство псевдонима قابلِ آڈٹ رہے۔

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

Предварительный просмотр یا canary псевдоним (`docs-preview.sora`) رول آؤٹ کریں تو منفرد alias کے
Отправка материалов и контроль качества продукции, а также контроль качества и контроль качества.

Привязка псевдонимов может быть использована в следующих случаях: `--alias-namespace`, `--alias-name`,
`--alias-proof`۔ Псевдоним درخواست منظور ہونے پر комплект доказательств (base64 یا
Norito байт) تیار کرتی ہے؛ Секреты CI в разделе `manifest submit`
Если вы хотите, чтобы вы знали, как это сделать, جب آپ صرف مینی فیسٹ pin کرنا
Выберите DNS-сервер или псевдоним, который вы хотите использовать.

## مرحلہ 5b — گورننس پروپوزل تیار کریں

ہر مینی فیسٹ کے ساتھ پارلیمنٹ کے لئے تیار پروپوزل ہونا چاہیے کہ کوئی بھی
Сора خصوصی اسناد ادھار لئے بغیر تبدیلی پیش کر سکے۔ отправить/подписать مراحل کے
Обратите внимание:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` کینونیکل `RegisterPinManifest` ہدایت, дайджест фрагмента,
پالیسی, اور псевдоним подсказка کو محفوظ کرتا ہے۔ اسے گورننس ٹکٹ یا پارلیمنٹ پورٹل کے
Вы можете создать оптимальную полезную нагрузку для создания полезной нагрузки.
دیکھ سکیں۔ Если вам нужен ключ авторизации Torii, вы можете использовать ключ авторизации.
شہری لوکل طور پر پروپوزل ڈرافٹ کر سکتا ہے۔

## مرحلہ 6 — доказательства اور ٹیلیمیٹری کی تصدیق

Для закрепления требуется детерминированная проверка:

```bash
sorafs_cli proof verify \
  --manifest "$OUT"/portal.manifest.to \
  --car "$OUT"/portal.car \
  --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
  --manifest "$OUT"/portal.manifest.to \
  --bundle "$OUT"/portal.manifest.bundle.json \
  --chunk-plan "$OUT"/portal.plan.json
```- `torii_sorafs_gateway_refusals_total`
  `torii_sorafs_replication_sla_total{outcome="missed"}` Возможные аномалии
- `npm run probe:portal` Попробуйте попробовать, как это сделать.
  نئے булавка شدہ مواد کے خلاف پرکھا جا سکے۔
- [Публикация и мониторинг](./publishing-monitoring.md)
  Есть возможность использовать DOCS-3c для обеспечения наблюдаемости и удобства наблюдения.
  پورا ہو۔ ہیلپر اب متعدد `bindings` записей (سائٹ، OpenAPI, SBOM, OpenAPI
  SBOM) Если вам нужен хост `hostname`, вам нужен хост
  `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` نافذ کرتا ہے۔ نیچے والی призыв
  Пакет доказательств в формате JSON (`portal.json`, `tryit.json`, `binding.json`,
  اور `checksums.sha256`) Вот как можно использовать:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## مرحلہ 6a — шлюз سرٹیفکیٹس کی منصوبہ بندی

TLS SAN/вызов для GAR и DNS-сервера для шлюза
منظور کنندگان ایک ہی شواہد دیکھیں۔ В комплект поставки входит DG-3 с зеркалом.
Канонические хосты с подстановочными знаками, сети SAN с красивыми хостами, сервер DNS-01 и многое другое.
ACME бросает вызов شمار کرتا ہے:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

JSON-файл может помочь вам в выборе (где вы можете использовать JSON, чтобы получить больше информации)
Torii или `torii.sorafs_gateway.acme`, например, в Сан-Франциско или в Сан-Франциско.
GAR позволяет использовать канонические/красивые отображения и производные хостов.
تصدیق کر سکیں۔ اسی ریلیز میں پروموٹ ہونے والے ہر суффикс کے لئے اضافی `--name`
دلائل شامل کریں۔

## مرحلہ 6b — канонические сопоставления хостов

Полезные нагрузки GAR کے سانچوں سے پہلے ہر alias کے لئے детерминированное сопоставление хостов ریکارڈ
کریں۔ `cargo xtask soradns-hosts` ہر `--name` کو اس کے канонический لیبل
(`<base32>.gw.sora.id`) Подстановочный знак (`*.gw.sora.id`) или подстановочный знак (`*.gw.sora.id`)
ہے، اور Pretty Host (`<alias>.gw.sora.name`) اخذ کرتا ہے۔ ریلیز آرٹیفیکٹس میں
Для создания карты DG-3 для отправки GAR необходимо картографирование.
Ответ:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

Привязка шлюза GAR к JSON и хосты
ناکامی کے لئے `--verify-host-patterns <file>` استعمال کریں۔ ہیلپر متعدد
Проверка или вызов шаблона GAR اور
скрепленный `portal.gateway.binding.json` Для ворса можно использовать:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

DNS/шлюз Сводная информация о проверке JSON
Канонические, подстановочные знаки, красивые хосты и производные хосты.
چلائے بغیر کر سکیں۔ جب بھی بنڈل میں نئے псевдонимы شامل ہوں تو یہ کمانڈ دوبارہ
چلائیں تاکہ بعد کے GAR اپڈیٹس میں وہی След доказательств

## مرحلہ 7 — Дескриптор переключения DNS.

переключение производства کامیاب
отправка (привязка псевдонима) کے بعد ہیلپر `artifacts/sorafs/portal.dns-cutover.json`
Вот несколько слов о том, как:- метаданные привязки псевдонимов (пространство имен/имя/доказательство, дайджест манифеста, URL-адрес Torii,
  представленная эпоха, авторитет);
- ریلیز سیاق و سباق (тег, метка псевдонима, пути манифеста/CAR, план фрагментов, пакет Sigstore);
- указатели проверки (псевдоним зонда + конечная точка Torii);
- Функция управления изменениями (идентификатор заявки, окно переключения, контакт с операторами,
  рабочее имя хоста/зона);
- сшитые метаданные продвижения маршрута `Sora-Route-Binding` ہیڈر سے اخذ کردہ
  (канонический хост/CID, заголовок + пути привязки, команды проверки), а также GAR
  продвижение по службе اور запасные упражнения ایک ہی شواہد کا حوالہ دیں؛
- сгенерированный план маршрута آرٹیفیکٹس (`gateway.route_plan.json`, шаблоны заголовков,
  Для отката заголовков) Возможность изменения билетов и перехватчики CI ہر DG-3
  Каноническое продвижение/откат или каноническое продвижение или откат
- Запретить метаданные аннулирования кэша (очистить конечную точку, переменную аутентификации, полезную нагрузку JSON,
  Это `curl` کمانڈ). اور
- подсказки по откату и дескриптор (тег выпуска и дайджест манифеста).
  شارہ کریں تاکہ изменить билеты میں детерминированный запасной путь

Очистка кэша или дескриптор переключения, например, канонический
Ответ:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

В комплект поставки входит `portal.cache_plan.json` для DG-3, который может быть использован в качестве источника питания.
Детерминированные хосты/пути (подсказки для аутентификации) или `PURGE`
просит بھیجیں۔ дескриптор или метаданные кэша, необходимые для хранения данных
Управление изменениями и контроль над конечными точками
رہیں جو Cutover کے دوران flash کیے جاتے ہیں۔

ہر DG-3 پیکٹ کو продвижение + контрольный список отката بھی درکار ہوتی ہے۔ اسے
`cargo xtask soradns-route-plan` — возможность управления изменениями
Псевдоним: предполетная подготовка, переключение, откат и другие действия:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

`gateway.route_plan.json` میں canonical/pretty hosts, поэтапная проверка работоспособности
Привязка GAR, очистка кэша, действия отката и многое другое.
GAR/привязка/переключение Операции по сценарию
قدموں کی مشق اور منظوری دے سکیں۔

`scripts/generate-dns-cutover-plan.mjs` — дескриптор, который может быть использован в качестве дескриптора
`sorafs-pin-release.sh` خودکار طور پر چلتا ہے۔ دستی طور پر دوبارہ بنانے
Есть несколько вариантов:

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

Помощник по выводам Дополнительная информация о дополнительных метаданных и переменных среды
Ответ:

| Переменная | Цель |
|----------|---------|
| `DNS_CHANGE_TICKET` | дескриптор идентификатора билета. |
| `DNS_CUTOVER_WINDOW` | Окно переключения ISO8601 (код `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | производственное имя хоста + авторитетная зона۔ |
| `DNS_OPS_CONTACT` | псевдоним дежурного по вызову یا контакт для эскалации ۔ |
| `DNS_CACHE_PURGE_ENDPOINT` | Дескриптор или конечная точка очистки кэша. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Токен очистки используется для env var (по умолчанию: `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | Метаданные отката или дескриптор переключения, а также путь. |Проверка изменения DNS Проверка JSON Проверка наличия дайджестов манифеста утверждающих, псевдонимов
привязки, пробник или CI, проверка подлинности Флаги CLI
И18НИ00000174Х, И18НИ00000175Х, И18НИ00000176Х,
И18НИ00000177Х, И18НИ00000178Х, И18НИ00000179Х,
`--cache-purge-auth-env`, или `--previous-dns-plan` وہی переопределяет فراہم کرتے ہیں
جب helper کو کے باہر چلایا جائے۔

## مرحلہ 8 — скелет файла зоны преобразователя بنائیں (اختیاری)

Окно переключения производства: скелет файла зоны SNS.
фрагмент резольвера DNS-записи и метаданные
Переменные среды и параметры CLI вспомогательный дескриптор переключения
بننے کے فوراً بعد `scripts/sns_zonefile_skeleton.py` چلائے گا۔ Как это сделать?
A/AAAA/CNAME в дайджесте GAR (полезная нагрузка GAR — BLAKE3-256)
کریں۔ Зона/имя хоста معلوم ہوں اور `--dns-zonefile-out` چھوڑ دیا جائے تو
helper `artifacts/sns/zonefiles/<zone>/<hostname>.json` پر لکھتا ہے اور
`ops/soradns/static_zones.<hostname>.json` — фрагмент резольвера, который можно использовать для проверки.

| Переменная/флаг | Цель |
|-----------------|---------|
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | Скелет файла зоны и путь. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Путь к фрагменту резольвера (по умолчанию `ops/soradns/static_zones.<hostname>.json`)۔ |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | Установите флажок TTL (по умолчанию: 600 секунд). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | Адреса IPv4 (разделенные запятыми env или повторяемый флаг CLI)۔ |
| И18НИ00000195Х, И18НИ00000196Х | IPv6-адреса۔ |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | Использование цели CNAME۔ |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | Контакты SHA-256 SPKI (base64)۔ |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Записи TXT (`key=value`)۔ |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | вычисленная версия файла зоны может быть переопределена |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | Начало переключения окна `effective_at` временная метка (RFC3339) или принудительное завершение |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | метаданные میں ریکارڈ доказательство буквального переопределения کریں۔ |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | Переопределение метаданных Переопределение CID |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Состояние заморозки Guardian (мягкое, жесткое, оттаивание, мониторинг, чрезвычайная ситуация). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | зависает کے لئے Справочник по билетам опекуна/совета۔ |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | оттаивание в соответствии с временной меткой RFC3339. |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | اضافی заморозить примечания (разделенный запятыми env یا повторяемый флаг)۔ |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | подписанная полезная нагрузка GAR کا BLAKE3-256 дайджест (шестнадцатеричный) привязки шлюза ہونے پر لازم۔ |

Рабочий процесс действий GitHub
производственный контакт خودکار طور پر Zonefile آرٹیفیکٹس بنائے۔ درج ذیل секреты/ценности
کنفیگر کریں (строки میں многозначные فیلڈز کے لئے, разделенные запятыми فہرستیں ہو سکتی ہیں):| Секрет | Цель |
|--------|---------|
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | helper کو دیا جانے والا имя хоста/зоны производства |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | дескриптор میں محفوظ псевдоним дежурного вызова۔ |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | Доступ к записям IPv4/IPv6. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | Использование цели CNAME۔ |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Контакты base64 SPKI |
| `DOCS_SORAFS_ZONEFILE_TXT` | Записи TXT۔ |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | скелет میں محفوظ заморозить метаданные۔ |
| `DOCS_SORAFS_GAR_DIGEST` | подписанная полезная нагрузка GAR или дайджест BLAKE3 в шестнадцатеричной кодировке. |

`.github/workflows/docs-portal-sorafs-pin.yml` Дополнительная информация `dns_change_ticket`
`dns_cutover_window` inputs فراہم کریں تاکہ окно изменения дескриптора/файла зоны.
метаданные لے۔ Как провести сухие прогоны или нет

Вызов (runbook владельца SN-7 کے مطابق):

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

helper خودکار طور پر изменить билет کو запись TXT کے طور پر لے جاتا ہے اور Cutover
Начало окна کو `effective_at` временная метка کے طور پر наследует کرتا ہے جب تک اسے
переопределить نہ کیا جائے۔ Как работает рабочий процесс?
`docs/source/sorafs_gateway_dns_owner_runbook.md` دیکھیں۔

### DNS-подключение

скелет файла зоны صرف زون کے авторитетный ریکارڈز متعین کرتا ہے۔ عام انٹرنیٹ کو
Для этого требуется родительская зона и делегирование NS/DS, а также DNS-сервер.
پر الگ سے سیٹ کرنا ضروری ہے۔
- переключение apex/TLD или ALIAS/ANAME (зависит от поставщика)
  Какие IP-адреса Anycast могут быть выбраны с помощью A/AAAA или нет?
- Получение производного красивого хоста (`<fqdn>.gw.sora.name`) с использованием CNAME
  شائع کریں۔
- Шлюз канонического хоста (`<hash>.gw.sora.id`)
  پبلک زون میں شائع نہیں ہوتا۔

### Шлюз ہیڈر ٹیمپلیٹ

развернуть помощник `portal.gateway.headers.txt` или `portal.gateway.binding.json` بھی
В DG-3 есть требования к привязке содержимого шлюза, например:

- `portal.gateway.headers.txt` Блок заголовка HTTP
  `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS,
  Дескриптор `Sora-Route-Binding` شامل ہیں) جسے Edge Gates ہر response کے
  ساتھ сшитый کرتے ہیں۔
- `portal.gateway.binding.json` وہی معلومات машиночитаемый شکل میں کارڈ کرتا
  Можно изменить билеты, автоматизировать привязки хоста/цида и оболочку, а также выполнить необходимые действия.
  Разница в различиях

Используйте `cargo xtask soradns-binding-template` для получения псевдонима манифеста, шлюза
имя хоста کو محفوظ کرتے ہیں جو `sorafs-pin-release.sh` کو فراہم کیے گئے تھے۔
Блок заголовка Можно настроить, а также настроить:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

`--csp-template`, `--permissions-template`, یا `--hsts-template` по умолчанию
шаблоны заголовков могут переопределить директивы развертывания и другие директивы.
В качестве переключателей `--no-*` можно указать заголовок, который можно использовать.

фрагмент заголовка и запрос на изменение CDN
конвейер автоматизации шлюза میں подача کریں تاکہ اصل продвижение хоста ریلیز доказательства
سے میل کھائے۔

xtask helper اب canonical راستہ ہے۔ ریلیز اسکرپٹ помощник по проверке خودکار طور
Если вы хотите использовать DG-3, вы можете использовать доказательства, которые вы хотите использовать. Добавление привязки JSON
Вот несколько примеров того, как можно использовать:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```Полезная нагрузка скреплена `Sora-Proof` или полезная нагрузка `Sora-Route-Binding`
метаданные, манифест CID + имя хоста, изменение заголовка, изменение заголовка
ہو تو فوراً неудача کرتی ہے۔ Если вам нужен CI, вы можете использовать его, чтобы получить больше информации.
Быстрое развертывание и установка DG-3 для установки
Переключение привязки سے پہلے validate ہوا تھا۔

> **Интеграция DNS-дескриптора:** `portal.dns-cutover.json` или `gateway_binding`
> سیکشن شامل کرتا ہے ان آرٹیفیکٹس (пути, CID контента, статус подтверждения и т. д.)
> буквальный шаблон заголовка) کی طرف اشارہ کرتا ہے **اور** `route_plan` строфа
> `gateway.route_plan.json` Шаблоны заголовков основного + отката.
> ہر DG-3 обменять билет
> `Sora-Name`/`Sora-Proof`/`CSP` может быть использовано в качестве дополнительного оборудования. کہ
> Продвижение/откат маршрута. Пакет доказательств.
> создать архив کھولے۔

## مرحلہ 9 — мониторы публикации چلائیں

روڈ میپ ٹاسک **DOCS-3c** کے لئے مسلسل доказательство درکار ہے کہ پورٹل, Попробуйте это پروکسی،
Привязки шлюза В формате 7-8 в составе консолидированной команды
Монитор или запланированные зонды, а также провода:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- Конфигурация `scripts/monitor-publishing.mjs` для настройки (схема کے لئے
  `docs/portal/docs/devportal/publishing-monitoring.md` دیکھیں) اور تین проверяет
  Дополнительные возможности: проверка пути к порталу + проверка политики CSP/разрешений, попробуйте прокси-сервер.
  зонды (проверка конечной точки `/metrics`), а также верификатор привязки шлюза
  (`cargo xtask soradns-verify-binding`) Проверка псевдонима/манифеста
  Sora-Content-CID — дополнительная информация + возможность получения дополнительной информации
- Можно использовать зонд или ненулевое значение, а также CI, cron jobs,
  Псевдонимы операторов runbook
- `--json-out` Сводная информация о полезной нагрузке JSON ہے جس میں и target status ہے؛
  `--evidence-dir` `summary.json`, `portal.json`, `tryit.json`, `binding.json`, اور
  `checksums.sha256` Для мониторинга мониторов.
  Разница в различиях Если вам нужен `artifacts/sorafs/<tag>/monitoring/`,
  Пакет Sigstore. Дескриптор переключения DNS.
- монитор آؤٹ پٹ, Grafana экспорт (`dashboards/grafana/docs_portal.json`), اور
  Идентификатор тренировки Alertmanager Как узнать, как работает DOCS-3c SLO
  آڈٹ ہو سکے۔ специальная книга для мониторинга публикаций یہاں ہے:
  `docs/portal/docs/devportal/publishing-monitoring.md`۔

Проверка портала HTTPS-проверка или проверка URL-адресов `http://` или проверка URL-адресов
Конфигурация монитора: `allowInsecureHttp`. производственные/постановочные цели
Если TLS не используется, переопределите предварительный просмотр, чтобы просмотреть информацию

Для создания файла Buildkite/cron используйте `npm run monitor:publishing`.
автоматизировать یہی کمانڈ، جب URL-адреса продукции с указанным ہو، وہ صحت مند چیکس فراہم
Используйте SRE/Docs, чтобы узнать больше о том, как это сделать.

## `sorafs-pin-release.sh` کے ساتھ آٹومیشن

`docs/portal/scripts/sorafs-pin-release.sh` для 2–6 вариантов инкапсуляции. یہ:1. `build/` в детерминированном архиве для хранения данных.
2. И18НИ00000283Х, И18НИ00000284Х, И18НИ00000285Х, И18НИ00000286Х,
   Код `proof verify`
3. Учетные данные Torii для ввода учетных данных `manifest submit` (псевдоним привязки سمیت)
   چلاتا ہے، اور
4. `artifacts/sorafs/portal.pin.report.json`, или `portal.pin.proposal.json`,
  Дескриптор переключения DNS (представленные материалы) и пакет привязки шлюза
  (`portal.gateway.binding.json` + текстовый блок заголовка)
  сети, работа с CI, набор доказательств, набор доказательств, возможность использования CI

`PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME`, اور (اختیاری)
`PIN_ALIAS_PROOF_PATH` سیٹ کریں۔ Сухие прогоны `--skip-submit`
Рабочий процесс GitHub Как изменить `perform_submit`

## Номер 8 — характеристики OpenAPI в комплектах SBOM.

DOCS-7 имеет встроенную сборку, спецификацию OpenAPI, а также версию SBOM.
детерминированный конвейер سے گزریں۔ Дополнительные помощники, которые помогут вам:

1. **Спецификация دوبارہ بنائیں اور سائن کریں۔**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   جب بھی تاریخی snapshot محفوظ کرنا ہو تو `--version=<label>` کے ذریعے ریلیز
   لیبل فراہم کریں (مثلاً `2025-q3`)۔ вспомогательный снимок کو
   `static/openapi/versions/<label>/torii.json` Как получить доступ к `versions/current`
   Зеркало с метаданными (SHA-256, статус манифеста, обновленная метка времени)
   `static/openapi/versions.json` میں ریکارڈ کرتا ہے۔ Индекс پورٹل یہ پڑھتا
   Используйте Swagger/RapiDoc для выбора версии или выберите дайджест/подпись
   информация inline پیش کر سکیں۔ `--version` چھوڑ دینے سے پچھلے ریلیز لیبل برقرار رہتے
   Используйте указатели `current` + `latest`.

   Манифест дайджеста SHA-256/BLAKE3 находится в шлюзе `/reference/torii-swagger`
   Заголовки `Sora-Proof` скреплены скобами.

2. **CycloneDX SBOMs и SBOMs на базе syft.
   رکھتی ہے، جیسا کہ `docs/source/sorafs_release_pipeline_plan.md` میں درج ہے۔
   Для создания артефактов можно использовать:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **Полезная нагрузка в зависимости от автомобиля**

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

   Для этого необходимо использовать `manifest build` / `manifest sign`.
   ہر اثاثے کے لئے псевдонимы ٹیون کریں (مثلاً spec کے لئے `docs-openapi.sora` اور
   سائن شدہ SBOM بنڈل کے لئے `docs-sbom.sora`)۔ Псевдонимы и доказательства SoraDNS,
   GARs, откат и изменение полезной нагрузки

4. **Добавьте привязку کریں۔** وہی Authority + Sigstore Bundle.
   Контрольный список для проверки псевдонимов кортежа
   Сора نام کس дайджест манифеста سے منسلک ہے۔

Манифесты Spec/SBOM для сборки или сборки
Воспользуйтесь услугами упаковщика, который будет работать с упаковщиком.

### آٹومیشن ہیلپر (CI/сценарий пакета)

`./ci/package_docs_portal_sorafs.sh` для 1–8 кодирования данных
**DOCS-7** может быть полезен при работе с **DOCS-7**. ہیلپر:- Подготовка к подготовке (`npm ci`, синхронизация OpenAPI/norito, тесты виджетов);
- OpenAPI, SBOM CAR + пары манифеста, `sorafs_cli` или `sorafs_cli`, например, `sorafs_cli`.
- اختیاری طور پر `sorafs_cli proof verify` (`--proof`) اور Sigstore подписание
  (`--sign`, `--sigstore-provider`, `--sigstore-audience`)
- تمام آرٹیفیکٹس کو `artifacts/devportal/sorafs/<timestamp>/` کے تحت چھوڑتا ہے اور
  `package_summary.json` Доступ к инструментам CI/выпуска для загрузки данных اور
- `artifacts/devportal/sorafs/latest` کو ریفریش کر کے تازہ ترین رن کی طرف اشارہ کرتا ہے۔

Пример (Sigstore + PoR для конвейера):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

قابلِ توجہ флаги:

- `--out <dir>` – переопределение корня артефакта کریں (отметка времени по умолчанию فولڈرز رکھتا ہے)۔
- `--skip-build` – موجودہ `docs/portal/build` دوبارہ استعمال کریں (جب CI آف لائن
  зеркала کی وجہ سے rebuild نہ کر سکے)۔
- `--skip-sync-openapi` – `npm run sync-openapi` Можно использовать для `cargo xtask openapi`
  crates.io تک نہ پہنچ سکے۔
- `--skip-sbom` – جب `syft` بائنری انسٹال نہ ہو تو اسے نہ چلائیں (اسکرپٹ alert دیتا ہے)۔
- `--proof` – ہر CAR/manifest جوڑی کے لئے `sorafs_cli proof verify` چلائیں۔ многофайловый
  payloads Интерфейс командной строки и chunk-plan Использование `plan chunk count`
  ошибки آئیں تو یہ فلیگ بند رکھیں اور восходящие ворота آنے پر دستی تصدیق کریں۔
- `--sign` – `sorafs_cli manifest sign` چلائیں۔ ٹوکن `SIGSTORE_ID_TOKEN` سے دیں
  (`--sigstore-token-env`) CLI или `--sigstore-provider/--sigstore-audience`
  کے ذریعے اسے принести کرنے دیں۔

производственные артефакты کے لئے `docs/portal/scripts/sorafs-pin-release.sh` استعمال کریں۔
Воспользуйтесь OpenAPI, а также полезными нагрузками SBOM.
`portal.additional_assets.json` может быть использован для метаданных или метаданных.
helper وہی اختیاری Ручки سمجھتا ہے جو CI packager استعمال کرتا ہے، ساتھ ہی نئے
`--openapi-*`, `--portal-sbom-*`, или `--openapi-sbom-*` переключатели.
ہر اثاثے کے لئے псевдоним кортежей مقرر کر سکیں، `--openapi-sbom-source` کے ذریعے SBOM
Переопределение полезных нагрузок и выбор полезных нагрузок (`--skip-openapi`/`--skip-sbom`),
Если не по умолчанию `syft`, можно использовать `--syft-bin`.

یہ اسکرپٹ ہر کمانڈ دکھاتا ہے جو وہ چلاتا ہے؛ log `package_summary.json` کے ساتھ
Используйте дайджесты CAR, метаданные плана, пакет Sigstore
хэши в специальной оболочке или в специальной оболочке

## Шаг 9 — Шлюз + SoraDNS.

переключение, чтобы получить доступ к псевдониму SoraDNS и разрешить разрешение.
رہا ہے اور шлюзы تازہ доказательства сшиты کر رہے ہیں:

1. **зондовый затвор** `ci/check_sorafs_gateway_probe.sh` необходимые приспособления
   `cargo xtask sorafs-gateway-probe` چلاتا ہے جو
   `fixtures/sorafs_gateway/probe_demo/` میں موجود ہیں۔ حقیقی развертывания کے لئے
   проверьте имя хоста и укажите:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   датчик `Sora-Name`, `Sora-Proof`, или `Sora-Proof-Status`.
   `docs/source/sorafs_alias_policy.md` позволяет декодировать дайджест манифеста,
   TTL, привязки GAR и дрейф, а также привязка GAR и дрейф.Для облегченных выборочных проверок (например, когда только пакет переплета
   изменилось), запустите `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   Помощник проверяет захваченный пакет привязок и удобен для выпуска.
   билеты, для которых требуется только обязательное подтверждение вместо полной проверки.

2. **доказательства бурения جمع کریں۔** آپریٹر یا PagerDuty пробные прогоны کے لئے کو
   `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario devportal-rollout -- …`
   Обертывание обертыванием заголовки/журналы оболочки
   `artifacts/sorafs_gateway_probe/<stamp>/` Информационная поддержка `ops/drill-log.md`
   Доступны перехватчики отката и полезные нагрузки PagerDuty. СораDNS
   Воспользуйтесь IP-жестким кодом `--host docs.sora`.

3. **Привязки DNS для проверки** Для проверки псевдонима и проверки пробы
   حوالہ دیا گیا GAR فائل (`--gar`) Используйте доказательства, которые могут вам понадобиться.
   Резольвер может быть установлен в `tools/soradns-resolver`.
   Кешированные записи и манифест, честь и т. д. Формат JSON для просмотра изображений
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   Детерминированное сопоставление хостов, метаданные манифеста и метки телеметрии в автономном режиме.
   подтвердить ہو جائیں۔ помощник подписал GAR کے ساتھ `--json-out` резюме بھی نکال سکتا ہے
   تاکہ рецензенты بغیر двоичный کھولے قابلِ تصدیق доказательства حاصل کریں۔
  نیا GAR بناتے وقت ترجیح دیں
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (`--manifest-cid <cid>` پر صرف تب جائیں جب Manifest فائل دستیاب نہ ہو)۔ помощник اب
  CID **اور** BLAKE3 дайджест манифеста JSON и пробелы
  обрезка کرتا ہے، دہرے `--telemetry-label` flags ختم کرتا ہے، labels sort کرتا ہے، اور
  Формат JSON для шаблонов CSP/HSTS/Permissions-Policy по умолчанию.
  детерминированная полезная нагрузка

4. **метрики псевдонимов پر نظر رکھیں۔** `torii_sorafs_alias_cache_refresh_duration_ms`
   Для `torii_sorafs_gateway_refusals_total{profile="docs"}` нужен зонд
   اسکرین پر رکھیں؛ Серия `dashboards/grafana/docs_portal.json`

## Глава 10 — Мониторинг сбора доказательств

- **Панели мониторинга** ہر ریلیز کے لئے `dashboards/grafana/docs_portal.json` (портальные SLO),
  `dashboards/grafana/sorafs_gateway_observability.json` (задержка шлюза + проверка работоспособности),
  اور `dashboards/grafana/sorafs_fetch_observability.json` (здоровье оркестратора)
  экспорт کریں۔ Экспорт в формате JSON может выполнять экспорт и экспортировать файлы в формате JSON.
  Prometheus запросы
- **Архивы зондов** `artifacts/sorafs_gateway_probe/<stamp>/` в git-приложении یا
  Ведро для доказательств میں رکھیں۔ сводка зонда, заголовки, сценарий телеметрии سے
  Использование полезной нагрузки PagerDuty
- **Пакет выпуска** پورٹل/SBOM/OpenAPI Сводки CAR, пакеты манифеста, Sigstore
  подписи, `portal.pin.report.json`, журналы проб Try-It, отчеты о проверке ссылок.
  Дата с отметкой времени (مثال `artifacts/sorafs/devportal/20260212T1103Z/`)
- **Журнал бурения** جب зонды کسی сверло کا حصہ ہوں تو
  `scripts/telemetry/run_sorafs_gateway_probe.sh` или `ops/drill-log.md`, добавьте
  Наличие доказательств SNNet-5, требование хаоса.
- **Ссылки на билеты** Изменить билет میں Grafana Идентификаторы панелей Прикрепленный экспорт PNG
  Путь к отчету о проверке Доступ к оболочке рецензентов
  Перекрестная проверка SLO

## مرحلہ 11 — данные по выборке из нескольких источников اورSoraFS для получения доказательств из нескольких источников (DOCS-7/SF-6)
Проверьте, есть ли доказательства DNS/шлюза или нет. Пин-код манифеста, который можно использовать:

1. **действующий манифест или `sorafs_fetch`, или ** или 2-3 плана/манифеста.
   Укажите учетные данные шлюза или учетные данные шлюза. ہر
   Для этого вам понадобится оркестратор, тропа принятия решений или повторный просмотр:

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

   - манифест или реклама поставщика услуг или реклама (مثلاً
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     اور انہیں `--provider-advert name=path` کے ذریعے پاس کریں تاکہ табло
     детерминированные окна возможностей оценки
     `--allow-implicit-provider-metadata` **صرف** CI میں повторы матчей کرتے وقت
     استعمال کریں؛ производственные буры میں штифт کے ساتھ آنے والے подписанные рекламные объявления ہی دیں۔
   - Манифест в регионах и кортежах провайдера может быть отключен.
     Доступ к кэшу/псевдониму и сопоставлению артефакта выборки.

2. ** پٹس آرکائیو کریں۔** `scoreboard.json`, `providers.ndjson`, `fetch.json`, اور
   `chunk_receipts.ndjson` کو ریلیز доказательства یہ فائلیں сверстник
   взвешивание, бюджет повторных попыток, задержка EWMA, получение поквартальных квитанций и многое другое.
   Вы можете использовать SF-7 в качестве источника питания

3. **Чтобы получить результаты**, выберите выходные данные: **SoraFS Fetch Observability**
   Установите флажок (`dashboards/grafana/sorafs_fetch_observability.json`) для получения дополнительной информации.
   `torii_sorafs_fetch_duration_ms`/`_failures_total` — панели поставщика услуг.
   аномалии Grafana Снимки панели, как показано на пути к табло, как показано на рисунке.
   لنک کریں۔

4. **Правила оповещения о курении** `scripts/telemetry/test_sorafs_fetch_alerts.sh`
   Проверьте пакет оповещений Prometheus, проверьте пакет оповещений. промтул
   Если вы хотите использовать DOCS-7 для установки в стойле اور
   оповещения о медленном провайдере

5. **Провод проводов CI** Рабочий процесс вывода `sorafs_fetch` или `perform_fetch_probe`
   ввод کے پیچھے رکھا گیا ہے؛ постановка/продюсирование رنز میں اسے فعال کریں تاکہ fetch
   Пакет манифеста доказательств کے ساتھ خودکار طور پر تیار ہو۔ لوکل сверла وہی اسکرپٹ
   Доступ к экспорту токенов шлюза `PIN_FETCH_PROVIDERS`
   список поставщиков, разделенный запятыми سے سیٹ کیا جائے۔

## پروموشن, наблюдаемость, откат

1. **پروموشن:** постановка производства کے لئے علیحدہ псевдонимы رکھیں۔ پروموشن کے لئے
   Манифест/пакет может быть установлен `manifest submit`.
   `--alias-namespace/--alias-name` — производственный псевдоним اس سے QA منظور
   شدہ промежуточный штифт کے بعد دوبارہ build یا re-sign کی ضرورت نہیں رہتی۔
2. **Мониторинг:** регистрация контактов
   (`docs/source/grafana_sorafs_pin_registry.json`) Дополнительные датчики
   (`docs/portal/docs/devportal/observability.md` دیکھیں) дрейф контрольной суммы,
   неудачные проверки, всплески повторных попыток и оповещения.
3. **Откат:** Нажмите «Отправить манифест» и отправьте «Псевдоним
   выйти на пенсию کریں) `sorafs_cli manifest submit --alias ... --retire` کے ذریعے۔ ہمیشہ
   последний заведомо исправный комплект اور CAR summary
   доказательства отката دوبارہ بن سکیں۔

## Шаблон рабочего процесса CI

Что такое трубопровод или трубопровод:1. Сборка + lint (`npm ci`, `npm run build`, генерация контрольной суммы).
2. Пакет (`car pack`) содержит манифесты вычислений.
3. OIDC на уровне задания с указанием знака (`manifest sign`).
4. Аудит артефактов и ошибок (CAR, манифест, пакет, план, сводки).
5. PIN-код реестра и отправьте сообщение:
   - Запросы на извлечение → `docs-preview.sora`.
   - Теги/защищенные ветки → продвижение псевдонима продукции.
6. Выход из пробников + ворота проверки доказательств.

`.github/workflows/docs-portal-sorafs-pin.yml` выпускает руководства کے لئے یہ تمام
مراحل جوڑتا ہے۔ рабочий процесс:

- Время сборки/тестирования
- `scripts/sorafs-pin-release.sh` کے ذریعے build پیکج کرتا ہے،
- GitHub OIDC Проверьте наличие пакета манифеста سائن/verify کرتا ہے،
- Сводки CAR/манифеста/пакета/плана/доказательства и артефакты, которые можно использовать или использовать для проверки.
- (اختیاری) секреты موجود ہوں تو манифест + привязка псевдонима отправить کرتا ہے۔

Работа может быть связана с секретами/переменными хранилища:

| Имя | Цель |
|------|---------|
| `DOCS_SORAFS_TORII_URL` | Torii хост или `/v1/sorafs/pin/register` ظاہر کرتا ہے۔ |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | материалы کے ساتھ ریکارڈ ہونے والا идентификатор эпохи۔ |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | манифест представления کے لئے полномочия подписи۔ |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | кортеж псевдонима جو `perform_submit` true ہونے پر манифест سے привязка ہوتا ہے۔ |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Пакет проверки псевдонима в кодировке Base64 (привязка псевдонима چھوڑنے کے لئے опустить کریں) |
| `DOCS_ANALYTICS_*` | Использование конечных точек аналитики/зондов и рабочих процессов Повторное использование ہوتے ہیں۔ |

Действия пользовательского интерфейса и триггер рабочего процесса:

1. `alias_label` или `docs.sora.link`, опционально `proposal_alias`,
   Дополнительное переопределение `release_tag`.
2. Артефакты بنانے کے لئے `perform_submit` کو unchecked چھوڑیں (Torii کو touch کیے بغیر)
   یا اسے включить کریں تاکہ настроить псевдоним پر براہ راست опубликовать ہو جائے۔

`docs/source/sorafs_ci_templates.md` в репозитории, который может быть использован для хранения данных
общие помощники CI

## Контрольный список

- [ ] `npm run build`, `npm run test:*`, или `npm run check:links`.
- [ ] `build/checksums.sha256` اور `build/release.json` артефакты میں محفوظ ہیں۔
- [ ] CAR, план, манифест, сводка `artifacts/` کے تحت بنے ہیں۔
- [ ] Пакет Sigstore + отдельная подпись
- [ ] `portal.manifest.submit.summary.json` или `portal.manifest.submit.response.json`
      материалы کے وقت محفوظ ہوئے۔
- [ ] `portal.pin.report.json` (اور اختیاری `portal.pin.proposal.json`)
      CAR/манифестные артефакты کے ساتھ آرکائیو ہوئے۔
- [ ] `proof verify` اور `manifest verify-signature` لاگز آرکائیو ہوئے۔
- [ ] Grafana панели мониторинга + пробники Try-It
- [ ] примечания к откату (предыдущий идентификатор манифеста + дайджест псевдонима)