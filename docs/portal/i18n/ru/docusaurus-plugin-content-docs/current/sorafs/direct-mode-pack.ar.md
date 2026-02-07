---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/direct-mode-pack.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: пакет прямого режима
title: حزمة الرجوع للوضع المباشر في SoraFS ‏(SNNet-5a)
Sidebar_label: حزمة الوضع المباشر
описание: الإعدادات المطلوبة وفحوصات الامتثال وخطوات الإطلاق عند تشغيل SoraFS في Torii/QUIC используется для подключения к SNNet-5a.
---

:::примечание
Был установлен `docs/source/sorafs/direct_mode_pack.md`. Он был создан в честь Сфинкса.
:::

Зарегистрируйтесь в SoraNet и выберите SoraFS, чтобы получить доступ к **SNNet-5a**. В фильме рассказывается о Джоне Уилсоне и его сыне Сэнсэй Хилле. طرح الخصوصية. Выполните настройку параметров в CLI/SDK, а затем выполните следующие действия. Установите флажок SoraFS для Torii/QUIC. Это не так.

В фильме "Страна" постановка фильма "Станок SNNet-5" и "SNNet-9" بوابات الجاهزية. احتفظ بالمواد أدناه مع حزمة نشر SoraFS المعتادة حتى يتمكن المشغلون من Он был убит Джоном Уилсоном в Нью-Йорке.

## 1. Поддержка CLI и SDK

- `sorafs_cli fetch --transport-policy=direct-only ...` используется для подключения к Torii/QUIC. Откройте CLI для `direct-only`, чтобы загрузить файл.
- В SDK установлена ​​`OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` для создания файла "Скрыть". Вы можете использовать `iroha::ClientOptions` и `iroha_android`, используя перечисление.
- Доступ к шлюзу (`sorafs_fetch` на Python) Доступ только для прямого доступа к Norito JSON Он сказал, что это не так.

В фильме "Капитанский университет" в Лос-Анджелесе, в Нью-Йорке, в Нью-Йорке. `iroha_config` был установлен на сайте.

## 2. Открыть шлюз

استخدم Norito JSON для создания файла. Для `docs/examples/sorafs_direct_mode_policy.json` код:

- `transport_policy: "direct_only"` — используется для подключения к сети SoraNet.
- `max_providers: 2` — используется для подключения к Torii/QUIC. Это произошло в Сьерра-Леоне.
- `telemetry_region: "regulated-eu"` — يوسم المقاييس المصدرة بحيث تميز لوحات المتابعة والتدقيق تشغيلات الرجوع.
- Добавление нового приложения (`retry_budget: 2`, `provider_failure_threshold: 3`) سيئة الضبط.

Создайте JSON-файл `sorafs_cli fetch --config` (открыто) и используйте SDK (`config_from_json`) для получения дополнительной информации. المشغلين. Установите табло (`persist_path`).

Откройте шлюз для `docs/examples/sorafs_gateway_direct_mode.toml`. Установите флажок `iroha app sorafs gateway direct-mode enable`, укажите конверт/вход, ограничение скорости. Установите флажок `direct_mode`, чтобы получить дополнительную информацию о состоянии системы. وملخصات манифест. Он сказал, что хочет, чтобы он сказал, что это не так.

## 3. مجموعة اختبارات الامتثال

Чтобы получить доступ к интерфейсу командной строки, выполните следующие действия:- `direct_only_policy_rejects_soranet_only_providers` يضمن أن `TransportPolicy::DirectOnly` на сайте SoraNet. 【crates/sorafs_orchestrator/src/lib.rs:7238】
- `direct_only_policy_prefers_direct_transports_when_available` используется для Torii/QUIC для подключения к SoraNet. الجلسة.【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` и `docs/examples/sorafs_direct_mode_policy.json` لضمان بقاء الوثائق متوافقة مع أدوات المساعدة.【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_policy.json:1】
- `fetch_command_respects_direct_transports` يختبر `sorafs_cli fetch --transport-policy=direct-only` أمام بوابة Torii وهمية, موفرا اختبار Smoke لبيئات Загрузите файл .【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` отображается в формате JSON и отображается на табло.

Сообщение о том, что произошло в 2017 году:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

Для работы с рабочей областью выберите команду восходящего потока, нажмите кнопку "Установить" для `status.md`. Он сказал, что это не так.

## 4. تشغيلات курить مؤتمتة

Доступ к интерфейсу командной строки для доступа к интерфейсу командной строки (для доступа к шлюзу). عدم تطابق проявляется). Для дыма используется `scripts/sorafs_direct_mode_smoke.sh` и `sorafs_cli fetch`, для чего требуется дополнительная информация. وحفظ الـ табло والتقاط الملخص.

Информация о:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```

- Запустите CLI для изменения ключ=значение (код `docs/examples/sorafs_direct_mode_smoke.conf`). Дайджест الخاص بالـ манифест ومدخلات рекламирует للموفّر بقيم الإنتاج قبل التشغيل.
- `--policy` вместо `docs/examples/sorafs_direct_mode_policy.json`, Луи Уинстона и JSON в формате `sorafs_orchestrator::bindings::config_to_json`. Откройте интерфейс командной строки `--orchestrator-config=PATH` для получения дополнительной информации. الأعلام يدويا.
- عندما لا يكوون `sorafs_cli` على `PATH`, يبنيه المساعد من crate `sorafs_orchestrator` (в выпуске) بحيث Тэхен Тэйр курит, когда его курят.
- Сообщение:
  - Установите флажок (`--output`, `artifacts/sorafs_direct_mode/payload.bin`).
  - Запись (`--summary`, افتراضيا بجوار الحمولة) на сайте منطقة التليمترية Он был создан в 2017 году в Кейл-Лейне.
  - Табло отображается в формате JSON (формат `fetch_state/direct_mode_scoreboard.json`). Он был убит в фильме "Старый мир".
- Обновление: Информационная поддержка `cargo xtask sorafs-adoption-check`. Открытое табло. В 2007 году он был избран в 1980-е годы. Установите `--min-providers=<n>`, чтобы установить его. Он был создан в 2017 году (`--adoption-report=<path>` был удален с сайта) ويمرر المساعد `--require-direct-only` افتراضيا (مطابق لمسار الرجوع) و`--require-telemetry` عندما تمرر العلم المطابق. Запустите `XTASK_SORAFS_ADOPTION_FLAGS` и выполните команду xtask (для `--allow-single-source` выполните команду تتسامح البوابة مع الرجوع وتفرضه). لا تتجاوز بوابة الاعتماد باستخدام `--skip-adoption-check`, чтобы получить доступ к Он выступил с предложением о том, как Стоун-Джеймс вступит в должность президента США. تقرير الاعتماد.

## 5. قائمة تحقق الإطلاق1. **Загрузить файл:** Создайте файл JSON для файла `iroha_config` и установите его. В تذكرة التغيير.
2. **Запуск шлюза:** Зарегистрируйтесь в разделе Torii и проверьте TLS и TLV-файлы. Он сказал, что это не так. Откройте для себя ворота Лос-Анджелеса.
3. **Можно использовать:** Попросите кого-нибудь, кто хочет сделать это, если хотите. Он был убит в 1980-х годах.
4. **Дизайн фильма:** В фильме "Постановка спектакля" выступила актриса Код Torii. Откройте табло с помощью CLI.
5. **Подробнее:** Установите флажок `transport_policy` или `direct_only` (например, `transport_policy`). قد اخترت `soranet-first`) и установите флажок `sorafs_fetch`, затем установите флажок المزوّدين). Впервые используется SoraNet для SNNet-4/5/5a/5b/6a/7/8/12/13 для `roadmap.md:532`.
6. **Восстановление экрана:** На экране появится табло, которое можно увидеть на экране. تذكرة التغيير. حدّث `status.md` بتاريخ السريان и شذوذات.

احتفظ بالقائمة دليل `sorafs_node_ops` حتى يتمكن المشغلون من التمرين قبل التحويل الفعلي. В составе SNNet-5 и GA, а также в штате Джорджия, штат Калифорния, США. إنتاج.

## 6. Нажмите кнопку «Открыть»

Он был отправлен на базу SF-6c. Табло с табло и конверт с манифестом, а также текстовое табло с изображением президента США. `cargo xtask sorafs-adoption-check` в приложении. Он сказал: تذاكر التغيير.

- **Возможность подключения:** Для `scoreboard.json` и `transport_policy="direct_only"` (для `transport_policy_override=true`) Сан-Франциско Сеймс). Он сказал Скелету, что он хочет, чтобы он сделал это. В 2007 году он был рожден в 1980-х годах.
- **Проверка:** Для подключения требуется только шлюз `provider_count=0` и `gateway_provider_count=<n>`. Код Torii. Формат JSON: с помощью CLI/SDK, встроенного программного обеспечения и встроенного программного обеспечения. التي تُغفل الفصل.
- **Для манифеста:** Зарегистрирован Torii и установлен `--gateway-manifest-envelope <path>` (для SDK). Установите `gateway_manifest_provided` и `gateway_manifest_id`/`gateway_manifest_cid` или `scoreboard.json`. Соответствует `summary.json` и `manifest_id`/`manifest_cid`. Он сказал:
- **Обратное сообщение:** عندما ترافق التليمترية الالتقاط, شغّل البوابة مع `--require-telemetry` был добавлен в программу «Управление в режиме онлайн». Его персонаж - Коллинз, Джон Тэхен, Луис Уилсон, CI, США. التغيير توثيق الغياب.

Название:

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```

أرفق `adoption_report.json` на табло и в конверте с манифестом и дымом. Он был создан для использования в CI (`ci/check_sorafs_orchestrator_adoption.sh`) в качестве примера. الوضع المباشر قابلة للتدقيق.