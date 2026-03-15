---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# إعلانات مزودي متعدد المصادر والجدولة

هذه الصفحة المواصفة القياسية في
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md)。
ستخدم هذا المستند للمخططات Norito حرفيا وسجلات التغيير؛ سخة البوابة تُبقي إرشادات المشغلين
SDK は SoraFS です。

## إضافات مخطط Norito

### قدرة النطاق (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – パスワード `>= 1`。
- `min_granularity` – 認証 `1 <= القيمة <= max_chunk_span`。
- `supports_sparse_offsets` – يسمح بإزاحات غير متصلة في طلب واحد。
- `requires_alignment` – 認証済み、`min_granularity`。
- `supports_merkle_proof` – يشير إلى دعم شاهد PoR。

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` ログイン
ペイロードとゴシップの両方。

### `StreamBudgetV1`
- 回答: `max_in_flight`、`max_bytes_per_sec`、`burst_bytes`。
- قواعد التحقق (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`、`max_bytes_per_sec > 0`。
  - `burst_bytes` عند وجوده يجب أن يكون `> 0` و`<= max_bytes_per_sec`。

### `TransportHintV1`
- 番号: `protocol: TransportProtocol`、`priority: u8` (0-15 インチ)
  `TransportHintV1::validate`)。
- バージョン: `torii_http_range`、`quic_stream`、`soranet_relay`、
  `vendor_reserved`。
- 最高のパフォーマンスを実現します。

### إضافات `ProviderAdvertBodyV1`
- `stream_budget` 番号: `Option<StreamBudgetV1>`。
- `transport_hints` 番号: `Option<Vec<TransportHintV1>>`。
- كلا الحقلين يمران الآن عبر `ProviderAdmissionProposalV1` وأغلفة الحوكمة
  CLI と JSON のフィクスチャ。

## قالربط بالحوكمة

`ProviderAdvertBodyV1::validate` و`ProviderAdmissionProposalV1::validate`
セキュリティ:

- جبأن تُفك قدرات النطاق وتستوفي حدود النطاق/التحبيب。
- ストリームの予算/トランスポートのヒント TLV の情報
  `CapabilityType::ChunkRangeFetch` وقائمة はヒントを示します。
- 広告を表示します。
- 提案/広告 بيانات النطاق عبر `compare_core_fields`
  ペイロードとゴシップの情報。

ログイン して翻訳を追加する
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`。

## フィクスチャ

- ペイロード数は `range_capability` و`stream_budget` و`transport_hints` です。
  `/v1/sorafs/providers` の備品の確認يجب أن تتضمن
  JSON は、ヒントを示します。
- `cargo xtask sorafs-admission-fixtures` ストリーム予算とトランスポートのヒント
  アーティファクト JSON は、JSON を使用して作成されます。
- フィクスチャ `fixtures/sorafs_manifest/provider_admission/` の番号:
  - 広告
  - `multi_fetch_plan.json` は、SDK のフェッチを実行します。

## كامل المُنسق وTorii

- يعيد Torii `/v1/sorafs/providers` بيانات قدرة النطاق المحللة مع
  `stream_budget` و`transport_hints`。ダウングレード عندما يحذف
  重要な問題は、次のとおりです。
- يفرض المُنسق متعدد المصادر (`sorafs_car::multi_fetch`) حدود النطاق ومحاذاة
  ストリームの予算を確認します。翻訳する 翻訳する 翻訳する
  チャンクは、大規模なチャンクです。
- يبث `sorafs_car::multi_fetch` ダウングレード (إخفاقات المحاذاة،
  طلبات المخفّضة) لتمكين المشغلين من تتبع سبب ازودين بعينهم أثناء التخطيط。

## 大事な

Torii Grafana **SoraFS フェッチ可観測性**
(`dashboards/grafana/sorafs_fetch_observability.json`) قواعد التنبيه المرافقة
(`dashboards/alerts/sorafs_fetch_rules.yml`)。

| और देखेंああ |ああ |ああ |
|----------|----------|----------|----------|
| `torii_sorafs_provider_range_capability_total` |ゲージ | `feature` (`providers`、`supports_sparse_offsets`、`requires_alignment`、`supports_merkle_proof`、`stream_budget`、`transport_hints`) زودونيعلنون ميزات قدرة النطاق。 |
| `torii_sorafs_range_fetch_throttle_events_total` |カウンター | `reason` (`quota`、`concurrency`、`byte_rate`)必要な情報を取得してください。 |
| `torii_sorafs_range_fetch_concurrency_current` |ゲージ | — |最高のパフォーマンスを見せてください。 |

PromQL の説明:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

ستخدم عداد التخفيض لتأكيد تطبيق الحصص قبل تفعيل القيم الافتراضية للمُنسق متعددああ
ログインしてください。 ログインしてください。