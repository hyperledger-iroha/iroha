---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/try-it.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# بيئة Try It التجريبية

В фильме "Попробуй" Дэн Тэхен в фильме "Попробуй" в фильме "Попробуй" Код Torii был установлен. Он был выбран в качестве президента США в 2007 году. Он был создан в рамках CORS в рамках программы по борьбе с коррупцией.

## المتطلبات المسبقة

- Версия Node.js 18.18 (в версии 18.18)
- Выполняется в промежуточном состоянии Torii.
- токен на предъявителя يمكنه استدعاء مسارات Torii

Он был отправлен в Нью-Йорк в Нью-Йорке. В сообщении говорится:

| المتغير | غرض | الافتراضي |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | عنوان Torii الاساسي الذي يعيد الوكيل توجيه الطلبات اليه | **Обязательно** |
| `TRYIT_PROXY_LISTEN` | عنوان الاستماع للتطوير المحلي (صيغة `host:port` и `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | قائمة مفصولة بفواصل للمصادر المسموح لها باستدعاء الوكيل | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | معرف يوضع في `X-TryIt-Client` в восходящем направлении | `docs-portal` |
| `TRYIT_PROXY_BEARER` | токен на предъявителя افتراضي يعاد توجيهه الى Torii | _пустой_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | السماح للمستخدمين بتقديم token خاص بهم عبر `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | Размер файла (байты) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | مهلة вверх по течению بالميلي ثانية | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Вы можете получить доступ к IP-адресам | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | نافذة انزلاقية للحد من المعدل (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Сохранение метрики بأسلوب Prometheus (`host:port` и `[ipv6]:port`) | _пусто (отключено)_ |
| `TRYIT_PROXY_METRICS_PATH` | Параметры HTTP-метрики | `/metrics` |

Создается файл `GET /healthz`, который позволяет использовать JSON для хранения токенов-носителей в режиме реального времени.

На `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` написано в журнале Swagger и RapiDoc. Вы можете использовать жетоны на предъявителя в любой ситуации. В 2007 году он был выбран в качестве посредника в работе над фильмом "Старый мир" в Нью-Йорке. استخخدم token الافتراضي او تجاوزا لكل طلب. اضبط `TRYIT_PROXY_CLIENT_ID` بالوسم الذي تريد ارساله كـ `X-TryIt-Client`
(Название `docs-portal`). В 2009 году он был отправлен в `X-TryIt-Client`, чтобы узнать больше о том, как это сделать. Постановщиком выступил Дэниел Бернс в постановке фильма "Тренер" в исполнении Дэна Робертса.

## تشغيل الوكيل محليا

Сообщение о том, что он сделал в 2017 году:

```bash
cd docs/portal
npm install
```

Обратитесь к номеру Torii по запросу:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

Для получения дополнительной информации обратитесь к `/proxy/*` на Torii. المحدد.

قبل ربط المنفذ يتحقق السكربت من ان
`static/openapi/torii.json` يطابق الـ дайджест المسجل في
`static/openapi/manifest.json`. Он сказал, что его сын Уилсон Хейлз
`npm run sync-openapi -- --latest`. صدّر
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` فقط لحالات الطوارئ؛ Сэйл Уилсон Пьер и Уилсон Трэйси в фильме
Он сказал, что это не так.

## ربط عناصر البوابة

عند بناء البوابة او تشغيلها, اضبط عنوان URL-адрес ссылки на сайт Людмила:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Чтобы получить информацию о `docusaurus.config.js`:- **Swagger UI** - Добавлен в `/reference/torii-swagger`; يسبق تفويض مخطط предъявитель عند وجود token,
  Он был установлен `X-TryIt-Client`, `X-TryIt-Auth`, а также установлен на устройстве.
  Это сообщение было создано `TRYIT_PROXY_PUBLIC_URL`.
- **RapiDoc** - добавлено в `/reference/torii-rapidoc`; يعكس حقل токен,
  Его заголовки были показаны Джоном Сваггером и его коллегой Сейном Сваггером.
- **Попробуйте консоль** - Нажмите на ссылку API. تتيح ارسال طلبات مخصصة،
  Используйте заголовки, чтобы получить доступ к ним.

يعرض كلا اللوحتين **محدد snapshots** الذي يقرأ
`docs/portal/static/openapi/versions.json`. املأ هذا الفهرس باستخدام
`npm run sync-openapi -- --version=<label> --mirror=current --latest` حتى يتمكن المراجعون من
Получение дайджеста SHA-256 для получения снимка SHA-256
Это будет проявлением того, что вы хотите сделать это в ближайшее время.

Получите токен в случае, если вы хотите получить доступ к токену الوكيل لا يخزن ولا يسجل الـ token المقدم.

## Проверка OAuth

لتجنب توزيع رموز Torii طويلة العمر على المراجعين, اربط وحدة Попробуйте بخادم OAuth لديك. Сан-Диего
Для этого необходимо указать код устройства, идентификатор носителя жетоны
Он был убит в Нью-Йорке.

| المتغير | غرض | الافتراضي |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | Открыть доступ к OAuth (`/oauth/device/code`) | _пусто (отключено)_ |
| `DOCS_OAUTH_TOKEN_URL` | Код токена تقبل `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _пустой_ |
| `DOCS_OAUTH_CLIENT_ID` | Вход в систему OAuth для проверки подлинности | _пустой_ |
| `DOCS_OAUTH_SCOPE` | نطاقات مفصولة بمسافة مطلوبة اثناء تسجيل الدخول | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Audience API Доступ к токену | _пустой_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Результаты опроса اثناء انتظار الموافقة (ms) | `5000` (время < 5000 мс) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Код устройства مدة صلاحية الاحتياطية (ثوان) | `600` (от 300 до 900 с) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Получить токен доступа الاحتياطية (ثوان) | `900` (от 300 до 900 с) |
| `DOCS_OAUTH_ALLOW_INSECURE` | `1` Доступ к протоколу OAuth | _закат_ |

Сообщение:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

Установите `npm run start` и `npm run build`, чтобы получить информацию о том, как это сделать.
`docusaurus.config.js`. اثناء المعاينة المحلية تعرض بطاقة Попробуйте زر
«Войти с кодом устройства». Вы можете получить доступ к протоколу OAuth; عند نجاح
Поток устройства

- Доступ к токену на предъявителя для использования в режиме реального времени. Попробуйте,
- Для заголовков `X-TryIt-Client` и `X-TryIt-Auth`.
- عرض العمر المتبقي، و
- حذف الـ token تلقائيا عند انتهاء الصلاحية.

يظل ادخال Bearer اليدوي متاحا؛ Для доступа к OAuth необходимо получить доступ к токену
بانفسهم, او صدّر `DOCS_OAUTH_ALLOW_INSECURE=1` لمعاينات محلية معزولة يكون فيها الوصول المجهول مقبولا.
Выполните сборку OAuth в соответствии с требованиями протокола DOCS-1b для проверки подлинности.

Название: راجع [Контрольный список повышения безопасности и пен-теста](./security-hardening.md)
قبل فتح البوابة خارج المختبر؛ Для проверки соответствия CSP/Trusted Types и проверки на проникновение.
Приложение DOCS-1b.

## Norito-RPCЗагрузите Norito-RPC для прокси-сервера и сантехники OAuth для JSON; в تضبط
`Content-Type: application/x-norito` Отключает полезную нагрузку Norito Загрузка данных в NRPC
(`docs/source/torii/nrpc_spec.md`). يوفر المستودع полезные нагрузки
`fixtures/norito_rpc/` добавлен в программу SDK для проверки подлинности.
Информационный центр CI.

### ارسال payload Norito в разделе Try It

1. Установите приспособление типа `fixtures/norito_rpc/transfer_asset.norito`. هذه الملفات هي
   Сообщение Norito خام؛ **لا** Вы можете использовать base64.
2. В Swagger в RapiDoc используется конечная точка NRPC (
   `POST /v2/pipeline/submit`) وغيّر محدد **Content-Type** الى
   `application/x-norito`.
3. Создайте файл **binary** (от «File» в Swagger и «Binary/File» в RapiDoc)
   Код: `.norito`. Он выступил с речью в газете "Борнес-Турнир".
4. Свободный день. اذا اعاد Torii `X-Iroha-Error-Code: schema_mismatch`, пожалуйста, проверьте
   Конечная точка отображает полезные данные и хэш схемы.
   `fixtures/norito_rpc/schema_hashes.json` установлен на сборку Torii.

Создаётся впечатление, что в режиме реального времени происходит перегрузка полезной нагрузки в режиме реального времени. жетоны
Там же Torii. اضافة `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` الى سير عملك
Создает пакетный файл для просмотра в формате NRPC-4 (log + файл JSON), а также для просмотра в формате JSON.
Посмотреть скриншоты Попробуйте попробовать.

### Доступ к CLI (curl)

يمكن اعادة تشغيل نفس светильники خارج البوابة عبر `curl`, وهو مفيد عند التحقق Во время праздника
تصحيح ردود шлюз:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
```

Приспособление установлено на `transaction_fixtures.manifest.json` в соответствии с полезной нагрузкой خاص بك عبر
`cargo xtask norito-rpc-fixtures`. عندما يكوون Torii и Canary يمكنك توجيه `curl` الى
пробный прокси (`https://docs.sora.example/proxy/v2/pipeline/submit`)
التي تخدمها عناصر البوابة.

## المراقبة والعمليات

Для этого необходимо указать метод, путь и источник, восходящие потоки данных.
(`override` или `default` или `client`). Доступ к токенам اطلاقا؛ Добавление заголовков носителя
В `X-TryIt-Auth` используется стандартный стандартный вывод, который используется в стандартном интерфейсе. تسرب الاسرار.

### فحوصات الصحة والتنبيه

Исследование, проведенное в 2008 году и в Брэнсоне Стоуном:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```

Видео по теме:

- `TRYIT_PROXY_SAMPLE_PATH` - مسار Torii اختياري (بدون `/proxy`) لاختباره.
- `TRYIT_PROXY_SAMPLE_METHOD` - Внутренний `GET`; Установите `POST`.
- `TRYIT_PROXY_PROBE_TOKEN` - токен на предъявителя для обмена данными.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - Отмена сигнала в течение 5 с.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - Установите Prometheus на `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - `key=value` для проверки подлинности `job=tryit-proxy` و `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - Установите метрики (для `http://localhost:9798/metrics`) `TRYIT_PROXY_METRICS_LISTEN`.

ادمج النتائج в сборщике текстовых файлов, عبر توجيه зонд الى مسار قابل للكتابة
(Для `/var/lib/node_exporter/textfile_collector/tryit.prom`) Обратите внимание:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

Он был отправлен в космос, чтобы получить полезную нагрузку Кейла.عند ضبط `TRYIT_PROXY_METRICS_LISTEN`, قم بتعيين
`TRYIT_PROXY_PROBE_METRICS_URL` позволяет получить метрики для проверки зонда и очистки данных.
(Входной сигнал отключен от брандмауэра). اعداد Production شائع هو
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

Владелец полиции исследует ситуацию с Бэнкс-Сити. Код Prometheus в приложении :

```yaml
groups:
  - name: tryit-proxy
    rules:
      - alert: TryItProxyUnhealthy
        expr: probe_success{job="tryit-proxy"} == 0
        for: 2m
        labels:
          severity: page
        annotations:
          summary: Try It proxy is failing health checks
          description: |
            The try-it proxy at {{ $labels.instance }} is not responding to probe requests.
```

### Получение метрик

`TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (для хоста/порта)
Для получения метрики используйте Prometheus. المسار الافتراضي هو `/metrics` ويمكن تغييره عبر
`TRYIT_PROXY_METRICS_PATH=/custom`. Чтобы очистить метод, используйте метод Scrape.
ограничение скорости и тайм-ауты в восходящем направлении.

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Установите Prometheus/OTLP для получения метрик.
`dashboards/grafana/docs_portal.json` используется для SRE в случае задержки задержки.
Дэн Тэйлл Уилсон. ينشر الوكيل تلقائيا `tryit_proxy_start_timestamp_ms` لمساعدة الشغلين
على اكتشاف اعادة التشغيل.

### اتمتة التراجع

Установите флажок Torii. يحفظ السكربت الاعداد السابق في
`.env.tryit-proxy.bak` حتى в التراجع امرا واحدا.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Для этого необходимо установить env عبر `--env` и `TRYIT_PROXY_ENV`, чтобы получить доступ к соответствующему файлу. В Миссисипи.