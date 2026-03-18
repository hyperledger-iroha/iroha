---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# بروتوكول عقدة ↔ عميل SoraFS

يلخص هذا الدليل التعريف المعتمد للبروتوكول في
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)。
上流の Norito の مستوى البايت وسجل التغييرات؛
ランブック SoraFS を実行してください。

## إعلانات المزوّد والتحقق

يقوم مزوّدو SoraFS ببث حمولات `ProviderAdvertV1` (راجع)
`crates/sorafs_manifest::provider_advert`) は、 المُشغّل الحوكمة です。
ثبت الإعلانات بيانات الاكتشاف والضوابط التي يفرضها المُنسِّق متعدد المصادر
ありがとう。

- **مدة الصلاحية** — `issued_at < expires_at ≤ issued_at + 86,400 s`。ヤシの木
  12 月 12 日。
- **TLV القدرات** — تسرد قائمة TLV ميزات النقل (Torii، QUIC+Noise، ترحيلات)
  SoraNet（ソラネット）。 يمكن تجاهل الأكواد المجهولة عندما
  `allow_unknown_capabilities = true` グリース。
- **تلميحات QoS** — طبقة `availability` (ホット/ウォーム/コールド)
  ストリームを視聴してください。 QoS を使用する
  ありがとうございます。
- **エンドポイントのランデブー** — エンドポイント、TLS/ALPN、エンドポイント
  最高のパフォーマンスを見せてください。
- **ステータス** — `min_guard_weight` ファンアウト AS/プール و
  `provider_failure_threshold` はマルチピアをサポートしています。
- **معرّفات الملف الشخصي** — يجب على المزوّدين نشر المقبض المعتمد (مثل
  `sorafs.sf1@1.0.0`) تساعد `profile_aliases` الاختيارية العملاء القدامى على
  ああ。

賭け金を賭ける/賭け金を賭ける/賭け金を賭ける/賭け金を賭ける
QoS を確認してください。テストを終了します。
قبلبث التحديثات (`compare_core_fields`) 。

### और देखें

評価:

|ああ |ああ |
|------|------|
| `CapabilityType::ChunkRangeFetch` | يعلن `max_chunk_span` و`min_granularity` وأعلام المحاذاة/الأدلة。 |
| `StreamBudgetV1` | (`max_in_flight`、`max_bytes_per_sec`、و`burst` اختياري)。 طلب قدرة نطاق。 |
| `TransportHintV1` |重要な情報 (`torii_http_range`、`quic_stream`、`soranet_relay`)。 `0–15` を確認してください。 |

回答:

- يجب على خطوط إعلانات المزوّدين التحقق من قدرة النطاق وميزانية ストリーム وتلميحات
  重要な問題は、次のとおりです。
- `cargo xtask sorafs-admission-fixtures` يجمع إعلانات متعددة المصادر مع
  備品は `fixtures/sorafs_manifest/provider_admission/` です。
- 評価 `stream_budget` 評価 `transport_hints`
  CLI/SDK のセキュリティとマルチソースのセキュリティの強化
  Torii。

## ナオミ ナオミ ナオミ

HTTP メッセージを送信してください。

### `GET /v1/sorafs/storage/car/{manifest_id}`|回答 |翻訳 |
|----------|----------|
| **ヘッダー** | `Range` (ナレーション) `dag-scope: block`、`X-SoraFS-Chunker`、`X-SoraFS-Nonce` `X-SoraFS-Stream-Token` Base64 です。 |
| **回答** | `206` مع `Content-Type: application/vnd.ipld.car`، و`Content-Range` يصف النافذة المقدمة، وبيانات `X-Sora-Chunk-Range`، وإعادةチャンカー/トークン。 |
| **障害モード** | `416` 日本語版 `401` 日本語版 `429` 日本語版ストリーム/バイト。 |

### `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

ダイジェスト版をダウンロードしてください。意味
自動車を運転してください。

## سير عمل المُنسِّق متعدد المصادر

SF-6 のセキュリティ (CLI Rust `sorafs_fetch` と SDK のサポート)
`sorafs_orchestrator`):

1. **جمع المدخلات** — فك خطة شرائح المانيفست، وجلب أحدث الإعلانات، وتمرير
   (`--telemetry-json` と `TelemetrySnapshot`)。
2. **スコアボード** — يقوم `Orchestrator::build_scoreboard` スコアボード
   سسجيل أسباب الرفض؛ `sorafs_fetch --scoreboard-out` JSON です。
3. ******** — يفرض `fetch_with_scoreboard` (`--plan`) قيود النطاق،
   ميزانيات ストリーム، وحدود إعادة المحاولة/الأقران (`--retry-budget`、`--max-peers`)
   ストリーム トークンは、ストリーム トークンです。
4.**********************************************************************************************************************
   CLI での動作 `provider_reports` و`chunk_receipts` و`ineligible_providers`
   そうです。

開発者/SDK:

|ああ |ああ |
|------|------|
| `no providers were supplied` |最高です。 |
| `no compatible providers available for chunk {index}` | عدم توافق نطاق أو ميزانية لشريحة محددة. |
| `retry budget exhausted after {attempts}` | `--retry-budget` を確認してください。 |
| `no healthy providers remaining` |最高のパフォーマンスを見せてください。 |
| `streaming observer failed` |自動車運転免許証。 |
| `orchestrator invariant violated` |スコアボード、JSON、CLI など。 |

## 認証済み

- 回答:  
  `sorafs_orchestrator_active_fetches`、`sorafs_orchestrator_fetch_duration_ms`、
  `sorafs_orchestrator_retries_total`、`sorafs_orchestrator_provider_failures_total`
  (マニフェスト/リージョン/プロバイダー)。 ضبط `telemetry_region` في الإعداد أو عبر
  CLI を使用して、セキュリティを強化します。
- CLI/SDK のスコアボード JSON のスコアボードの表示
  SF-6/SF-7 を使用してください。
- テスト `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  SRE は、SRE を実行します。

## CLI وREST

- `iroha app sorafs pin list|show` و`alias list` و`replication list` 残りの部分
  ピン Norito JSON の認証が必要です。
- `iroha app sorafs storage pin` و`torii /v1/sorafs/pin/register` のマニフェスト
  Norito JSON の証明 別名 後継者証拠
  証明 `400` 証明 証明 `503` 証明 `Warning: 110` 証明
  証明は `412` です。
- 残り REST (`/v1/sorafs/pin`、`/v1/sorafs/aliases`、`/v1/sorafs/replication`)
  تتضمن هياكل attestation حتى يتمكن العملاء من التحقق من البيانات مقابل أحدث
  ありがとうございます。## いいえ

- 回答:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- CLI: `crates/iroha_cli/src/commands/sorafs.rs`、
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- 回答: `crates/sorafs_orchestrator`
- メッセージ: `dashboards/grafana/sorafs_fetch_observability.json`