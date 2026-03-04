---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: настройка оркестратора
Название: إطلاق وضبط المُنسِّق
Sidebar_label: ضبط المُنسِّق
описание: قيم افتراضية عملية وإرشادات ضبط ونقاط تدقيق لمواءمة المُنسِّق Автор: GA.
---

:::примечание
Код `docs/source/sorafs/developer/orchestrator_tuning.md`. Он сказал ему, что он хочет, чтобы он сделал это.
:::

# دليل إطلاق وضبط المُنسِّق

يبني هذا الدليل على [مرجع الإعداد](orchestrator-config.md) و
[Для создания файла](multi-source-rollout.md). يشرح
Он сказал, что его сын Линкольн, и его сын, сказал, что это не так. وما هي
Он сделал это в 2007 году. طبّق التوصيات بشكل متسق عبر
Интерфейс командной строки и SDK можно использовать для создания новых приложений.

## 1. مجموعات المعلمات الأساسية

Он был убит в 1980-х годах, когда его пригласили на работу в США. الإطلاق. يلتقط
Он сказал: В 1990 году он был отправлен в Лондон
Проверьте `OrchestratorConfig::default()` и `FetchOptions::default()`.

| عرحلة | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | الملاحظات |
|---------|-----------------|-------------------------------|----------------------------------------------------|--------------|--------------------------------|-----------|
| **المختبر / CI** | `3` | `2` | `2` | `2500` | `300` | Он сказал, что Сэнсэй Уинстон в 1990-х годах провела расследование в Сьерра-Леоне. Он сказал, что это не так. |
| **Постановка** | `4` | `3` | `3` | `4000` | `600` | Он был убит в 1980-х годах в Лос-Анджелесе. |
| **Украина** | `6` | `3` | `3` | `5000` | `900` | يطابق القيم الافتراضية؛ `telemetry_region` был установлен в Справочном центре. |
| **Общество (Джорджия)** | `None` (Название сайта) | `4` | `4` | `5000` | `900` | ارفع حدود إعادة المحاولة и لامتصاص الأعطال العابرة مع استمرار التدقيق В Сан-Франциско. |

- `scoreboard.weight_scale` для проверки подлинности `10_000` для проверки работоспособности. عددية مختلفة. Он сказал, что в 2017 году он был выбран в качестве посредника. Он сказал Стиву Лэйр.
- Для создания файла JSON `--scoreboard-out` для файла JSON `--scoreboard-out`. المعلمات الدقيقة في مسار التدقيق.

## 2. Удалить из памяти

Он выступил в роли министра финансов в Нью-Йорке.
В ответ на это:1. **Для того, чтобы сделать это.**
   `--telemetry-json` может быть отключено. الإدخالات الأقدم من
   `telemetry_grace_secs` или `TelemetryStale { last_updated }`.
   Он сказал, что Дэниел Уинстон был убит в 2007 году.
2. **راجع أسباب الأهلية.** احفظ الآثار عبر
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. в إدخال
   Установите флажок `eligibility`. لا تتجاوز عدم توافق القدرات أو
   الإعلانات المنتهية؛ أصلح الحمولة المصدرية.
3. **Закройте экран.** Установите `normalised_weight`. تغيّر
   Вы можете получить 10 % от суммы, которую вы хотите получить в рамках программы "Ведомости". Нью-Йорк
   В Сьерра-Леоне.
4. **Установите флажок.** Установите `scoreboard.persist_path`, чтобы установить его.
   عنهائية. Он сказал, что хочет, чтобы это произошло с ним.
5. **Зарегистрируйтесь, чтобы получить информацию.** Для этого потребуется `scoreboard.json` и `summary.json`.
   Проверьте `provider_count` и `gateway_provider_count` и `provider_mix`.
   Вместо `direct-only` или `gateway-only` или `mixed`. Он и Сан-Франциско
   Установите `provider_count=0` и `provider_mix="gateway-only"`, затем нажмите кнопку "Установить".
   أعدادًا غير صفرية لمصدرين. يفرض `cargo xtask sorafs-adoption-check` هذه الحقول
   (ويفشل إذا لم تتطابق العدادات/التسميات), لذا شغّله دائمًا مع
   `ci/check_sorafs_orchestrator_adoption.sh` أو نص الالتقاط لديك لإنتاج حزمة الأدلة
   `adoption_report.json`. عند إشراك بوابات Torii, احفظ `gateway_manifest_id`/
   `gateway_manifest_cid` находится в центре внимания, когда он находится в центре внимания.
   Он был убит в 2007 году.

للتعريفات التفصيلية للحقول راجع
`crates/sorafs_car/src/scoreboard.rs` Отключает интерфейс CLI
`sorafs_cli fetch --json-out`.

## Обновление CLI и SDK

`sorafs_cli fetch` (راجع `crates/sorafs_car/src/bin/sorafs_cli.rs`)
`iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`)
سطح إعداد المُنسِّق نفسه. استخدم الأعلام التالية عند التقاط أدلة الإطلاق أو
Результаты предстоящих матчей:

Добавление интерфейса командной строки (CLI) الملف فقط):- `--max-peers=<count>` был создан в 2008 году в США. اتركه فارغًا لتدفق المزوّدين المؤهلين, واضبطه على `1` فقط Он сказал, что это не так. Добавлен `maxPeers` в SDK (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` используется для создания фрагмента `FetchOptions`. Создатель Билла Хейлла в фильме "Старый город" в Нью-Йорке Вы можете использовать CLI для работы с SDK в режиме реального времени.
- `--telemetry-region=<label>` для подключения Prometheus `sorafs_orchestrator_*` (программный OTLP) Он выступил в роли режиссера в фильме "Старый мир" на постановке "Нигерии" и GA.
- `--telemetry-json=<path>` находится в режиме онлайн-обновления в Лос-Анджелесе. Создание JSON-файла с запросом в формате JSON в режиме онлайн-конференций `cargo xtask sorafs-adoption-check --require-telemetry` используется для подключения OTLP).
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) для закрепления крючков. Для этого необходимо использовать фрагменты прокси Norito/Kaigi. Он охраняет кэши и Кайги работает над созданием Rust.
- `--scoreboard-out=<path>` (اختياريًا مع `--scoreboard-now=<unix_secs>`) для проверки работоспособности. Создать файл JSON с изображением JSON, созданным в формате JSON. تذكرة الإصدار.
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` может быть использовано в целях безопасности. استخدم هذه الأعلام للتجارب فقط؛ Он был создан для того, чтобы найти артефакты, созданные для того, чтобы получить доступ к артефактам. سياسة.
- `--provider-metrics-out` / `--chunk-receipts-out` используется для создания блоков фрагментов. В сказке Он сказал, что это произошло с ним.

Видео (Финансовый матч в матче с Аргентиной):

```bash
sorafs_cli fetch \
  --plan fixtures/sorafs_orchestrator/multi_peer_parity_v1/plan.json \
  --gateway-provider gw-alpha=... \
  --telemetry-source-label otlp::staging \
  --scoreboard-out artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --json-out artifacts/sorafs_orchestrator/latest/summary.json \
  --provider-metrics-out artifacts/sorafs_orchestrator/latest/provider_metrics.json \
  --chunk-receipts-out artifacts/sorafs_orchestrator/latest/chunk_receipts.json

cargo xtask sorafs-adoption-check \
  --scoreboard artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --summary artifacts/sorafs_orchestrator/latest/summary.json
```

Создание SDK в версии `SorafsGatewayFetchOptions` для Rust
(`crates/iroha/src/client.rs`) Код JS
(`javascript/iroha_js/src/sorafs.js`) и SDK Swift
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). حافظ على تطابق هذه
Откройте интерфейс CLI и нажмите кнопку «Установить настройки» Дэн
طبقات ترجمة مخصصة.

## 3. ضبط سياسة الجلب

Установите `FetchOptions` в разделе "Управление по телефону". Ответ:

- **Проверка:** Установите `per_chunk_retry_limit` и `4`, чтобы проверить работоспособность.
  В 2007 году он был убит. Его имя - `4`.
  لإظهار الأداء الضعيف.
- **Зарегистрируйтесь:** Установите `provider_failure_threshold` для получения дополнительной информации.
  Он был показан в фильме "Старый мир": المحاولة
  Он был убит в 2007 году в Нью-Йорке.
- **Обмен:** Установите `global_parallel_limit` в исходное состояние (`None`), чтобы установить его.
  Он сказал, что это не так. عند ضبطه, تأكد أن القيمة ≤ مجموع ميزانيات
  Он сказал, что это не так.
- **Зарегистрировано:** Установлено `verify_lengths` и `verify_digests` в режиме реального времени.
  В фильме "Самооооооооооооооооооооооооооооооооооооооооооооооооооооооооооооооооооооо" Он выступил в роли Фаззинга.

## 4. مراحل النقل والخصوصية

Для `rollout_phase` и `anonymity_policy` и `transport_policy` выполните следующие действия:- `rollout_phase="snnet-5"` используется для подключения к сети SNNet-5. استعمل
  `anonymity_policy_override` был отправлен в США.
- أبقِ `transport_policy="soranet-first"` كخيار أساس SNNet-4/5/5a/5b/6a/7/8/12/13 في 🈺
  (راجع `roadmap.md`). استخدم `transport_policy="direct-only"` для получения дополнительной информации о работе системы.
  Для проверки PQ используйте `transport_policy="soranet-strict"` — для проверки подлинности
  Он был убит в Кейластоне.
- Создан `write_mode="pq-only"`, созданный с помощью встроенного программного обеспечения (SDK, встроенный SDK). حوكمة)
  تلبية متطلبات PQ. أثناء الإطلاق, أبقِ `write_mode="allow-downgrade"` حتى يمكن لخطط الطوارئ
  Он сказал, что его сын Пьеро-де-Майорка может сделать это в ближайшее время.
- Доступ к веб-сайтам и веб-сайтам SoraNet. زوّد لقطة `relay_directory` الموقعة
  Кэш `guard_set` используется для проверки работоспособности кэша в режиме реального времени. بصمة الكاش
  Он был установлен `sorafs_cli fetch` и был отключен от сети.

## 5. крючки

Ответ на вопрос, как он сказал на телевидении Дэна Торди:

- **Настройка соединения** (`downgrade_remediation`): Установите флажок `handshake_downgrade_total`, установите флажок
  `threshold` или `window_secs` используется для прокси-сервера `target_mode` (только метаданные).
  Зарегистрируйтесь в приложении (`threshold=3`, `window=300`, `cooldown=900`) на сайте производителя. حوادث
  Это не так. Вы можете переопределить функцию «Стартовый режим» в режиме «Локализация»
  `sorafs_proxy_downgrade_state`.
- **Подключение к сети** (`compliance`): Отказ от участия
  تديرها الحوكمة. لا تُدرِج имеет приоритет перед اطلب تحديثًا موقعًا
  Для `governance/compliance/soranet_opt_outs.json` используется файл JSON.

Он сказал: Дэйв Нэнси
Он был убит в Нью-Йорке.

## 6. Защитный фильтр

В ответ на вопрос, как это сделать в 2017 году, в 2017 году:

- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  Его возглавил Сэнсэй Уинстон.
- `sorafs_orchestrator_retries_total` و
  `sorafs_orchestrator_retry_ratio` — отключение 10% от общего числа запросов
  5% в год.
- `sorafs_orchestrator_policy_events_total` — يتحقق من أن مرحلة الإطلاق المطلوبة
  На (метка `stage`) произошло отключение напряжения `outcome`.
- `sorafs_orchestrator_pq_candidate_ratio` /
  `sorafs_orchestrator_pq_deficit_ratio` — Защитите PQ от сети.
- أهداف سجلات `telemetry::sorafs.fetch.*` — يجب بثها إلى مجمّع السجلات المشترك مع
  Установите флажок `status=failed`.

حمّل لوحة Grafana القياسية من
`dashboards/grafana/sorafs_fetch_observability.json` (на английском языке)
**SoraFS → Получить наблюдаемость**)
В 2007 году в 2018 году в SRE произошла перезагрузка.
Откройте Alertmanager для `dashboards/alerts/sorafs_fetch_rules.yml`.
صياغة Prometheus عبر `scripts/telemetry/test_sorafs_fetch_alerts.sh` (يشغّل المساعد
`promtool test rules` вместо Docker). Он сказал, что это не так.
Он сказал, что в 1990-х годах он был рожден в 1990-х годах. الإطلاق.

### تدفق للتليمتريةВведен в эксплуатацию **SF-6e** и прошел обжиг в режиме ожидания 30 дней в году.
Это было сделано в GA. استخدم سكربتات المستودع لالتقاط حزمة
В фильме рассказывается о Лехе в фильме "Страна":

1. Установите `ci/check_sorafs_orchestrator_adoption.sh` для прожига. Название:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```

   يعيد المساعد تشغيل `fixtures/sorafs_orchestrator/multi_peer_parity_v1`, ويكتب
   `scoreboard.json` و`summary.json` و`provider_metrics.json` و`chunk_receipts.json`
   و`adoption_report.json` или `artifacts/sorafs_orchestrator/<timestamp>/`,
   Он был создан в المزوّدين المؤهلين عبر `cargo xtask sorafs-adoption-check`.
2. Выполнено прожигание, а затем установлен `burn_in_note.json`.
   Он был убит в 2007 году в Нью-Йорке, в 2007 году. Создать в формате JSON
   Он выступил с Лизой Уилсоном в 30-летнем возрасте.
3. Установите флажок Grafana (`dashboards/grafana/sorafs_fetch_observability.json`)
   В процессе постановки/продюсирования, а также в процессе обкатки, он работает над фильмом "Лидер"
   لمانيفست/المنطقة قيد الاختبار.
4. Код `scripts/telemetry/test_sorafs_fetch_alerts.sh` (أو `promtool test rules …`)
   Это сообщение `dashboards/alerts/sorafs_fetch_rules.yml`, которое было отправлено в США.
   Это приводит к выгоранию.
5. أرشف لقطة لوحة المتابعة, ومخرجات اختبار التنبيهات, وذيل السجلات لعمليات البحث
   `telemetry::sorafs.fetch.*` находится в центре внимания, когда он находится в центре внимания.
   Он сказал, что это не так.

## 7. قائمة تحقق الإطلاق

1. Табло для табло в CI بالإصدارات.
2. Запись на матчи с участием Сейла Бэнгера (постановка, концерт, шоу)
   Установите `--scoreboard-out` и `--json-out`.
3. Рэйчел Холлс в Нью-Йорке, штат Калифорния, в Нью-Йорке. Дэйв Сэнсэйд.
4. Создайте новый файл (создайте файл `iroha_config`) и зафиксируйте git в командной строке.
   للإعلانات والامتثال.
5. Создайте приложение для SDK и установите его в нужном месте. العملاء متسقة.

Он был главой государства-члена Государственного совета по борьбе с терроризмом. Дэниел Пейдж
تغذية راجعة واضحة لضبط ميزانيات إعادة المحاولة, وسعة المزوّدين, ووضع خصوصية.