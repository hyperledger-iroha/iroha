---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

## نظرة عامة

Для подключения к **DOCS-7** (номер SoraFS) и **DOCS-8** (контактный разъем) (для CI/CD) в режиме онлайн. Для сборки/очистки выполните SoraFS, установите флажок Sigstore, проверьте В 2007 году он был назначен президентом США Джоном Стоуном и его коллегой по работе. Если вы хотите, чтобы это произошло, вы можете сделать это.

Он был создан в Лос-Анджелесе `sorafs_cli` (название `--features cli`), а также Конечная точка Torii находится в контактном реестре, а OIDC находится рядом с Sigstore. Установите флажок (`IROHA_PRIVATE_KEY`, `SIGSTORE_ID_TOKEN`, Torii) в CI; Компания «Лордовская фабрика» экспортирует свою оболочку.

## المتطلبات المسبقة

- Узел 18.18 находится рядом с `npm` и `pnpm`.
- `sorafs_cli` вместо `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- عنوان Torii на `/v1/sorafs/*` в разделе "Программа/выбор" Будьте готовы к исчезновению.
- مُصدِر OIDC (GitHub Actions, GitLab, идентификатор рабочей нагрузки, код) لاصدار `SIGSTORE_ID_TOKEN`.
— Добавлено: `examples/sorafs_cli_quickstart.sh` для настройки рабочих процессов и `docs/source/sorafs_ci_templates.md` для рабочих процессов на GitHub/GitLab.
- Проверка OAuth Попробуйте попробовать (`DOCS_OAUTH_*`)
  [Контрольный список по усилению безопасности](./security-hardening.md) при первой сборке. Вы можете получить информацию о TTL/опросе в режиме TTL/опроса. النوافذ المفروضة؛ استخدم `DOCS_OAUTH_ALLOW_INSECURE=1` для проверки подлинности. ارفق ادلة اختبار الاختراق مع تذكرة الاصدار.

## الخطوة 0 - التقاط حزمة وكيل Попробуйте

Получите доступ к Netlify на сайте https://www.netlify.com Попробуйте это прямо сейчас OpenAPI Добавлено в список:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

Для `scripts/tryit-proxy-release.mjs` используется прокси/зонд/откат, а для OpenAPI используется `release.json`. Код `checksums.sha256`. Вы можете получить доступ к шлюзу Netlify/SoraFS с возможностью подключения к Интернету. Для этого необходимо установить Torii. Для получения токенов на предъявителя необходимо использовать токены на предъявителя (`allow_client_auth`) Используйте CSP.

## الخطوة 1 - بناء وفحص lint للبوابة

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

Для `npm run build` используется `scripts/write-checksums.mjs`:

- `build/checksums.sha256` - Для SHA256 используется `sha256sum -c`.
- `build/release.json` - Добавление (`tag`, `generated_at`, `source`) АВТОМОБИЛЬ/МАШИНА.

Он был убит в автомобиле CAR, который был в центре города в Нью-Йорке. بدون اعادة بناء.

## 2 - تغليف الاصول الثابتة

Автомобильный упаковщик Docusaurus. Он был создан в соответствии с `artifacts/devportal/`.

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

Поддержка JSON, фрагментов, дайджестов и проверка доказательств `manifest build` для CI استخدامها لاحقا.

## الخطوة 2b - تغليف مرافقات OpenAPI и SBOMВ формате DOCS-7 используются файлы OpenAPI и SBOM, установленные в приложении. Создан для артефакта `Sora-Proof`/`Sora-Content-CID`. Встроенное программное обеспечение (`scripts/sorafs-pin-release.sh`) Защитное устройство OpenAPI (`static/openapi/`) для SBOM Установите `syft` в каталоге CAR `openapi.*`/`*-sbom.*` в разделе "Автомобили". `artifacts/sorafs/portal.additional_assets.json`. Для получения информации о полезной нагрузке 2–4 частей можно использовать метаданные. (от `--car-out "$OUT"/openapi.car` до `--metadata alias_label=docs.sora.link/openapi`). Создайте манифест/псевдоним Torii (название OpenAPI, SBOM SBOM OpenAPI). DNS-сервер работает в режиме реального времени, доказывая наличие артефакта.

## Запись 3 — Информационный бюллетень

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

Политика вывода контактов была введена в действие при помощи PIN-кода (например, `--pin-storage-class hot`). Создайте файл JSON для создания файла.

## Ошибка 4 - Проверка Sigstore

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

Для пакета используется фрагменты BLAKE3 с токеном OIDC для JWT. Комплект поставки والتوقيع المنفصل؛ Он играет в фильме "Старый мир" в Нью-Йорке. توقيع. Установите флажок `--identity-token-env` (или `SIGSTORE_ID_TOKEN`). البيئة) عندما يصدر مساعد OIDC خارجي التوكن.

## الخطوة 5 - Регистрация контактов PIN-кода

Установите флажок (для фрагментов) Torii. Краткое содержание книги было опубликовано в журнале "Старые книги".

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority <katakana-i105-account-id> \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

Он был назначен Кейланом (`docs-preview.sora`), а также псевдонимом Дэвида Стива. Служба контроля качества отвечает за вопросы, связанные с финансами.

Для этого выполните следующие действия: `--alias-namespace` и `--alias-name` и `--alias-proof`. Для проверки пакета (base64 и Norito байт). خزنه ضمن اسرار CI وقدمه كملف قبل استدعاء `manifest submit`. Псевдоним этого пользователя может быть изменен с помощью DNS.

## الخطوة 5b - توليد مقترح حوكمة

В фильме "Миссис Джонс" в фильме "Сора" рассказывается о Ханне и Соре. Он выступил с Дэниелом Пэнсоном в Нью-Йорке. بعد خطوات отправить/подписать:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

Для `portal.pin.proposal.json` выберите псевдоним `RegisterPinManifest`. Он был создан в 1980-х годах в Нью-Йорке в 1980-х годах. Полезная нагрузка была изменена. Он был написан в честь Torii, а также в Лос-Анджелесе в Нью-Йорке.

## الخطوة 6 - التحقق من доказательство

В разделе "Программы" есть информация:

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

- Установите `torii_sorafs_gateway_refusals_total` и `torii_sorafs_replication_sla_total{outcome="missed"}`.
- شغّل `npm run probe:portal` для запуска Try-It в режиме онлайн.
- اجمع ادلة المراقبة المذكورة في
  [Публикация и мониторинг](./publishing-monitoring.md) Для загрузки файлов DOCS-3c. Для `bindings` (например, OpenAPI, SBOM للبوابة, SBOM لـ OpenAPI) Установите `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` для установки `hostname`. Создайте файл JSON для создания файла (`portal.json`, `tryit.json`, `binding.json`, `checksums.sha256`).```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## الخطوة 6a - تخطيط شهادات البوابة

Подключение TLS SAN/challenge к серверу GAR для подключения к DNS-серверу الادلة. Создан для DG-3, созданного в Лос-Анджелесе wildcard и SAN للـ Pretty-Host На DNS-01 есть ссылка:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

В формате JSON в формате JSON (в зависимости от размера файла) لصق قيم SAN داخل اعدادات `torii.sorafs_gateway.acme` ويتأكد مراجعو GAR من الخرائط canonical/pretty دون اعادة الحساب. `--name` используется в качестве источника питания для предварительного просмотра.

## الخطوة 6b - اشتقاق خرائط المضيفين القانونية

У Сайлана Гара был псевдоним Скоула Стоуна. Если `cargo xtask soradns-hosts` используется для `--name`, выберите подстановочный знак (`<base32>.gw.sora.id`). Установите (`*.gw.sora.id`) и установите флажок (`<alias>.gw.sora.name`). احفظ الناتج ضمن artefacts الاصدار كي ممكن مراجعو DG-3 من مقارنة الخريطة مع ارسال ГАР:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

Создайте `--verify-host-patterns <file>` для просмотра файлов GAR и JSON. الخاص بالربط. В 2007 году он был убит в 1997 году, когда его отец Уилсон ГАР и `portal.gateway.binding.json` в футах. Сообщение:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

Создание файла JSON и отображение данных в DNS/открытых файлах DNS. Он был отправлен в Нью-Йорк / Нью-Йорк в Вашингтоне. Он был известен под псевдонимом Дэниел Дэвис, которого назвал ГАР Новым.

## Ошибка 7 – переключение DNS

Он был убит в 1980-х годах в Вашингтоне. بعد نجاح الارسال (ربط الاسم المستعار), يصدر المساعد
`artifacts/sorafs/portal.dns-cutover.json`, например:

- بيانات ربط الاسم المستعار (пространство имен/имя/доказательство, بصمة المانيفست, عنوان Torii, эпоха الارسال، السلطة).
- Добавление файлов (тег, псевдоним метки, код приложения/CAR, фрагменты кода, пакет Sigstore).
- مؤشرات التحقق (собственный зонд, псевдоним + конечная точка Torii).
- Дэйл Скарлетт Уинстон (в фильме «Вечеринка», «Переходный ролик», مضيف/منطقة الانتاج).
- Зарегистрируйтесь в приложении `Sora-Route-Binding` (проверьте код/CID, код). الرأس + الربط, اوامر التحقق) لضمان ان ترقية GAR и التراجع تشير لنفس الادلة.
- План маршрута المولدة (`gateway.route_plan.json`, قوالب الرؤوس, ورؤوس التراجع الاختيارية) حتى Создан для DG-3 и ворса в CI в штатном режиме.
- Выполните очистку файла (очистка, проверка подлинности, поддержка JSON, код `curl`).
- Поздравляем с Днем Рождения (с) لابقاء المسار الحتمي للتراجع.

В ходе «чистки» Уилла Уинстона в Вашингтоне:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

`portal.cache_plan.json` создан для DG-3 в Лос-Анджелесе, штат Миссури/Миссисипи (Миссисипи). авторизация) عند ارسال طلبات `PURGE`. Он был убит в 1980-х годах в Уолл-стрит.

Он был создан DG-3 в 1980-х годах. Код `cargo xtask soradns-route-plan`:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

يسجل `gateway.route_plan.json` المضيفين القانونيين/الجميلين, تذكيرات فحوص الصحة Проведите очистку ГАР, очистите воздух и очистите его. ارفقه مع ملفات GAR/binding/cutover قبل ارسال التذكرة حتى يتمكن فريق Ops من التدريب على الخطوات نفسها.`scripts/generate-dns-cutover-plan.mjs` был установлен в `sorafs-pin-release.sh`. Краткое описание:

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

Информация о том, как сделать это в календаре событий:

| المتغير | غرض |
| --- | --- |
| `DNS_CHANGE_TICKET` | Написано в журнале. |
| `DNS_CUTOVER_WINDOW` | Переключение осуществляется по стандарту ISO8601 (код `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | Наслаждайтесь + приветствие. |
| `DNS_OPS_CONTACT` | جهة الاتصال للطوارئ. |
| `DNS_CACHE_PURGE_ENDPOINT` | Проведите чистку дома. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Произведите очистку токенов (например, `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | Он был убит в Лос-Анджелесе. |

Создание JSON для DNS и поисковые запросы в локальной сети. В ходе расследования было проведено расследование о том, что Бэнтон и Сэйр CI. Откройте интерфейс командной строки `--dns-change-ticket` и `--dns-cutover-window` и `--dns-hostname` и `--dns-zone` и `--ops-contact`. و`--cache-purge-endpoint` و`--cache-purge-auth-env` و`--previous-dns-plan` в режиме CI.

## Ошибка 8 - Открыть файл зоны لمحلل (اختياري)

Для этого необходимо создать файл зоны Zonefile. В социальных сетях и фрагменте للمحلل تلقائيا. Откройте DNS и создайте учетную запись CLI; سيستدعي المساعد `scripts/sns_zonefile_skeleton.py` مباشرة توليد الوصف. Отключено A/AAAA/CNAME в GAR (BLAKE3-256 в режиме ожидания). اذا كانت المنطقة/المضيف معروفة وتم حذف `--dns-zonefile-out`, سيكتب المساعد في `artifacts/sns/zonefiles/<zone>/<hostname>.json` — `ops/soradns/static_zones.<hostname>.json` — фрагмент преобразователя.

| المتغير / الخيار | غرض |
| --- | --- |
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | Создан скелет файла зоны. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Фрагмент преобразователя (الافتراضي `ops/soradns/static_zones.<hostname>.json`). |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | TTL-отключение (время: 600 пикселей). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | Поддержка IPv4 (доступен для использования в режиме реального времени). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | Есть IPv6. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | В CNAME اختياري. |
| И18НИ00000195Х, И18НИ00000196Х | Поддержка SPKI SHA-256 (base64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Загрузите TXT-файл (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | Создан лейбл النسخة المحسوب. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | Временная метка `effective_at` (RFC3339) используется для проверки. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | Это доказательство того, что они есть в мире. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | Он был арестован CID в Нью-Йорке. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | حالة تجميد الحارس (мягкий, жесткий, оттаивание, мониторинг, экстренный режим). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | تذكرة التجميد. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | временная метка RFC3339 для оттаивания. |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | Он сказал: |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | بصمة BLAKE3-256 (шестнадцатеричный) для проверки. Используйте крепления для крепления. |

Действия на GitHub можно выполнить в разделе «Обзоры», где вы можете закрепить артефакты. Предыдущее сообщение (вместе с Дэвидом Уилсоном и его коллегой):| Секрет | غرض |
| --- | --- |
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | مضيف/منطقة الانتاج للمساعد. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | Он сказал, что это не так. |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | Поддержка IPv4/IPv6. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | В CNAME اختياري. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Используйте SPKI base64. |
| `DOCS_SORAFS_ZONEFILE_TXT` | Получите сообщение TXT. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | Дэнни Уилсон в Лос-Анджелесе. |
| `DOCS_SORAFS_GAR_DIGEST` | Он был создан BLAKE3 в 1990-х годах. |

Установите `.github/workflows/docs-portal-sorafs-pin.yml` и создайте файл `dns_change_ticket` и `dns_cutover_window` в файле /zonefile. القطع الصحيحة. Был проведен пробный прогон.

Ответ Нэнси (записной книжек от владельца SN-7):

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

Вы можете просмотреть текстовое сообщение с текстом TXT и получить доступ к следующему файлу. `effective_at` находится в центре города. Установите флажок `docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Проверка DNS-запроса

Файл зоны находится в зоне действия приложения. Для создания базы данных NS/DS необходимо установить соединение с DNS-сервером. Это было сделано в честь президента США.

- Переключение домена в apex/TLD с псевдонимом/ANAME (независимое имя) и в режиме A/AAAA с любыми трансляциями الخاصة بالبوابة.
- Установите CNAME для симпатичного хоста (`<fqdn>.gw.sora.name`).
- المضيف القانوني (`<hash>.gw.sora.id`) العام.

### قالب رؤوس البوابة

Встроенные артефакты `portal.gateway.headers.txt` и `portal.gateway.binding.json` являются артефактами, установленными на DG-3. Ответ на вопрос:

- Установите `portal.gateway.headers.txt` с помощью HTTP-клиента (задайте `Sora-Name` и `Sora-Content-CID`). و`Sora-Proof` وCSP وHSTS و`Sora-Route-Binding`) Он был отправлен в США в США.
- Приложение `portal.gateway.binding.json` было создано в 2007 году в 2017 году. Для хоста/цида используется оболочка оболочки.

Имя пользователя: `cargo xtask soradns-binding-template`, псевдоним и имя хоста. Установите флажок `sorafs-pin-release.sh`. Информация о том, как это сделать:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

Коды `--csp-template` и `--permissions-template` и `--hsts-template` можно найти в приложении. Он был установлен на `--no-*` в режиме реального времени.

Для создания файла CDN и JSON-файла для просмотра файлов CDN. Он ответил на вопрос, как это сделать.

Он был убит в 1980-х годах, когда был убит DG-3. حديثة. Для создания файла JSON:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

تفك هذه الاوامر حمولة `Sora-Proof` , который находится в بيانات `Sora-Route-Binding` تطابق CID определяет имя хоста и имя хоста. احتفظ بمخرجات الامر خارج CI.

> **Задать DNS:** Введите `portal.dns-cutover.json` в разделе `gateway_binding`, чтобы получить доступ к DNS (англ.) وcontent CID и доказательство والقالب النصي للرؤوس) **وكذلك** مقطع `route_plan` الذي يشير الى `gateway.route_plan.json` Наслаждайтесь просмотром/выбором. Он был создан для DG-3 в Лос-Анджелесе, где находится `Sora-Name/Sora-Proof/CSP`. Он был создан в 1990-х годах в Нью-Йорке.

## الخطوة 9 - تشغيل مراقبات النشرЗагрузите файл **DOCS-3c**, чтобы получить доступ к **DOCS-3c**. Попробуйте это وروابط Он сказал, что это не так. Проведенный матч в матче против 7-8 в матче против Баскетбола:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- يقوم `scripts/monitor-publishing.mjs` بتحميل ملف اعداد (راجع `docs/portal/docs/devportal/publishing-monitoring.md` للمخطط) Доступно: Запустите CSP/Permissions-Policy и попробуйте попробовать (`/metrics`). Загрузите файл Sora-Content-CID (`cargo xtask soradns-verify-binding`) для просмотра содержимого Sora-Content-CID. псевдоним/манифест.
- В ответ на запрос CI/cron/с помощью зонда CI/cron/ Это не так.
- تمرير `--json-out` с файлом JSON и файлом JSON. و`--evidence-dir` يصدر `summary.json` و`portal.json` و`tryit.json` و`binding.json` و`checksums.sha256` حتى Он был убит в Нью-Йорке в Нью-Йорке. В пакете `artifacts/sorafs/<tag>/monitoring/` есть пакет Sigstore и дескриптор переключения DNS.
- Зарегистрируйтесь в приложении Alertmanager для Grafana (`dashboards/grafana/docs_portal.json`) и проверьте Alertmanager. В Лос-Анджелесе SLO используется DOCS-3c, и он находится в Лос-Анджелесе. Создан для `docs/portal/docs/devportal/publishing-monitoring.md`.

Для подключения к HTTPS используется `http://` для подключения к `allowInsecureHttp`. الاعداد. Используйте TLS в режиме онлайн-сервиса и в режиме онлайн-трансляции.

Создан файл `npm run monitor:publishing` в Buildkite/cron, созданный с помощью Buildkite/cron. Он был убит в 1980-х годах в Нью-Йорке.

## Дополнительная информация `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` содержит информацию 2-6. В:

1. Загрузите `build/` в tar-архив.
2. `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature` и `proof verify`,
3. Добавлен `manifest submit` (привязка псевдонима بما فيه).
4. Добавьте `artifacts/sorafs/portal.pin.report.json` и `portal.pin.proposal.json` и дескриптор переключения DNS, который будет удален (`portal.gateway.binding.json`). ((((((((((((((((((((((((((((( Дэнни Сэйр CI.

Установите `PIN_ALIAS`, `PIN_ALIAS_NAMESPACE` и `PIN_ALIAS_NAME` и (справа) `PIN_ALIAS_PROOF_PATH` для проверки. Дополнительная информация `--skip-submit` Рабочий процесс на GitHub создан в формате `perform_submit`.

## Ошибка 8 — вызов OpenAPI для SBOM

Создан DOCS-7 в соответствии со стандартом OpenAPI и SBOM, установленным в системе. Информационные сообщения:

1. **Оборудование для обмена сообщениями.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   Установите `--version=<label>` для подключения к сети (например, `2025-q3`). Для этого необходимо установить `static/openapi/versions/<label>/torii.json` и использовать `versions/current` для `static/openapi/versions.json`. Он установлен на SHA-256 и находится в режиме ожидания. Он был убит в 1980-х годах в Лос-Анджелесе. البصمة/التوقيع. Он был установлен `--version` и установлен на `current` и `latest`.

   В комплект поставки входит SHA-256/BLAKE3, созданный на базе `Sora-Proof` لـ `/reference/torii-swagger`.

2. **Оборудования CycloneDX SBOM.** Для получения обновлений SBOM используется syft `docs/source/sorafs_release_pipeline_plan.md`. Ниже представлены артефакты:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **Отображение полезной нагрузки в CAR.**

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
   ```Используйте `manifest build` / `manifest sign` для создания псевдонимов артефакта (مثل). `docs-openapi.sora` и `docs-sbom.sora` для SBOM الموقعة). С помощью SoraDNS и GAR можно получить полезную нагрузку.

4. **Запуск.** Задайте полномочия для Sigstore Bundle, задайте псевдоним кортежа для проверки подлинности. Он был убит Сорой в фильме "Сора".

На фестивале OpenAPI/SBOM вам будет предложено купить билет на фестиваль. Найдите нового упаковщика.

### Создание сценария (скрипт CI/пакета)

`./ci/package_docs_portal_sorafs.sh` используется в течение 1–8 дней. В ответ на это:

- Добавление виджетов (`npm ci`, встроенные виджеты OpenAPI/Norito).
- Для автомобилей CAR используются модели OpenAPI и SBOM `sorafs_cli`.
- Установлен `sorafs_cli proof verify` (`--proof`), установлен Sigstore (`--sign`, `--sigstore-provider`). `--sigstore-audience`) اختياريا.
- Установите флажок `artifacts/devportal/sorafs/<timestamp>/` и установите `package_summary.json` для версии CI/release.
- تحديث `artifacts/devportal/sorafs/latest` в режиме онлайн.

Сообщение (протокол Sigstore + PoR):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

Уважаемый:

- `--out <dir>` - تغيير جذر الاصول (временная метка الافتراضي مجلدات).
- `--skip-build` - Установите флажок `docs/portal/build`.
- `--skip-sync-openapi` - Зарегистрируйтесь `npm run sync-openapi` на сайте crates.io `cargo xtask openapi`.
- `--skip-sbom` - عدم استدعاء `syft` عند عدم توفره (مع تذير).
- `--proof` - Проверьте `sorafs_cli proof verify` в автомобиле/манифесте.
- `--sign` - Есть `sorafs_cli manifest sign`.

Установите флажок `docs/portal/scripts/sorafs-pin-release.sh`. Портал/OpenAPI/SBOM используется для хранения метаданных в `portal.additional_assets.json`. يدعم مفاتيح `--openapi-*` و`--portal-sbom-*` و`--openapi-sbom-*` псевдоним لكل artefact وتجاوز مصدر SBOM `--openapi-sbom-source` и полезные нагрузки (`--skip-openapi`/`--skip-sbom`) и `syft`. عبر `--syft-bin`.

يعرض السكربت كل اوامر؛ انسخ السجل الى تذكرة الاصدار مع `package_summary.json` حتى يتمكن المراجعون من مقارنة дайджесты Метаданные используются в оболочке.

## Номер 9 – доступ к шлюзу + SoraDNS

При переключении канала, псевдониме الجديد и SoraDNS, а также поисковых доказательствах:

1. **Затвор датчика.** Для `ci/check_sorafs_gateway_probe.sh` используйте `cargo xtask sorafs-gateway-probe` для крепления `fixtures/sorafs_gateway/probe_demo/`. Введите имя хоста и укажите имя хоста:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   Датчик датчика `Sora-Name` и `Sora-Proof` и `Sora-Proof-Status` и `docs/source/sorafs_alias_policy.md` для проверки. Доступно для TTL и привязок GAR.

   Для облегченных выборочных проверок (например, когда только пакет переплета
   изменилось), запустите `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   Помощник проверяет захваченный пакет привязок и удобен для выпуска.
   билеты, для которых требуется только обязательное подтверждение вместо полной проверки.

2. **Сверла с сверлами.** `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario devportal-rollout -- ...` для `artifacts/sorafs_gateway_probe/<stamp>/` для `artifacts/sorafs_gateway_probe/<stamp>/`. `ops/drill-log.md`, используйте перехватчики отката в PagerDuty. Загрузите `--host docs.sora` на сайт SoraDNS.3. **Управление привязками DNS.** Проверьте наличие псевдонимов и проверьте GAR, чтобы получить доступ к ним. الاصدار. Воспользуйтесь резольвером `tools/soradns-resolver`. В формате JSON используется `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]` для сопоставления хостов, метаданных и меток телеметрии. Он был установлен на `--json-out` в GAR الموقّع.
  В GAR используется `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`, а также `--manifest-cid`, установленный на сайте. المانيفست. В программе CID и в BLAKE3 используется JSON, созданный с помощью JSON. Вы можете установить ярлыки для CSP/HSTS/Permissions-Policy и настроить параметры доступа.

4. **псевдоним пользователя **. Он был создан для `dashboards/grafana/docs_portal.json`.

## 10 - المراقبة وحزم الادلة

- **اللوحات.** صدّر `dashboards/grafana/docs_portal.json` و`dashboards/grafana/sorafs_gateway_observability.json` و`dashboards/grafana/sorafs_fetch_observability.json` для проверки работоспособности.
- **проверенные зонды.** Загрузите `artifacts/sorafs_gateway_probe/<stamp>/` в git-annex и откройте приложение.
- **حزمة الاصدار.** خزّن ملخصات CAR ومجموعات المانيفست وتواقيع Sigstore و`portal.pin.report.json` — Try-It, который можно использовать для проверки.
- **Сверла.** Для сверл используются `scripts/telemetry/run_sorafs_gateway_probe.sh` и `ops/drill-log.md` для подключения к SNNet-5.
- **روابط التذاكر.** Загрузите файл Grafana в PNG с помощью зонда تقرير. Он сказал, что у него есть раковина.

## الخطوة 11 - تمرين fetch متعدد المصادر وادلة tableboard

Введите SoraFS и выберите файл конфигурации (DOCS-7/SF-6) для DNS/шлюза. Ответ на вопрос:

1. **Загрузить `sorafs_fetch` в соответствии с требованиями.** Создать план/манифест в соответствующем разделе. Сообщение от Дэвида Дэвиса:

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

   - Реклама провайдера в разделе "Объявления от провайдера" (формат `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`) и `--provider-advert name=path`. Лидер Тэхен сказал ему, что это не так. استخدم `--allow-implicit-provider-metadata` **فقط** عند اعادة تشغيل светильники في CI.
   - Вы можете использовать кэш/псевдоним для создания кэша/псевдонима.

2. **Запустите.** Установите `scoreboard.json` и `providers.ndjson` и `fetch.json` и `chunk_receipts.ndjson` в случае необходимости. الادلة.

3. **Загрузить **SoraFS Fetch Observability** (`dashboards/grafana/sorafs_fetch_observability.json`) `torii_sorafs_fetch_duration_ms`/`_failures_total`.

4. ** Установите флажок. ** Установите `scripts/telemetry/test_sorafs_fetch_alerts.sh` и используйте promtool.

5. **Запись в CI.** Для `sorafs_fetch` и `perform_fetch_probe` в процессе постановки/производства.

## Справочная информация

1. **Обозначение:** Псевдонимы منفصلة لـ связаны с постановкой и производством. Имя пользователя `manifest submit`, имя псевдонима `--alias-namespace/--alias-name`. الانتاج.
2. **Запустите:** Установите `docs/source/grafana_sorafs_pin_registry.json` и установите флажок `docs/portal/docs/devportal/observability.md`.
3. **Общество:** Свободное сообщение с именем Свободы (псевдоним التحالي) عبر `sorafs_cli manifest submit --alias ... --retire`.

## قالب CI

В ответ на вопрос о том, как это сделать:1. Сборка + проверка (`npm ci`, `npm run build`, проверка контрольных сумм).
2. Заглушка (`car pack`) с возможностью установки.
3. Зарегистрируйте токен OIDC для токена (`manifest sign`).
4. رفع الاصول (CAR, مانيفست, комплект, план, резюме).
5. Реестр PIN-кодов:
   - Запросы на извлечение -> `docs-preview.sora`.
   - Теги / فروع محمية -> ترقية псевдоним الانتاج.
6. Проверка зондов + проверка подлинности для доказательства.

`.github/workflows/docs-portal-sorafs-pin.yml` находится в режиме ожидания. Рабочий процесс:

- بناء/اختبار البوابة،
- Установлена сборка `scripts/sorafs-pin-release.sh`.
- Пакет манифеста توقيع/تحقق на GitHub OIDC,
- Сводки CAR/манифеста/пакета/плана/доказательства и артефакты.
- (اختياريا) المانيفست وربط, псевдоним عند توفر secrets.

Секреты/переменные для проверки:

| Имя | غرض |
| --- | --- |
| `DOCS_SORAFS_TORII_URL` | Код Torii или `/v1/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | Эпоха المسجل مع الارسال. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | Нажмите на кнопку «Скрыть». |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | кортеж псевдонима المربوط عند `perform_submit` = `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | доказательство пакета للalias بترميز base64 (اختياري). |
| `DOCS_ANALYTICS_*` | Аналитика/зонд الحالية. |

Просмотр рабочего процесса Действия:

1. Установите `alias_label` (например, `docs.sora.link`), установите `proposal_alias` и `release_tag`.
2. Установите `perform_submit` в тестовом режиме в Нью-Йорке Torii (пробный прогон) в режиме реального времени. Псевдоним المباشر على.

يبقى `docs/source/sorafs_ci_templates.md`, созданный в Лос-Анджелесе, рабочий процесс. للاصدارات اليومية.

## قائمة التحقق

- [ ] نجاح `npm run build` و`npm run test:*` و`npm run check:links`.
- [ ] حفظ `build/checksums.sha256` و`build/release.json` ضمن الاصول.
- [] Создайте CAR, план, манифест и сводку `artifacts/`.
- [ ] Пакет Sigstore доступен для скачивания.
- [ ] حفظ `portal.manifest.submit.summary.json` و`portal.manifest.submit.response.json` عند الارسال.
- [ ] Установите `portal.pin.report.json` (و`portal.pin.proposal.json` позже).
- [ ] حفظ سجلات `proof verify` и `manifest verify-signature`.
- [ ] تحديث لوحات Grafana Новые тесты Try-It.
- [ ] ارفاق ملاحظات التراجع (ID المانيفست السابق + псевдоним дайджеста) مع تذكرة الاصدار.