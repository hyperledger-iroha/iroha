---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-index.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: developer-sdk-index
כותרת: Guias de SDK da SoraFS
sidebar_label: Guias de SDK
תיאור: Trechos por linguagem para integrar artefatos da SoraFS.
---

:::שים לב Fonte canonica
Esta pagina espelha `docs/source/sorafs/developer/sdk/index.md`. Mantenha ambas as copias sincronizadas.
:::

השתמש ב-este hub para acompanhar os helpers por linguagem que acompanham a toolchain da SoraFS.
Para snippets especificos de Rust, va para [Rust SDK snippets](./developer-sdk-rust.md).

## עוזרים לשפות

- **Python** - `sorafs_multi_fetch_local` (מבחני עשן עושים orquestrador local) ה
  `sorafs_gateway_fetch` (תרגילים E2E de gateway) agora aceitam um `telemetry_region`
  אופציונלי לעקוף את `transport_policy`
  (`"soranet-first"`, `"soranet-strict"` או `"direct-only"`), espelhando os knobs de
  השקה לעשות CLI. Quando um proxy QUIC local sobe, `sorafs_gateway_fetch` retorna o
  מניפסט דפדפן עם `local_proxy_manifest` עבור חבילת אמון
  para adaptadores de navegador.
- **JavaScript** - `sorafsMultiFetchLocal` espelha o helper de Python, retornando
  bytes de payload ו resumes de recibos, enquanto `sorafsGatewayFetch` תרגול
  שערים Torii, encadeia manifests de proxy local e expoe os mesmos overrides
  de telemetria/transporte do CLI.
- **חלודה** - servicos podem embutir o scheduler diretamente via
  `sorafs_car::multi_fetch`; veja a referencia de
  [Rust SDK snippets](./developer-sdk-rust.md) לעזרה של הוכחה-זרם e
  integracao do orquestrador.
- **אנדרואיד** - `HttpClientTransport.sorafsGatewayFetch(...)` מחדש או מבצע HTTP
  do Torii e honra `GatewayFetchOptions`. שלב com
  `ClientConfig.Builder#setSorafsGatewayUri` e o רמז להעלאה PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) quando העלאות precisarem
  ficar em caminhos somente PQ.

## לוח תוצאות וכפתורי פוליטיקה

OS helpers de Python (`sorafs_multi_fetch_local`) ו-JavaScript
(`sorafsMultiFetchLocal`) תערוכת או לוח תוצאות לעשות מתזמן com telemetria usado
pelo CLI:- Binarios de producao habilitam o לוח התוצאות por padrao; defina `use_scoreboard=True`
  (ou forneca entradas `telemetry`) ao reproduzir fixtures para que o helper derive
  a ordenacao ponderada de provedores a partir de metadados de advert e צילומי מצב
  הטלמטריה האחרונה.
- Defina `return_scoreboard=True` para receber os pesos calculados junto com recibos
  de chunk, אישור רישומי CI capturem diagnosticos.
- השתמשו במערכים `deny_providers` או `boost_providers` לחיפוש עמיתים או הוספת מידע
  `priority_delta` לבחירת מתזמן.
- Mantenha a postura padrao `"soranet-first"` a menos que esteja preparando um שדרוג לאחור;
  forneca `"direct-only"` apenas quando uma regiao de compliance ממסרי evitar מדויקים
  או אאו נסייר או fallback SNNet-5a, e Reserve `"soranet-strict"` עבור פיילוטים PQ בלבד
  com aprovacao de governanca.
- Helpers de gateway tambem expoem `scoreboardOutPath` e `scoreboardNowUnixSecs`.
  Defina `scoreboardOutPath` עבור מתמיד או לחישוב לוח התוצאות (ספרייה או דגל
  `--scoreboard-out` do CLI) para que `cargo xtask sorafs-adoption-check` valide
  artefatos de SDK, השתמשו ב-`scoreboardNowUnixSecs` מתקני quando precisarem de um
  valor `assume_now` estavel para metadados reproduziveis. אין עוזר ל-JavaScript,
  voce tambem pode definir `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`;
  quando o label e omitido, ele deriva `region:<telemetryRegion>` (fallback para `sdk:js`).
  O helper de Python emite automaticamente `telemetry_source="sdk:python"` quando
  להתמיד על לוח התוצאות e mantem metadados implicitos desabilitados.

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