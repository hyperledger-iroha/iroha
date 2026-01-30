---
id: developer-sdk-index
lang: he
direction: rtl
source: docs/portal/docs/sorafs/developer-sdk-index.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note מקור קנוני
עמוד זה משקף את `docs/source/sorafs/developer/sdk/index.md`. שמרו על שתי הגרסאות מסונכרנות עד שהסט הישן של Sphinx יופסק.
:::

השתמשו בהאב הזה כדי לעקוב אחר ה-helpers לפי שפה שמגיעים עם toolchain של SoraFS.
למקטעי Rust ספציפיים קפצו ל-[Rust SDK snippets](./developer-sdk-rust.md).

## Helpers לפי שפה

- **Python** — `sorafs_multi_fetch_local` (בדיקות עשן לאורקסטרטור המקומי) ו-
  `sorafs_gateway_fetch` (תרגילי E2E ל-gateway) מקבלים כעת `telemetry_region` אופציונלי
  לצד override ל-`transport_policy`
  (`"soranet-first"`, `"soranet-strict"` או `"direct-only"`), בהתאם ל-knobs של
  rollout ב-CLI. כאשר proxy QUIC מקומי עולה, `sorafs_gateway_fetch` מחזיר את
  manifest הדפדפן תחת `local_proxy_manifest` כדי שהבדיקות יוכלו למסור את trust bundle
  למתאמי דפדפן.
- **JavaScript** — `sorafsMultiFetchLocal` משקף את helper של Python ומחזיר bytes של
  payload וסיכומי קבלות, בעוד `sorafsGatewayFetch` מפעיל gateways של Torii, משחיל
  manifests של proxy מקומי וחושף את אותם overrides של telemetry/transport כמו ה-CLI.
- **Rust** — שירותים יכולים להטמיע את ה-scheduler ישירות דרך `sorafs_car::multi_fetch`;
  ראו את [Rust SDK snippets](./developer-sdk-rust.md) לעזרי proof-stream ואינטגרציית
  orchestrator.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` עושה reuse ל-Torii HTTP executor
  ומכבד `GatewayFetchOptions`. שלבו זאת עם
  `ClientConfig.Builder#setSorafsGatewayUri` ועם רמז העלאת PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) כאשר העלאות חייבות להיצמד
  למסלולים PQ-only.

## Scoreboard ו-knobs של מדיניות

ה-helpers של Python (`sorafs_multi_fetch_local`) ושל JavaScript
(`sorafsMultiFetchLocal`) חושפים את scoreboard של ה-scheduler המודע לטלמטריה
שבשימוש ה-CLI:

- הבינארים בפרודקשן מפעילים את ה-scoreboard כברירת מחדל; הגדירו `use_scoreboard=True`
  (או ספקו ערכי `telemetry`) בעת replay של fixtures כדי שה-helper יגזור סדר משוקלל
  של ספקים ממטאדאטה של adverts ומצלומי טלמטריה עדכניים.
- הגדירו `return_scoreboard=True` כדי לקבל את המשקלים המחושבים לצד קבלות chunk וכך
  לאפשר ללוגי CI ללכוד דיאגנוסטיקה.
- השתמשו במערכי `deny_providers` או `boost_providers` כדי לדחות peers או להוסיף
  `priority_delta` כאשר ה-scheduler בוחר ספקים.
- שמרו על ברירת המחדל `"soranet-first"` אלא אם אתם מבצעים downgrade; ספקו
  `"direct-only"` רק כאשר אזור ציות חייב להימנע מ-relays או בעת חזרה על fallback
  של SNNet-5a, ושמרו `"soranet-strict"` לפיילוטים PQ-only עם אישור ממשל.
- Helpers של gateway חושפים גם `scoreboardOutPath` ו-`scoreboardNowUnixSecs`.
  הגדירו `scoreboardOutPath` כדי לשמר את ה-scoreboard המחושב (משקף את דגל
  `--scoreboard-out` של ה-CLI) כך ש-`cargo xtask sorafs-adoption-check` יוכל לאמת
  ארטיפקטים של SDK, והשתמשו ב-`scoreboardNowUnixSecs` כאשר fixtures צריכים ערך
  `assume_now` יציב למטאדאטה שחזורית. ב-helper של JavaScript ניתן גם להגדיר
  `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`; כאשר התווית מושמטת
  הוא גוזר `region:<telemetryRegion>` (עם fallback ל-`sdk:js`). ה-helper של Python
  פולט אוטומטית `telemetry_source="sdk:python"` בכל פעם שהוא משמר scoreboard ומשאיר
  מטאדאטה אימפליציטית כבויה.

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
