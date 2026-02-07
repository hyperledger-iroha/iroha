---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: 開発者-SDK-インデックス
タイトル: SDK SoraFS
サイドバーラベル: SDK の開発
説明: SoraFS。
---

:::note ノート
テストは `docs/source/sorafs/developer/sdk/index.md` です。スフィンクス、スフィンクス、スフィンクス。
:::

テストは、SoraFS を実行します。
Rust の [مقتطفات Rust SDK](./developer-sdk-rust.md) を参照してください。

## 大事な

- **Python** — `sorafs_multi_fetch_local` (ختبارات دخان للمُنسِّق المحلي)
  `sorafs_gateway_fetch` (E2E للبوابة) يقبلان الآن `telemetry_region` اختياريًا
  `transport_policy`
  (`"soranet-first"`、`"soranet-strict"`、`"direct-only"`) は、
  CLI。プロキシ QUIC محلي، يعيد `sorafs_gateway_fetch` مانيفست المتصفح تحت
  `local_proxy_manifest` 信頼バンドルを保証します。
- **JavaScript** — يعكس `sorafsMultiFetchLocal` مساعد Python ويعيد بايتات الحمولة وملخصات
  認証 `sorafsGatewayFetch` 認証 Torii 認証 プロキシ 認証
  CLI を使用してください。
- **Rust** — يمكن للخدمات تضمين المُجدول مباشرةً عبر `sorafs_car::multi_fetch`؛やあ
  [Rust SDK](./developer-sdk-rust.md) のプルーフ ストリーム。
- **Android** — يعيد `HttpClientTransport.sorafsGatewayFetch(…)` استخدام مُنفّذ HTTP الخاص
  Torii は `GatewayFetchOptions` です。ああ、
  `ClientConfig.Builder#setSorafsGatewayUri` ومع تلميح رفع PQ
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) ログイン してください。
  PQ です。

## スコアボード والسياسات

Python (`sorafs_multi_fetch_local`) وJavaScript
(`sorafsMultiFetchLocal`) スコアボード スコアボード スコアボード CLI:

- スコアボード スコアボード`use_scoreboard=True`
  (أو وفّر إدخالات `telemetry`) フィクスチャー حتى يستخلص المساعد ترتيب
  広告を掲載しています。
- اضبط `return_scoreboard=True` لتلقي الأوزان المحسوبة مع إيصالات الـ chunk حتى تتمكن
  CI は、 التقاط التشخيصات です。
- ستخدم مصفوفتَي `deny_providers` أو `boost_providers` لرفض الأقران أو إضافة
  `priority_delta` 認証済み。
- حافظ على الوضع الافتراضي `"soranet-first"` ما لم تكن تجهّز لخفض المستوى؛ قدّم
  `"direct-only"` فقط عندما يتعين على منطقة امتثال تجنّب المرحلات أو عند تدريب
  SNNet-5a، واحجز `"soranet-strict"` لطيارين PQ のみのセキュリティ。
- `scoreboardOutPath` و`scoreboardNowUnixSecs` を確認してください。ああ
  `scoreboardOutPath` スコアボード المحسوبة (يعكس علم CLI `--scoreboard-out`)
  `cargo xtask sorafs-adoption-check` のテスト SDK のテスト
  `scoreboardNowUnixSecs` フィクスチャー フィクスチャー `assume_now` フィクスチャー
  صفية قابلة لعادة الإنتاج. JavaScript يمكنك أيضًا ضبط
  `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` عند حذف الملصق
  `region:<telemetryRegion>` (フォールバック `sdk:js`)。 Python のバージョン
  `telemetry_source="sdk:python"` كلما حفظ لوحة スコアボード ويُبقي البيانات الوصفية
  そうです。

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