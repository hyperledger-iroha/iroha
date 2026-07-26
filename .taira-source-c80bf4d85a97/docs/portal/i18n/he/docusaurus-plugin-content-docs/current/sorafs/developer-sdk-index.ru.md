---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-index.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: developer-sdk-index
כותרת: Руководства по SDK SoraFS
sidebar_label: Руководства по SDK
תיאור: Языковые сниппеты для интеграции артефактов SoraFS.
---

:::note Канонический источник
:::

Используйте этот хаб, чтобы отслеживать שפה עוזרי, поставляемые с toolchain SoraFS.
Для Rust-специфичных сниппетов переходите к [Rust SDK snippets](./developer-sdk-rust.md).

## עוזרים Языковые

- **Python** — `sorafs_multi_fetch_local` (בדיקות עשן локального оркестратора) и
  `sorafs_gateway_fetch` (שער E2E упражнения) теперь принимают опциональный
  `telemetry_region` לעקוף עבור `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` או `"direct-only"`), отражая ידיות פתיחה
  CLI. Когда локальный QUIC proxy поднимается, `sorafs_gateway_fetch` возвращает
  מניפסט דפדפן ב-`local_proxy_manifest`, чтобы тесты могли передать חבילת אמון
  браузерным адаптерам.
- **JavaScript** — `sorafsMultiFetchLocal` отражает Python helper, возвращая בתים מטען
  и סיכומים квитанций, тогда как `sorafsGatewayFetch` упражняет Torii שערים,
  прокидывает מניפסט локального proxy и раскрывает те же עקיפות טלמטריה/תחבורה,
  что ו-CLI.
- **חלודה** — сервисы могут встраивать מתזמן напрямую через `sorafs_car::multi_fetch`;
  см. [קטעי SDK של חלודה](./developer-sdk-rust.md) לעזרי proof-stream и интеграции
  оркестратора.
- **אנדרואיד** — `HttpClientTransport.sorafsGatewayFetch(…)` переиспользует Torii מבצע HTTP
  и учитывает `GatewayFetchOptions`. Комбинируйте с
  רמז להעלאה של `ClientConfig.Builder#setSorafsGatewayUri` ו-PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) когда загрузки должны идти по PQ-only путям.

## לוח תוצאות и כפתורי מדיניות

עוזרי Python (`sorafs_multi_fetch_local`) ו-JavaScript (`sorafsMultiFetchLocal`)
לוח תוצאות מתזמן מודע לטלמטריה, используемый CLI:- ייצור бинари включают לוח התוצאות по умолчанию; установите `use_scoreboard=True`
  (или передайте `telemetry` ערכים) при проигрывании גופי, чтобы helper выводил
  взвешенный порядок провайдеров из פרסומות מטא נתונים ותמונות מצב של טלמטריה.
- Установите `return_scoreboard=True`, чтобы получать рассчитанные веса вместе с chunk קבלות,
  позволяя CI логам фиксировать диагностику.
- שימוש במכשירים `deny_providers` או `boost_providers` עבור עמיתים או דוברים
  `priority_delta`, מתזמן когда выбирает провайдеров.
- Сохраняйте позу `"soranet-first"` по умолчанию, если только не готовите שדרוג לאחור;
  указывайте `"direct-only"` лишь когда תאימות регион обязан избегать ממסרים או при
  החזר SNNet-5a, ו-`"soranet-strict"` ל-PQ-only пилотов с
  одобрением ממשל.
- עוזרי שער также раскрывают `scoreboardOutPath` ו-`scoreboardNowUnixSecs`.
  Задайте `scoreboardOutPath` для сохранения вычисленного לוח התוצאות (соответствует флагу
  CLI `--scoreboard-out`), чтобы `cargo xtask sorafs-adoption-check` мог валидировать
  SDK артефакты, используйте `scoreboardNowUnixSecs`, когда fixtures требуется
  стабильное значение `assume_now` עבור воспроизводимых מטא נתונים. В עוזר JavaScript можно
  дополнительно установить `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`;
  если label опущен, он выводит `region:<telemetryRegion>` (החלפה של `sdk:js`). עוזר פייתון
  автоматически пишет `telemetry_source="sdk:python"` при сохранении לוח תוצאות ו держит
  מטא נתונים מרומזים выключенными.

```python
result = sorafs_multi_fetch_local(
    plan_json,
    providers,
    options={
        "use_scoreboard": True,
        "telemetry": [
            {"provider_id": "alpha-id", "qos_score": 98, "last_updated_unix": 4_100_000_000},
            {"provider_id": "beta-id", "penalty": True},
        ],
        "return_scoreboard": True,
        "deny_providers": ["beta"],
        "boost_providers": [{"provider": "alpha", "delta": 25}],
    },
)
for row in result["scoreboard"]:
    print(row["provider_id"], row["eligibility"], row["normalized_weight"])
```